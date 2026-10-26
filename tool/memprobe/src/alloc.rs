//! Allocation-counting global allocator — the probe's measuring tape.
//!
//! Wraps `std::alloc::System` and tracks the live allocation size and
//! the high-water mark. Every allocation that goes through Rust's
//! allocator is counted, including the ones the probes pay for.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

static CURRENT: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            track(0, layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        System.dealloc(p, layout);
        track(layout.size(), 0);
    }

    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let q = System.realloc(p, layout, new_size);
        track(layout.size(), new_size);
        q
    }
}

/// Applies one allocation side effect: `grown - released` moves the
/// live count (signed, because a dealloc can arrive before the allocs
/// that predate the counter land), then the peak takes the new mark.
fn track(released: usize, grown: usize) {
    let next = if grown >= released {
        CURRENT.fetch_add((grown - released) as isize, Ordering::Relaxed) + (grown - released) as isize
    } else {
        CURRENT.fetch_sub((released - grown) as isize, Ordering::Relaxed) - (released - grown) as isize
    };
    if next > 0 {
        PEAK.fetch_max(next, Ordering::Relaxed);
    }
}

/// Live allocation size right now, as tracked by this allocator.
pub fn current() -> usize {
    CURRENT.load(Ordering::Relaxed).max(0) as usize
}

/// Sets the peak to the current live count, so a step's growth starts
/// from where the last one ended.
pub fn reset_peak() {
    PEAK.store(CURRENT.load(Ordering::Relaxed).max(0), Ordering::Relaxed);
}

/// How far above `base` the peak reached since the last `reset_peak()`.
pub fn peak_over(base: usize) -> usize {
    PEAK.load(Ordering::Relaxed).saturating_sub(base as isize) as usize
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;
