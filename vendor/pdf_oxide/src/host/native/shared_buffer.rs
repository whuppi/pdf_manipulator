//! Shared memory layout between Rust lane threads and the Dart isolate.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Both sides (Rust + Dart) agree on exact byte offsets. Rust defines
//! the layout here; the Dart side mirrors it byte for byte. The buffer
//! is allocated by Dart via `calloc` and passed to Rust as a raw pointer.
//!
//! Two channel types:
//! - Read channel: engine requests source bytes from the host.
//! - Write channel: engine sends output chunks to the host.
//!
//! Synchronization: a heap-allocated Mutex+Condvar pair, raw pointer
//! stored in the buffer's sync_ptr slot (Dart never reads it).
//! Cross-platform: Linux, macOS, Windows, Android, iOS.
//! The lane thread sleeps (zero CPU) while waiting. The Dart isolate
//! signals via an FFI call that locks the mutex before notify_one().

use crate::host::native::cancel::CancelFlags;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

// ── Flag bits (same values on both Rust and Dart sides) ────────────
//
// Each channel carries three u32 words, and each word has ONE writer, so
// neither side ever read-modify-writes a word the other side writes:
//
//   request word  — written only by Rust. A sequence number, bumped on
//                   every request the channel carries, for the channel's
//                   whole life (a held channel outlives its jobs).
//   response word — written only by Dart. Low nibble: the kind of the
//                   response (READY / ERROR / ACK). Upper 28 bits: the
//                   request number being answered, echoed. Rust takes a
//                   response only when it echoes the request it waits on,
//                   so the late answer of a cancelled request, landing
//                   after a newer request was posted, is ignored; Dart
//                   reads the request word again after its await and
//                   drops an answer whose request has moved on.
//   cancel word   — written only by Rust (the cancellers, and the lane
//                   thread when a job adopts a held channel). Holds
//                   FLAG_CANCELLED; sticky until that adoption.

/// Response kind: host has filled the response data.
pub const FLAG_READY: u32 = 1 << 0;
/// Response kind: host encountered an error.
pub const FLAG_ERROR: u32 = 1 << 1;
/// Cancel-word bit: operation has been cancelled.
pub const FLAG_CANCELLED: u32 = 1 << 2;
/// Response kind: host acknowledged the written chunk.
pub const FLAG_ACK: u32 = 1 << 3;

/// Bits of the response word that carry the response kind.
pub const RESPONSE_KIND_MASK: u32 = FLAG_READY | FLAG_ERROR | FLAG_ACK;
/// Bit position where the echoed request number starts.
pub const RESPONSE_SEQ_SHIFT: u32 = 4;
/// Request numbers wrap inside the 28 bits the response word can echo.
pub const REQUEST_SEQ_MASK: u32 = u32::MAX >> RESPONSE_SEQ_SHIFT;

// ── Read channel byte layout ───────────────────────────────────────
//
// Offset  Field              Size   Purpose
// 0       request_offset     8      byte offset engine wants to read
// 8       request_count      8      byte count engine wants
// 16      response_length    8      bytes actually returned by host
// 24      response           4      Dart-written word: kind + echoed request number
// 28      cancel             4      Rust-written word: FLAG_CANCELLED
// 32      sync_ptr           8      *mut SyncPair (Rust-side only, Dart never reads)
// 40      request            4      Rust-written word: request number
// 44      (reserved)         116
// 160     data               64KB   response byte payload

/// Byte layout constants for the read (source) channel.
pub mod read_channel {
    /// Maximum payload bytes per read request.
    pub const DATA_CAPACITY: usize = crate::host::constants::READ_BUF_CAPACITY;

