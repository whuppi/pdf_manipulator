//! CallbackWriter — O(1)-memory output I/O via condvar.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Implements `Write` for the PDF engine. Output chunks flow to the
//! host (Dart isolate) via shared memory + condvar. The pool thread
//! blocks until the host acknowledges each chunk.
//!
//! Also implements `Seek` for `stream_position()` only — the engine
//! calls `SeekFrom::Current(0)` to track byte offsets for xref tables.
//! NO backward seeking ever occurs on the output. Output is sequential.
//!
//! O(1)-memory guarantee: at most one shared-buffer chunk (256KB) is
//! in flight at any time. The output is never buffered in memory.

use crate::host::native::shared_buffer::{self as sb, write_channel as wc};
use std::io::{self, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const WRITE_TIMEOUT_SECS: u64 = 30;

/// Write+Seek implementation backed by shared-memory I/O with the host.
pub struct CallbackWriter {
    buf: *mut u8,
    notify_fn: unsafe extern "C" fn(),
    cancel: Option<Arc<AtomicBool>>,
    position: u64,
    pending: usize,
}

unsafe impl Send for CallbackWriter {}
unsafe impl Sync for CallbackWriter {}

impl CallbackWriter {
    /// # Safety
    ///
    /// - `buf` must point to a write-channel buffer (`wc::TOTAL_SIZE` bytes)
    ///   with sync initialized via `sb::init_sync`.
    /// - `notify_fn` must be safe to call from any thread.
    /// - The buffer must outlive this writer.
    pub unsafe fn new(
        buf: *mut u8,
        notify_fn: unsafe extern "C" fn(),
        cancel: Option<Arc<AtomicBool>>,
    ) -> Self {
        Self { buf, notify_fn, cancel, position: 0, pending: 0 }
    }

    fn flush_pending(&mut self) -> io::Result<()> {
        if self.pending == 0 {
            return Ok(());
        }
        if self.is_cancelled() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }

        let pair = unsafe { sb::get_sync(self.buf, wc::OFFSET_SYNC_PTR) };
        let guard = pair.mutex.lock().unwrap();

        unsafe {
            sb::write_i64(self.buf, wc::OFFSET_CHUNK_LENGTH, self.pending as i64);
            sb::clear_flags(self.buf, wc::OFFSET_FLAGS);
            sb::set_flag_bits(self.buf, wc::OFFSET_FLAGS, sb::FLAG_READY);
        }

        unsafe { (self.notify_fn)(); }

        let flags = sb::wait_for_flags(
            pair,
            guard,
            self.buf,
            wc::OFFSET_FLAGS,
            sb::FLAG_ACK | sb::FLAG_ERROR | sb::FLAG_CANCELLED,
            std::time::Duration::from_secs(WRITE_TIMEOUT_SECS),
        )?;

        self.pending = 0;

        if flags & sb::FLAG_ACK != 0 {
            return Ok(());
        }

        if flags & sb::FLAG_ERROR != 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "host write failed"));
        }

        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    }

    fn is_cancelled(&self) -> bool {
        if let Some(ref flag) = self.cancel {
            if flag.load(Ordering::Relaxed) { return true; }
        }
        sb::has_flag(self.buf, wc::OFFSET_FLAGS, sb::FLAG_CANCELLED)
    }
}

impl Write for CallbackWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        if self.is_cancelled() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }

        let mut written = 0;
        while written < data.len() {
            let remaining_capacity = wc::DATA_CAPACITY - self.pending;
            let to_copy = (data.len() - written).min(remaining_capacity);

            unsafe {
                let dst = sb::data_ptr(self.buf, wc::OFFSET_DATA).add(self.pending);
                std::ptr::copy_nonoverlapping(data[written..].as_ptr(), dst, to_copy);
            }
            self.pending += to_copy;
            self.position += to_copy as u64;
            written += to_copy;

            if self.pending == wc::DATA_CAPACITY {
                self.flush_pending()?;
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_pending()
    }
}

impl Drop for CallbackWriter {
    fn drop(&mut self) {
        let _ = self.flush_pending();
    }
}

impl Seek for CallbackWriter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Current(0) => Ok(self.position),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "CallbackWriter only supports stream_position (SeekFrom::Current(0))",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_position_tracks() {
        let mut writer = CallbackWriter {
            buf: std::ptr::null_mut(),
            notify_fn: noop_notify,
            cancel: None,
            position: 42,
            pending: 0,
        };
        assert_eq!(writer.seek(SeekFrom::Current(0)).unwrap(), 42);
    }

    #[test]
    fn backward_seek_rejected() {
        let mut writer = CallbackWriter {
            buf: std::ptr::null_mut(),
            notify_fn: noop_notify,
            cancel: None,
            position: 100,
            pending: 0,
        };
        assert!(writer.seek(SeekFrom::Start(0)).is_err());
        assert!(writer.seek(SeekFrom::Current(-10)).is_err());
        assert!(writer.seek(SeekFrom::End(0)).is_err());
    }

    #[test]
    fn cancelled_write_returns_interrupted() {
        let mut buf = vec![0u8; wc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), wc::OFFSET_SYNC_PTR); }

        let cancel = Arc::new(AtomicBool::new(true));
        let mut writer = unsafe {
            CallbackWriter::new(buf.as_mut_ptr(), noop_notify, Some(cancel))
        };
        let err = writer.write(&[1, 2, 3]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Interrupted);

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), wc::OFFSET_SYNC_PTR); }
    }

    #[test]
    fn empty_write_returns_zero() {
        let mut writer = CallbackWriter {
            buf: std::ptr::null_mut(),
            notify_fn: noop_notify,
            cancel: None,
            position: 0,
            pending: 0,
        };
        assert_eq!(writer.write(&[]).unwrap(), 0);
    }

    unsafe extern "C" fn noop_notify() {}
}
