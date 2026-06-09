//! Shared memory layout between Rust pool threads and the Dart isolate.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Both sides (Rust + Dart) agree on exact byte offsets. Rust defines
//! the layout here. Dart mirrors it in `shared_buffer.dart`. The buffer
//! is allocated by Dart via `calloc` and passed to Rust as a raw pointer.
//!
//! Two channel types:
//! - Read channel: engine requests source bytes from the host.
//! - Write channel: engine sends output chunks to the host.
//!
//! Synchronization: a heap-allocated Mutex+Condvar pair, raw pointer
//! stored in the buffer's sync_ptr slot (Dart never reads it).
//! Cross-platform: Linux, macOS, Windows, Android, iOS.
//! The pool thread sleeps (zero CPU) while waiting. The Dart isolate
//! signals via an FFI call that locks the mutex before notify_one().

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

// ── Flag bits (same values on both Rust and Dart sides) ────────────

/// Flag bit: host has filled the response data.
pub const FLAG_READY: u32 = 1 << 0;
/// Flag bit: host encountered an error.
pub const FLAG_ERROR: u32 = 1 << 1;
/// Flag bit: operation has been cancelled.
pub const FLAG_CANCELLED: u32 = 1 << 2;
/// Flag bit: host acknowledged the written chunk.
pub const FLAG_ACK: u32 = 1 << 3;

// ── Read channel byte layout ───────────────────────────────────────
//
// Offset  Field              Size   Purpose
// 0       request_offset     8      byte offset engine wants to read
// 8       request_count      8      byte count engine wants
// 16      response_length    8      bytes actually returned by host
// 24      flags              4      atomic flag bits
// 28      (padding)          4
// 32      sync_ptr           8      *mut SyncPair (Rust-side only, Dart never reads)
// 40      (reserved)         120
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
    /// Offset of the atomic flags word (u32).
    pub const OFFSET_FLAGS: usize = 24;
    /// Offset of the SyncPair raw pointer (usize).
    pub const OFFSET_SYNC_PTR: usize = 32;
    /// Offset where the payload data begins.
    pub const OFFSET_DATA: usize = 160;

    /// Total buffer size in bytes.
    pub const TOTAL_SIZE: usize = OFFSET_DATA + DATA_CAPACITY;
}

// ── Write channel byte layout ──────────────────────────────────────
//
// Offset  Field              Size   Purpose
// 0       chunk_length       8      bytes in this output chunk
// 8       flags              4      atomic flag bits
// 12      (padding)          4
// 16      sync_ptr           8      *mut SyncPair (Rust-side only, Dart never reads)
// 24      (reserved)         120
// 144     data               256KB  output byte payload

/// Byte layout constants for the write (output) channel.
pub mod write_channel {
    /// Maximum payload bytes per write chunk.
    pub const DATA_CAPACITY: usize = crate::host::constants::WRITE_BUF_CAPACITY;