    /// Offset of the requested byte position (i64).
    pub const OFFSET_REQUEST_OFFSET: usize = 0;
    /// Offset of the requested byte count (i64).
    pub const OFFSET_REQUEST_COUNT: usize = 8;
    /// Offset of the actual response length (i64).
    pub const OFFSET_RESPONSE_LENGTH: usize = 16;
    /// Offset of the response word (u32), written only by Dart.
    pub const OFFSET_RESPONSE: usize = 24;
    /// Offset of the cancel word (u32), written only by Rust.
    pub const OFFSET_CANCEL: usize = 28;
    /// Offset of the SyncPair raw pointer (usize).
    pub const OFFSET_SYNC_PTR: usize = 32;
    /// Offset of the request-number word (u32), written only by Rust.
    pub const OFFSET_REQUEST_SEQ: usize = 40;
    /// Offset where the payload data begins.
    pub const OFFSET_DATA: usize = 160;

    /// Total buffer size in bytes.
    pub const TOTAL_SIZE: usize = OFFSET_DATA + DATA_CAPACITY;
}

// ── Write channel byte layout ──────────────────────────────────────
//
// Offset  Field              Size   Purpose
// 0       chunk_length       8      bytes in this output chunk
// 8       response           4      Dart-written word: kind + echoed request number
// 12      cancel             4      Rust-written word: FLAG_CANCELLED
// 16      sync_ptr           8      *mut SyncPair (Rust-side only, Dart never reads)
// 24      request            4      Rust-written word: request number
// 28      (reserved)         116
// 144     data               256KB  output byte payload

/// Byte layout constants for the write (output) channel.
pub mod write_channel {
    /// Maximum payload bytes per write chunk.
    pub const DATA_CAPACITY: usize = crate::host::constants::WRITE_BUF_CAPACITY;

    /// Offset of the chunk length (i64).
    pub const OFFSET_CHUNK_LENGTH: usize = 0;
    /// Offset of the response word (u32), written only by Dart.
    pub const OFFSET_RESPONSE: usize = 8;
    /// Offset of the cancel word (u32), written only by Rust.
    pub const OFFSET_CANCEL: usize = 12;
    /// Offset of the SyncPair raw pointer (usize).
    pub const OFFSET_SYNC_PTR: usize = 16;
    /// Offset of the request-number word (u32), written only by Rust.
    pub const OFFSET_REQUEST_SEQ: usize = 24;
    /// Offset where the payload data begins.
    pub const OFFSET_DATA: usize = 144;

    /// Total buffer size in bytes.
    pub const TOTAL_SIZE: usize = OFFSET_DATA + DATA_CAPACITY;
}

// ── Accessor helpers (safe wrappers around pointer arithmetic) ─────

/// # Safety
/// `base` must point to a buffer at least `offset + 8` bytes long.
#[inline]
pub unsafe fn read_i64(base: *const u8, offset: usize) -> i64 {
    (base.add(offset) as *const i64).read()
}

/// # Safety
/// `base` must point to a buffer at least `offset + 8` bytes long.
#[inline]
pub unsafe fn write_i64(base: *mut u8, offset: usize, value: i64) {
    (base.add(offset) as *mut i64).write(value);
}

/// # Safety
/// `base` must point to a buffer with an AtomicU32 at `offset`.
#[inline]
pub unsafe fn flags_ref(base: *const u8, offset: usize) -> &'static AtomicU32 {
    &*(base.add(offset) as *const AtomicU32)
}

/// # Safety
/// `base` must have data starting at `offset`.
#[inline]
pub unsafe fn data_ptr(base: *mut u8, offset: usize) -> *mut u8 {
    base.add(offset)
}

#[inline]
/// Load the response word (written only by Dart) with Acquire, so the
/// payload Dart wrote before it is visible once the word has changed.
pub fn load_response(base: *const u8, response_offset: usize) -> u32 {
    unsafe { flags_ref(base, response_offset).load(Ordering::Acquire) }
}

#[inline]
/// Load the cancel word (written only by Rust).
pub fn load_cancel(base: *const u8, cancel_offset: usize) -> u32 {
    unsafe { flags_ref(base, cancel_offset).load(Ordering::Acquire) }
}

#[inline]
/// Check whether the channel's cancel word holds FLAG_CANCELLED.
pub fn is_cancelled(base: *const u8, cancel_offset: usize) -> bool {
    load_cancel(base, cancel_offset) & FLAG_CANCELLED != 0
}

