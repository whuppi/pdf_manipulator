//! The one counting allocator: what each thread holds, and what the process holds.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Every allocation goes to `std::alloc::System` (the platform allocator on
//! native, the module's own on wasm32) with its size added to two counts: a
//! per-thread pair of (live, peak) and a process-wide pair. A lane is one
//! thread, so the per-thread figure is the lane's heap and is blind to every
//! other lane. Memory that crosses a thread — a request allocated on the
//! caller's thread and freed on the lane's, a response allocated on the lane's
//! thread and freed on Dart's — would skew it, so the boundary moves those
//! bytes with [`adopt`] and [`release`].
//!
//! The machine's font caches (the renderer's `FONT_BYTES_CACHE` and
//! `FACE_CACHE`) belong to the process, not to the lane that happened to fill
//! them, so their inserts and evictions run inside [`process_owned`], which
//! keeps those bytes in the process-wide pair and out of the thread's. The one
//! place the thread's figure can still drift: an evicted `Arc` that another
//! lane still held a clone of is freed by that lane, which never counted it,
//! and its live figure dips by the size of the value. The drift is bounded by
//! the caches' budgets (`MACHINE_FONT_BYTES` and `MACHINE_FACE_BYTES`, which
//! share their allocations) and is accepted: it can only lower that lane's
//! figure, so it never retires an executor that should be kept.
//!
//! The per-thread pair is a `thread_local!` `Cell` with a const initialiser
//! and no destructor. An allocator may touch only that kind: a lazily
//! initialised or `Drop` thread-local allocates, or registers a destructor, on
//! first use, and that re-enters the allocator. Where a thread-local is a
//! pthread key (Android), even this kind allocates its value on a thread's
//! first use; Rust 1.93 takes that value from the system allocator, and before
//! it, std took it from this one, which recursed until the stack overflowed.
//! So the engine needs Rust 1.93 (`RUST_VERSION` in the package's
//! tool/versions.env).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What the pool keeps resident: one document plus one operation.
///
/// One resident document holds an object cache of `DEFAULT_OBJECT_CACHE_MAX_BYTES`
/// (8 MiB, document.rs), a content cache of `CONTENT_CACHE_BYTES` (4 MiB,
/// host/content/cache.rs), fonts of `DOCUMENT_FONT_BYTES` (1 MiB,
/// host/font_budget.rs) and the text caches of host/font_budget.rs (spans
/// 512 KiB, characters, pages, Form spans and Form images 256 KiB each, the
/// search index 1 MiB: 2.5 MiB): 15.5 MiB. Every lawful operation peaks at
/// most 16 MiB over its baseline (the `limit` rows of tool/memprobe/src/main.rs),
/// so one document and one operation stay under 31.5 MiB, inside 32 MiB. The
/// three exception rows retire their executor.
pub const KEEP_BYTES: usize = 32 * 1024 * 1024;

struct Counting;

std::thread_local! {
    /// What the current thread holds and the most it held since [`reset_peak`].
    static MINE: Cell<(isize, isize)> = const { Cell::new((0, 0)) };

    /// How many [`process_owned`] calls the current thread is inside.
    static OWNED: Cell<u32> = const { Cell::new(0) };
}

/// What the process holds, and the most it held since [`process_reset_peak`].
pub(crate) static PROCESS_LIVE: AtomicUsize = AtomicUsize::new(0);
pub(crate) static PROCESS_PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(bytes: usize) {
    let now = PROCESS_LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PROCESS_PEAK.fetch_max(now, Ordering::Relaxed);
    move_mine(bytes as isize);
}

fn shrink(bytes: usize) {
    PROCESS_LIVE.fetch_sub(bytes, Ordering::Relaxed);
    move_mine(-(bytes as isize));
}

fn move_mine(bytes: isize) {
    if OWNED.with(Cell::get) > 0 {
        return;
    }
    MINE.with(|m| {
        let (live, peak) = m.get();
        let live = live + bytes;
        m.set((live, peak.max(live)));
    });
}

// SAFETY: every call is forwarded to the system allocator unchanged; only counters are added.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            grow(layout.size());
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(layout);
        if !p.is_null() {
            grow(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        System.dealloc(p, layout);
        shrink(layout.size());
    }

    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let q = System.realloc(p, layout, new_size);
        if !q.is_null() {
            if new_size >= layout.size() {
                grow(new_size - layout.size());
            } else {
                shrink(layout.size() - new_size);
            }
        }
        q
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The bytes the current thread holds. Memory it frees that another thread
/// allocated counts against it until [`adopt`] says it was handed over.
pub fn live() -> usize {
    MINE.with(|m| m.get().0).max(0) as usize
}

/// The most the current thread held since [`reset_peak`], or since it began.
pub fn peak() -> usize {
    MINE.with(|m| m.get().1).max(0) as usize
}

/// Starts the current thread's peak over from what it holds now.
pub fn reset_peak() {
    MINE.with(|m| {
        let (live, _) = m.get();
        m.set((live, live));
    });
}

/// Runs `f` with every allocation and free on this thread kept out of its
/// count and in the process's. For memory that outlives the lane that
/// allocated it and serves every lane.
pub fn process_owned<T>(f: impl FnOnce() -> T) -> T {
    struct Owned;
    impl Drop for Owned {
        fn drop(&mut self) {
            OWNED.with(|o| o.set(o.get() - 1));
        }
    }
    OWNED.with(|o| o.set(o.get() + 1));
    let _owned = Owned;
    f()
}

/// `bytes` that another thread allocated and this thread will free are now
/// this thread's.
pub fn adopt(bytes: usize) {
    move_mine(bytes as isize);
}

/// `bytes` that this thread allocated and another thread will free are no
/// longer this thread's.
pub fn release(bytes: usize) {
    move_mine(-(bytes as isize));
}

/// Runs `f` and returns what it returned with the most bytes the current
/// thread held above where it began. Blind to every other thread, so a bound
/// on it is tight.
pub fn growth<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = MINE.with(|m| {
        let (live, _) = m.get();
        m.set((live, live));
        live
    });
    let out = f();
    let peak = MINE.with(|m| m.get().1);
    (out, (peak - base).max(0) as usize)
}

/// The bytes the whole process holds.
pub fn process_live() -> usize {
    PROCESS_LIVE.load(Ordering::Relaxed)
}

/// The most the whole process held since [`process_reset_peak`], or since it began.
pub fn process_peak() -> usize {
    PROCESS_PEAK.load(Ordering::Relaxed)
}

/// Starts the process's peak over from what it holds now, so one step's
/// growth begins where the last one ended.
pub fn process_reset_peak() {
    PROCESS_PEAK.store(PROCESS_LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}
