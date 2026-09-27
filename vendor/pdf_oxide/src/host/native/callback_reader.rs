//! CallbackReader — O(1)-memory source I/O via condvar.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Implements `Read + Seek` for the PDF engine. The reader runs on a
//! lane thread. The host (Dart main isolate) fills read requests via
//! shared memory. Communication: std::sync Mutex+Condvar (cross-platform).
//! The lane thread sleeps (zero CPU) while waiting for bytes.
//!
//! O(1)-memory guarantee: at most one shared-buffer chunk (64KB) is
//! in flight at any time. The source file is never buffered in memory.
//!
//! Lifecycle:
//!   1. Lane thread locks the mutex
//!   2. Lane thread writes (offset, count) to shared buffer, clears flags
//!   3. Lane thread calls notify_fn (wakes Dart listener)
//!   4. Lane thread blocks on condvar (atomically unlocks mutex + parks)
//!   5. Dart reads from DataSource, writes bytes to shared buffer
//!   6. Dart sets FLAG_READY, locks mutex, signals condvar, unlocks mutex
//!   7. Lane thread wakes (mutex re-locked), copies bytes, unlocks, returns
//!
//! Cancellation: FLAG_CANCELLED is STICKY on the channel — each
//! request clears only the response bits, so a cancel can never be
//! erased by a racing request cycle. The token is checked before each
//! request and re-checked (with the sticky flag) before notifying.
//! Because the killer takes this buffer's pair mutex before flagging,
//! a kill can never land between the re-check and the notify call —
//! so once a kill returns, this reader will never invoke `notify_fn`
//! again.

use crate::host::native::cancel::CancelToken;
use crate::host::native::shared_buffer::{self as sb, read_channel as rc};
use std::io::{self, Read, Seek, SeekFrom};

/// Read+Seek implementation backed by shared-memory I/O with the host.
pub struct CallbackReader {
    buf: *mut u8,
    notify_fn: unsafe extern "C" fn(),
    token: CancelToken,
    position: u64,
    length: u64,
}

unsafe impl Send for CallbackReader {}
unsafe impl Sync for CallbackReader {}

impl CallbackReader {
    /// # Safety
    ///
    /// - `buf` must point to a read-channel buffer (`rc::TOTAL_SIZE` bytes)
    ///   with sync initialized via `sb::init_sync`.
    /// - `notify_fn` must be safe to call from any thread.
    /// - The buffer must outlive this reader.
    pub unsafe fn new(
        buf: *mut u8,
        notify_fn: unsafe extern "C" fn(),
        length: u64,
        token: CancelToken,
    ) -> Self {
        Self { buf, notify_fn, token, position: 0, length }
    }

    fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
            || sb::has_flag(self.buf, rc::OFFSET_FLAGS, sb::FLAG_CANCELLED)
    }
}

impl Read for CallbackReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.length {
            return Ok(0);
        }
        if self.is_cancelled() {
            return Err(crate::host::native::cancel::cancelled());
        }

        let remaining = self.length - self.position;
        let to_read = out.len()
            .min(remaining as usize)
            .min(rc::DATA_CAPACITY);

        let pair = unsafe { sb::get_sync(self.buf, rc::OFFSET_SYNC_PTR) };
        let guard = pair.mutex.lock().unwrap();

        unsafe {
            sb::write_i64(self.buf, rc::OFFSET_REQUEST_OFFSET, self.position as i64);
            sb::write_i64(self.buf, rc::OFFSET_REQUEST_COUNT, to_read as i64);
            sb::clear_response_flags(self.buf, rc::OFFSET_FLAGS);
        }

        // Re-check after clearing response bits: FLAG_CANCELLED is
        // sticky, so both a token kill and a buffer-flag cancel are
        // caught here — before notify_fn could ring a dead callback.
        if self.is_cancelled() {
            return Err(crate::host::native::cancel::cancelled());
        }

        unsafe { (self.notify_fn)(); }

        let flags = sb::wait_for_flags(
            pair,
            guard,
            self.buf,
            rc::OFFSET_FLAGS,
            sb::FLAG_READY | sb::FLAG_ERROR | sb::FLAG_CANCELLED,
            &self.token,
        )?;

        if flags & sb::FLAG_READY != 0 {
            let n = unsafe { sb::read_i64(self.buf, rc::OFFSET_RESPONSE_LENGTH) as usize };
            unsafe {
                let src = sb::data_ptr(self.buf, rc::OFFSET_DATA);
                std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), n);
            }
            self.position += n as u64;
            return Ok(n);
        }

        if flags & sb::FLAG_ERROR != 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "host read failed"));
        }

        Err(crate::host::native::cancel::cancelled())
    }
}

impl Seek for CallbackReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.length as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek to negative position",
            ));
        }
        self.position = new_pos as u64;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    unsafe extern "C" fn noop_notify() {}

    #[test]
    fn seek_positions() {
        let mut buf = vec![0u8; rc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }

        let mut reader = unsafe {
            CallbackReader::new(buf.as_mut_ptr(), noop_notify, 1000, CancelToken::unconnected())
        };

        assert_eq!(reader.seek(SeekFrom::Start(500)).unwrap(), 500);
        assert_eq!(reader.seek(SeekFrom::Current(100)).unwrap(), 600);
        assert_eq!(reader.seek(SeekFrom::End(-100)).unwrap(), 900);
        assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap(), 0);

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }
    }

    #[test]
    fn read_at_eof() {
        let mut buf = vec![0u8; rc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }

        let mut reader = unsafe {
            CallbackReader::new(buf.as_mut_ptr(), noop_notify, 0, CancelToken::unconnected())
        };
        let mut out = [0u8; 16];
        assert_eq!(reader.read(&mut out).unwrap(), 0);

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }
    }

    #[test]
    fn read_with_cancelled_token_returns_non_retryable_error() {
        let mut buf = vec![0u8; rc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }

        let job = Arc::new(AtomicBool::new(true));
        let token = CancelToken::new(Arc::new(AtomicBool::new(false)), job);
        let mut reader = unsafe {
            CallbackReader::new(buf.as_mut_ptr(), noop_notify, 1000, token)
        };
        let mut out = [0u8; 16];
        let err = reader.read(&mut out).unwrap_err();
        // Must NOT be Interrupted — std combinators retry that kind.
        assert_ne!(err.kind(), io::ErrorKind::Interrupted);
        assert!(err.to_string().contains("cancelled"));

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }
    }

    #[test]
    fn read_with_cancelled_buffer_flag_returns_non_retryable_error() {
        let mut buf = vec![0u8; rc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }
        sb::set_flag_bits(buf.as_mut_ptr(), rc::OFFSET_FLAGS, sb::FLAG_CANCELLED);

        let mut reader = unsafe {
            CallbackReader::new(buf.as_mut_ptr(), noop_notify, 1000, CancelToken::unconnected())
        };
        let mut out = [0u8; 16];
        let err = reader.read(&mut out).unwrap_err();
        assert_ne!(err.kind(), io::ErrorKind::Interrupted);
        assert!(err.to_string().contains("cancelled"));

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }
    }
}