#[inline]
/// Set FLAG_CANCELLED in the cancel word.
///
/// Cancellation is STICKY on a channel: once set it survives every
/// request cycle, so a cancel can never be erased by a racing request.
/// Only [`clear_cancelled`] revives a channel.
pub fn set_cancelled(base: *mut u8, cancel_offset: usize) {
    unsafe { flags_ref(base, cancel_offset).fetch_or(FLAG_CANCELLED, Ordering::Release) };
}

#[inline]
/// Revive a held channel collaterally flagged by a finished job's cancel.
/// Called only by the lane thread, strictly between the moments a cancel
/// can name the channel (see `ChannelRegistry::register`).
pub fn clear_cancelled(base: *mut u8, cancel_offset: usize) {
    unsafe { flags_ref(base, cancel_offset).fetch_and(!FLAG_CANCELLED, Ordering::Release) };
}

#[inline]
/// Load the request number (written only by Rust).
pub fn load_request(base: *const u8, request_offset: usize) -> u32 {
    unsafe { flags_ref(base, request_offset).load(Ordering::Acquire) }
}

/// Number a new request: bump the channel's request word and return the
/// new number. Only the lane thread posts requests on a channel, one at a
/// time, so a load-then-store is the whole story. Zero is never used: it
/// is what a fresh response word reads as.
pub fn next_request(base: *mut u8, request_offset: usize) -> u32 {
    let mut next = load_request(base, request_offset).wrapping_add(1) & REQUEST_SEQ_MASK;
    if next == 0 {
        next = 1;
    }
    unsafe { flags_ref(base, request_offset).store(next, Ordering::Release) };
    next
}

#[cfg(test)]
/// Answer request number `request` the way the Dart host does: publish the
/// response kind and the echoed request number in one store.
pub fn host_respond_to(base: *mut u8, response_offset: usize, request: u32, kind: u32) {
    let word = (request << RESPONSE_SEQ_SHIFT) | kind;
    unsafe { flags_ref(base, response_offset).store(word, Ordering::Release) };
}

#[cfg(test)]
/// Answer the latest request on the channel.
pub fn host_respond(base: *mut u8, request_offset: usize, response_offset: usize, kind: u32) {
    host_respond_to(base, response_offset, load_request(base, request_offset), kind);
}

// ── SyncPair — heap-allocated, pointer stored in buffer ────────────

/// Mutex+Condvar pair for cross-thread signaling.
pub struct SyncPair {
    /// Mutex protecting the condvar wait.
    pub mutex: Mutex<()>,
    /// Condvar signaled when flags change.
    pub condvar: Condvar,
}

/// Write the SyncPair raw pointer into the buffer at `ptr_offset`.
#[inline]
unsafe fn write_sync_ptr(base: *mut u8, ptr_offset: usize, ptr: *mut SyncPair) {
    (base.add(ptr_offset) as *mut usize).write(ptr as usize);
}

/// Read the SyncPair raw pointer from the buffer at `ptr_offset`.
#[inline]
unsafe fn read_sync_ptr(base: *const u8, ptr_offset: usize) -> *mut SyncPair {
    (base.add(ptr_offset) as *const usize).read() as *mut SyncPair
}

/// Get a reference to the SyncPair stored in the buffer.
///
/// # Safety
/// Buffer must have been initialized via `init_sync`.
#[inline]
pub unsafe fn get_sync(base: *const u8, ptr_offset: usize) -> &'static SyncPair {
    &*read_sync_ptr(base, ptr_offset)
}

/// Initialize sync pair for a buffer. Call once before any use.
///
/// # Safety
/// `base` must point to a valid buffer with space at `ptr_offset`.
pub unsafe fn init_sync(base: *mut u8, ptr_offset: usize) {
    let pair = Box::new(SyncPair {
        mutex: Mutex::new(()),
        condvar: Condvar::new(),
    });
    write_sync_ptr(base, ptr_offset, Box::into_raw(pair));
}