    /// Offset of the chunk length (i64).
    pub const OFFSET_CHUNK_LENGTH: usize = 0;
    /// Offset of the atomic flags word (u32).
    pub const OFFSET_FLAGS: usize = 8;
    /// Offset of the SyncPair raw pointer (usize).
    pub const OFFSET_SYNC_PTR: usize = 16;
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
/// Atomically load the flags word from the buffer.
pub fn load_flags(base: *const u8, flags_offset: usize) -> u32 {
    unsafe { flags_ref(base, flags_offset).load(Ordering::Acquire) }
}

#[inline]
/// Atomically store the flags word into the buffer.
pub fn store_flags(base: *mut u8, flags_offset: usize, value: u32) {
    unsafe { flags_ref(base, flags_offset).store(value, Ordering::Release) }
}

#[inline]
/// Atomically OR the given bits into the flags word.
pub fn set_flag_bits(base: *mut u8, flags_offset: usize, bits: u32) {
    unsafe { flags_ref(base, flags_offset).fetch_or(bits, Ordering::Release) };
}

#[inline]
/// Clear all flags to zero.
pub fn clear_flags(base: *mut u8, flags_offset: usize) {
    store_flags(base, flags_offset, 0);
}

#[inline]
/// Check whether a specific flag bit is set.
pub fn has_flag(base: *const u8, flags_offset: usize, bit: u32) -> bool {
    load_flags(base, flags_offset) & bit != 0
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

/// Wait for any of the specified flag bits, with timeout.
/// Caller MUST hold the mutex (via `get_sync().mutex.lock()`) before
/// calling this — the mutex must be held across the entire
/// setup → wait → response cycle to prevent lost wakeups.
pub fn wait_for_flags(
    pair: &SyncPair,
    mut guard: MutexGuard<'_, ()>,
    buf: *const u8,
    flags_offset: usize,
    target_bits: u32,
    timeout: Duration,
) -> Result<u32, std::io::Error> {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let flags = load_flags(buf, flags_offset);
        if flags & target_bits != 0 {
            return Ok(flags);
        }

        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out"));
        }

        let (new_guard, timeout_result) = pair.condvar.wait_timeout(guard, remaining).unwrap();
        guard = new_guard;

        if timeout_result.timed_out() {
            let flags = load_flags(buf, flags_offset);
            if flags & target_bits != 0 {
                return Ok(flags);
            }
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_channel_layout() {
        assert_eq!(read_channel::OFFSET_DATA, 160);
        assert_eq!(read_channel::TOTAL_SIZE, 160 + read_channel::DATA_CAPACITY);
    }

    #[test]
    fn write_channel_layout() {
        assert_eq!(write_channel::OFFSET_DATA, 144);
        assert_eq!(write_channel::TOTAL_SIZE, 144 + write_channel::DATA_CAPACITY);
    }

    #[test]
    fn flag_operations() {
        let mut buf = vec![0u8; 64];
        let base = buf.as_mut_ptr();

        assert!(!has_flag(base, 0, FLAG_READY));
        set_flag_bits(base, 0, FLAG_READY);
        assert!(has_flag(base, 0, FLAG_READY));
        assert!(!has_flag(base, 0, FLAG_ERROR));

        set_flag_bits(base, 0, FLAG_ERROR);
        assert!(has_flag(base, 0, FLAG_READY));
        assert!(has_flag(base, 0, FLAG_ERROR));

        clear_flags(base, 0);
        assert!(!has_flag(base, 0, FLAG_READY));
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
    fn wait_immediate_flag() {
        let mut buf = vec![0u8; 256];
        let ptr_offset = 32;
        let flags_offset = 24;
        unsafe {
            init_sync(buf.as_mut_ptr(), ptr_offset);
            set_flag_bits(buf.as_mut_ptr(), flags_offset, FLAG_READY);
            let pair = get_sync(buf.as_ptr(), ptr_offset);
            let guard = pair.mutex.lock().unwrap();
            let result = wait_for_flags(
                pair, guard, buf.as_ptr(), flags_offset,
                FLAG_READY | FLAG_ERROR, Duration::from_secs(1),
            );
            assert!(result.is_ok());
            assert!(result.unwrap() & FLAG_READY != 0);
            destroy_sync(buf.as_mut_ptr(), ptr_offset);
        }
    }

    #[test]
    fn wait_timeout() {
        let mut buf = vec![0u8; 256];
        let ptr_offset = 32;
        let flags_offset = 24;
        unsafe {
            init_sync(buf.as_mut_ptr(), ptr_offset);
            let pair = get_sync(buf.as_ptr(), ptr_offset);
            let guard = pair.mutex.lock().unwrap();
            let result = wait_for_flags(
                pair, guard, buf.as_ptr(), flags_offset,
                FLAG_READY, Duration::from_millis(10),
            );
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
            destroy_sync(buf.as_mut_ptr(), ptr_offset);
        }
    }
}
