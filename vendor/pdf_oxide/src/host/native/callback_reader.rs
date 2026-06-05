//! CallbackReader — O(1)-memory source I/O via condvar.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Implements `Read + Seek` for the PDF engine. The reader runs on a
//! Rust pool thread. The host (Dart isolate) fills read requests via
//! shared memory. Communication: std::sync Mutex+Condvar (cross-platform).
//! The pool thread sleeps (zero CPU) while waiting for bytes.
//!
//! O(1)-memory guarantee: at most one shared-buffer chunk (64KB) is
//! in flight at any time. The source file is never buffered in memory.
//!
//! Lifecycle:
//!   1. Pool thread locks the mutex
//!   2. Pool thread writes (offset, count) to shared buffer, clears flags
//!   3. Pool thread calls notify_fn (wakes Dart isolate listener)
//!   4. Pool thread blocks on condvar (atomically unlocks mutex + parks)
//!   5. Dart reads from DataSource, writes bytes to shared buffer
//!   6. Dart sets FLAG_READY, locks mutex, signals condvar, unlocks mutex
//!   7. Pool thread wakes (mutex re-locked), copies bytes, unlocks, returns

use crate::host::native::shared_buffer::{self as sb, read_channel as rc};
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const READ_TIMEOUT_SECS: u64 = 30;

/// Read+Seek implementation backed by shared-memory I/O with the host.
pub struct CallbackReader {
    buf: *mut u8,
    notify_fn: unsafe extern "C" fn(),
    cancel: Option<Arc<AtomicBool>>,
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
        cancel: Option<Arc<AtomicBool>>,
    ) -> Self {
        Self { buf, notify_fn, cancel, position: 0, length }
    }

    fn is_cancelled(&self) -> bool {
        if let Some(ref flag) = self.cancel {
            if flag.load(Ordering::Relaxed) { return true; }
        }
        sb::has_flag(self.buf, rc::OFFSET_FLAGS, sb::FLAG_CANCELLED)
    }
}

impl Read for CallbackReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.length {
            return Ok(0);
        }
        if self.is_cancelled() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
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
            sb::clear_flags(self.buf, rc::OFFSET_FLAGS);
        }

        unsafe { (self.notify_fn)(); }

        let flags = sb::wait_for_flags(
            pair,
            guard,
            self.buf,
            rc::OFFSET_FLAGS,
            sb::FLAG_READY | sb::FLAG_ERROR | sb::FLAG_CANCELLED,
            Duration::from_secs(READ_TIMEOUT_SECS),
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

        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
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

    unsafe extern "C" fn noop_notify() {}

    #[test]
    fn seek_positions() {
        let mut buf = vec![0u8; rc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }

        let mut reader = unsafe {
            CallbackReader::new(buf.as_mut_ptr(), noop_notify, 1000, None)
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
            CallbackReader::new(buf.as_mut_ptr(), noop_notify, 0, None)
        };
        let mut out = [0u8; 16];
        assert_eq!(reader.read(&mut out).unwrap(), 0);

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }
    }

    #[test]
    fn read_cancelled() {
        let mut buf = vec![0u8; rc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }

        let cancel = Arc::new(AtomicBool::new(true));
        let mut reader = unsafe {
            CallbackReader::new(buf.as_mut_ptr(), noop_notify, 1000, Some(cancel))
        };
        let mut out = [0u8; 16];
        assert_eq!(reader.read(&mut out).unwrap_err().kind(), io::ErrorKind::Interrupted);

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), rc::OFFSET_SYNC_PTR); }
    }
}