/// Destroy sync pair. Call once after all use is done.
///
/// # Safety
/// `base` must have been initialized via `init_sync`.
pub unsafe fn destroy_sync(base: *mut u8, ptr_offset: usize) {
    let ptr = read_sync_ptr(base, ptr_offset);
    if !ptr.is_null() {
        let _ = Box::from_raw(ptr);
        write_sync_ptr(base, ptr_offset, std::ptr::null_mut());
    }
}

/// Signal the condvar. Called from Dart via FFI after fulfilling a request.
///
/// MUST lock the mutex before signaling — this prevents lost wakeups
/// when the signal fires between the waiter's flag-check and park.
///
/// # Safety
/// `base` must have been initialized via `init_sync`.
pub unsafe fn notify(base: *mut u8, ptr_offset: usize) {
    let pair = &*read_sync_ptr(base, ptr_offset);
    let _guard = pair.mutex.lock().unwrap();
    pair.condvar.notify_one();
}

/// Wait for the host's answer to the request just posted, or cancellation.
///
/// `request` is the number [`next_request`] gave the request just posted;
/// the host answers by echoing it in the response word, and any other
/// number (the late answer of an earlier, cancelled request) is ignored.
/// Returns the answer's kind bits (`FLAG_READY` / `FLAG_ERROR` /
/// `FLAG_ACK`), plus `FLAG_CANCELLED` when the channel's cancel word is set.
///
/// Caller MUST hold the mutex (via `get_sync().mutex.lock()`) before
/// calling this — the mutex must be held across the entire
/// setup → wait → response cycle to prevent lost wakeups.
///
/// There is deliberately NO timeout. The only exits are the answer
/// arriving or the token being cancelled — behavior never depends on
/// how slow the device or the host is. The periodic wake below is a
/// belt-and-suspenders token re-check plus a debug-build diagnostic
/// for long parks; it decides nothing. Every canceller also signals
/// the condvar (after setting its flag, under this mutex), so
/// cancellation latency does not depend on the heartbeat either.
pub fn wait_for_response(
    pair: &SyncPair,
    mut guard: MutexGuard<'_, ()>,
    buf: *const u8,
    response_offset: usize,
    cancel_offset: usize,
    request: u32,
    token: &CancelFlags,
) -> Result<u32, std::io::Error> {
    const HEARTBEAT: Duration = Duration::from_secs(30);

    loop {
        if token.is_cancelled() {
            return Err(crate::host::native::cancel::cancelled());
        }

        let response = load_response(buf, response_offset);
        let cancelled = is_cancelled(buf, cancel_offset);
        let answered_here = response >> RESPONSE_SEQ_SHIFT == request;
        if answered_here || cancelled {
            let answered = if answered_here { response & RESPONSE_KIND_MASK } else { 0 };
            return Ok(answered | if cancelled { FLAG_CANCELLED } else { 0 });
        }

        let (new_guard, timeout_result) = pair.condvar.wait_timeout(guard, HEARTBEAT).unwrap();
        guard = new_guard;

        #[cfg(debug_assertions)]
        if timeout_result.timed_out() {
            eprintln!(
                "[pdf_oxide] I/O wait parked > {}s (response_offset={response_offset}) — \
                 host slow or stalled; still waiting",
                HEARTBEAT.as_secs()
            );
        }
        #[cfg(not(debug_assertions))]
        let _ = timeout_result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_channel_layout() {
        assert_eq!(read_channel::OFFSET_RESPONSE, 24);
        assert_eq!(read_channel::OFFSET_CANCEL, 28);
        assert_eq!(read_channel::OFFSET_SYNC_PTR, 32);
        assert_eq!(read_channel::OFFSET_REQUEST_SEQ, 40);
        assert_eq!(read_channel::OFFSET_DATA, 160);
        assert_eq!(read_channel::TOTAL_SIZE, 160 + read_channel::DATA_CAPACITY);
    }

    #[test]
    fn write_channel_layout() {
        assert_eq!(write_channel::OFFSET_RESPONSE, 8);
        assert_eq!(write_channel::OFFSET_CANCEL, 12);
        assert_eq!(write_channel::OFFSET_SYNC_PTR, 16);
        assert_eq!(write_channel::OFFSET_REQUEST_SEQ, 24);
        assert_eq!(write_channel::OFFSET_DATA, 144);
        assert_eq!(write_channel::TOTAL_SIZE, 144 + write_channel::DATA_CAPACITY);
    }

    #[test]
    fn the_three_words_never_share_bytes() {
        let mut buf = vec![0u8; 64];
        let base = buf.as_mut_ptr();

        let request = next_request(base, 8);
        set_cancelled(base, 4);
        host_respond(base, 8, 0, FLAG_READY);
        assert!(is_cancelled(base, 4));
        assert_eq!(load_response(base, 0) & RESPONSE_KIND_MASK, FLAG_READY);
        assert_eq!(load_request(base, 8), request);

        clear_cancelled(base, 4);
        assert!(!is_cancelled(base, 4));
        assert_eq!(load_response(base, 0) & RESPONSE_KIND_MASK, FLAG_READY);
        assert_eq!(load_request(base, 8), request);
    }

    #[test]
    fn cancelled_flag_is_sticky_across_requests_and_responses() {
        let mut buf = vec![0u8; 64];
        let base = buf.as_mut_ptr();

        set_cancelled(base, 4);
        next_request(base, 8);
        host_respond(base, 8, 0, FLAG_READY);
        next_request(base, 8);
        host_respond(base, 8, 0, FLAG_ACK);
        assert!(is_cancelled(base, 4));
    }

    #[test]
    fn every_request_gets_a_new_nonzero_number() {
        let mut buf = vec![0u8; 64];
        let base = buf.as_mut_ptr();

        let mut last = 0;
        for _ in 0..3 {
            let now = next_request(base, 8);
            assert_ne!(now, 0, "zero is what a fresh response word reads as");
            assert_ne!(now, last);
            last = now;
        }
    }

    #[test]
    fn request_numbers_wrap_inside_what_the_response_word_can_echo() {
        let mut buf = vec![0u8; 64];
        let base = buf.as_mut_ptr();
        unsafe { flags_ref(base, 8).store(REQUEST_SEQ_MASK, Ordering::Release) };

        let wrapped = next_request(base, 8);
        assert_eq!(wrapped, 1, "zero is skipped on the wrap");
        host_respond(base, 8, 0, FLAG_READY);
        assert_eq!(load_response(base, 0) >> RESPONSE_SEQ_SHIFT, wrapped);
    }

    #[test]
    fn i64_round_trip() {
        let mut buf = vec![0u8; 32];
        let base = buf.as_mut_ptr();
        unsafe {
            write_i64(base, 0, 42);
            write_i64(base, 8, -1);
            write_i64(base, 16, i64::MAX);
            assert_eq!(read_i64(base, 0), 42);
            assert_eq!(read_i64(base, 8), -1);
            assert_eq!(read_i64(base, 16), i64::MAX);
        }
    }

    #[test]
    fn sync_lifecycle() {
        let mut buf = vec![0u8; 256];
        let ptr_offset = 32;
        unsafe {
            init_sync(buf.as_mut_ptr(), ptr_offset);
            // notify should not panic
            notify(buf.as_mut_ptr(), ptr_offset);
            destroy_sync(buf.as_mut_ptr(), ptr_offset);
        }
    }

    #[test]
    fn wait_returns_at_once_for_the_answer_to_its_request() {
        let mut buf = vec![0u8; 256];
        let ptr_offset = 32;
        unsafe {
            init_sync(buf.as_mut_ptr(), ptr_offset);
            let request = next_request(buf.as_mut_ptr(), 40);
            host_respond(buf.as_mut_ptr(), 40, 24, FLAG_READY);
            let pair = get_sync(buf.as_ptr(), ptr_offset);
            let guard = pair.mutex.lock().unwrap();
            let token = CancelFlags::unconnected();
            let result = wait_for_response(pair, guard, buf.as_ptr(), 24, 28, request, &token);
            assert_eq!(result.unwrap(), FLAG_READY);
            destroy_sync(buf.as_mut_ptr(), ptr_offset);
        }
    }

    #[test]
    fn wait_ignores_the_late_answer_of_an_earlier_request() {
        // Request 1 was cancelled and its slow answer lands while request 2
        // is posted. Waiting for 2 must not take it, or the engine would
        // read request 1's bytes as its own.
        let mut buf = vec![0u8; 256];
        let ptr_offset = 32;
        unsafe {
            init_sync(buf.as_mut_ptr(), ptr_offset);
            let first = next_request(buf.as_mut_ptr(), 40);
            let second = next_request(buf.as_mut_ptr(), 40);
            assert_ne!(first, second);

            let base = buf.as_mut_ptr() as usize;
            let host = std::thread::spawn(move || {
                let base = base as *mut u8;
                std::thread::sleep(Duration::from_millis(30));
                let pair = get_sync(base, 32);
                {
                    let _guard = pair.mutex.lock().unwrap();
                    host_respond_to(base, 24, first, FLAG_ERROR);
                    pair.condvar.notify_all();
                }
                std::thread::sleep(Duration::from_millis(30));
                let _guard = pair.mutex.lock().unwrap();
                host_respond_to(base, 24, second, FLAG_READY);
                pair.condvar.notify_all();
            });

            let started = std::time::Instant::now();
            let pair = get_sync(buf.as_ptr(), ptr_offset);
            let guard = pair.mutex.lock().unwrap();
            let token = CancelFlags::unconnected();
            let result = wait_for_response(pair, guard, buf.as_ptr(), 24, 28, second, &token);
            assert_eq!(result.unwrap(), FLAG_READY, "took the earlier request's answer");
            assert!(started.elapsed() >= Duration::from_millis(50), "returned on the stale answer");

            host.join().unwrap();
            destroy_sync(buf.as_mut_ptr(), ptr_offset);
        }
    }

    #[test]
    fn wait_pre_cancelled_token_returns_cancelled_error() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let mut buf = vec![0u8; 256];
        let ptr_offset = 32;
        unsafe {
            init_sync(buf.as_mut_ptr(), ptr_offset);
            let pair = get_sync(buf.as_ptr(), ptr_offset);
            let guard = pair.mutex.lock().unwrap();
            let lane = Arc::new(AtomicBool::new(true));
            let token = CancelFlags::new(lane, Arc::new(AtomicBool::new(false)));
            let result = wait_for_response(pair, guard, buf.as_ptr(), 24, 28, 1, &token);
            let err = result.unwrap_err();
            // Must NOT be Interrupted — std combinators retry that kind.
            assert_ne!(err.kind(), std::io::ErrorKind::Interrupted);
            assert!(err.to_string().contains("cancelled"));
            destroy_sync(buf.as_mut_ptr(), ptr_offset);
        }
    }

    #[test]
    fn wait_woken_by_cancel_flag_and_signal() {
        // The canceller protocol: set FLAG_CANCELLED under the pair
        // mutex, then notify. A parked waiter must wake and return
        // the flags (the caller maps FLAG_CANCELLED to the
        // non-retryable cancelled error).
        let mut buf = vec![0u8; 256];
        let ptr_offset = 32;
        unsafe {
            init_sync(buf.as_mut_ptr(), ptr_offset);

            let base = buf.as_mut_ptr() as usize;
            let canceller = std::thread::spawn(move || {
                let base = base as *mut u8;
                std::thread::sleep(Duration::from_millis(50));
                let pair = get_sync(base, 32);
                let _guard = pair.mutex.lock().unwrap();
                set_cancelled(base, 28);
                pair.condvar.notify_all();
            });

            let pair = get_sync(buf.as_ptr(), ptr_offset);
            let guard = pair.mutex.lock().unwrap();
            let token = CancelFlags::unconnected();
            let result = wait_for_response(pair, guard, buf.as_ptr(), 24, 28, 1, &token);
            assert!(result.unwrap() & FLAG_CANCELLED != 0);

            canceller.join().unwrap();
            destroy_sync(buf.as_mut_ptr(), ptr_offset);
        }
    }
}
