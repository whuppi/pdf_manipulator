//! CallbackWriter — O(1)-memory output I/O via condvar.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Implements `Write` for the PDF engine. Output chunks flow to the
//! host (Dart main isolate) via shared memory + condvar. The lane
//! thread blocks until the host acknowledges each chunk.
//!
//! The engine tracks byte offsets through a counting wrapper
//! (`CountingWriter`); no seek ever occurs on the output. Output is
//! sequential.
//!
//! O(1)-memory guarantee: at most one shared-buffer chunk (256KB) is
//! in flight at any time. The output is never buffered in memory.
//!
//! Cancellation mirrors CallbackReader: FLAG_CANCELLED is sticky
//! (it lives in the channel's Rust-written cancel word, which a flush
//! never touches), and the token + flag
//! are re-checked before notifying, under the pair mutex the terminating caller
//! must take first — so once a terminate returns, this writer will never
//! invoke `notify_fn` again.

use crate::host::native::cancel::CancelFlags;
use crate::host::native::shared_buffer::{self as sb, write_channel as wc};
use std::io::{self, Write};

/// Write+Seek implementation backed by shared-memory I/O with the host.
pub struct CallbackWriter {
    buf: *mut u8,
    notify_fn: unsafe extern "C" fn(),
    token: CancelFlags,
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
        token: CancelFlags,
    ) -> Self {
        Self { buf, notify_fn, token, pending: 0 }
    }

    fn is_cancelled(&self) -> bool {
        self.token.is_cancelled() || sb::is_cancelled(self.buf, wc::OFFSET_CANCEL)
    }

    fn flush_pending(&mut self) -> io::Result<()> {
        if self.pending == 0 {
            return Ok(());
        }
        if self.is_cancelled() {
            self.pending = 0;
            return Err(crate::host::native::cancel::cancelled());
        }

        let pair = unsafe { sb::get_sync(self.buf, wc::OFFSET_SYNC_PTR) };
        let guard = pair.mutex.lock().unwrap();

        let request = sb::next_request(self.buf, wc::OFFSET_REQUEST_SEQ);
        unsafe {
            sb::write_i64(self.buf, wc::OFFSET_CHUNK_LENGTH, self.pending as i64);
        }

        // Re-check after the chunk is written — same protocol as
        // the reader: sticky flag + token, before ringing a possibly
        // dead callback.
        if self.is_cancelled() {
            self.pending = 0;
            return Err(crate::host::native::cancel::cancelled());
        }

        unsafe { (self.notify_fn)(); }

        let flags = sb::wait_for_response(
            pair,
            guard,
            self.buf,
            wc::OFFSET_RESPONSE,
            wc::OFFSET_CANCEL,
            request,
            &self.token,
        )?;

        self.pending = 0;

        if flags & sb::FLAG_ACK != 0 {
            return Ok(());
        }

        if flags & sb::FLAG_ERROR != 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "host write failed"));
        }

        Err(crate::host::native::cancel::cancelled())
    }
}

impl Write for CallbackWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        if self.is_cancelled() {
            return Err(crate::host::native::cancel::cancelled());
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

impl crate::host::bridge_api::BufferedSink for CallbackWriter {
    fn discard(&mut self) {
        self.pending = 0;
    }
}

impl Drop for CallbackWriter {
    /// Writes nothing: bytes still buffered are the request door's to flush
    /// after an ok op, or to discard after a failed or cancelled one.
    fn drop(&mut self) {
        debug_assert!(
            self.pending == 0 || std::thread::panicking(),
            "a CallbackWriter was dropped with buffered bytes: flush or discard it first"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    unsafe extern "C" fn noop_notify() {}

    #[test]
    fn cancelled_write_returns_non_retryable_error() {
        let mut buf = vec![0u8; wc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), wc::OFFSET_SYNC_PTR); }

        let job = Arc::new(AtomicBool::new(true));
        let token = CancelFlags::new(Arc::new(AtomicBool::new(false)), job);
        let mut writer = unsafe {
            CallbackWriter::new(buf.as_mut_ptr(), noop_notify, token)
        };
        let err = writer.write(&[1, 2, 3]).unwrap_err();
        // Must NOT be Interrupted — std combinators retry that kind.
        assert_ne!(err.kind(), io::ErrorKind::Interrupted);
        assert!(err.to_string().contains("cancelled"));

        unsafe { sb::destroy_sync(buf.as_mut_ptr(), wc::OFFSET_SYNC_PTR); }
    }

    static CHUNKS: AtomicUsize = AtomicUsize::new(0);
    static CHANNEL: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// The host's acknowledgement of the chunk just announced.
    unsafe extern "C" fn acknowledging_notify() {
        CHUNKS.fetch_add(1, Ordering::SeqCst);
        let buf = CHANNEL.load(Ordering::SeqCst);
        sb::host_respond(buf, wc::OFFSET_REQUEST_SEQ, wc::OFFSET_RESPONSE, sb::FLAG_ACK);
    }

    /// Runs `body` on a writer whose host acknowledges every chunk, and
    /// returns how many chunks the host was sent.
    fn chunks_sent_by(body: impl FnOnce(&mut Option<CallbackWriter>)) -> usize {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let mut buf = vec![0u8; wc::TOTAL_SIZE];
        unsafe { sb::init_sync(buf.as_mut_ptr(), wc::OFFSET_SYNC_PTR); }
        CHANNEL.store(buf.as_mut_ptr(), Ordering::SeqCst);
        CHUNKS.store(0, Ordering::SeqCst);
        let mut writer = Some(unsafe {
            CallbackWriter::new(buf.as_mut_ptr(), acknowledging_notify, CancelFlags::unconnected())
        });
        let outcome = catch_unwind(AssertUnwindSafe(|| body(&mut writer)));
        drop(writer);
        let sent = CHUNKS.load(Ordering::SeqCst);
        unsafe { sb::destroy_sync(buf.as_mut_ptr(), wc::OFFSET_SYNC_PTR); }
        if let Err(panic) = outcome {
            resume_unwind(panic);
        }
        sent
    }

    #[test]
    fn a_flush_sends_the_buffered_bytes_once() {
        let sent = chunks_sent_by(|writer| {
            let w = writer.as_mut().unwrap();
            w.write_all(&[1, 2, 3]).unwrap();
            w.flush().unwrap();
        });
        assert_eq!(sent, 1);
    }

    #[test]
    fn a_discarded_writer_sends_nothing_when_dropped() {
        let sent = chunks_sent_by(|writer| {
            let w = writer.as_mut().unwrap();
            w.write_all(&[1, 2, 3]).unwrap();
            crate::host::bridge_api::BufferedSink::discard(w);
            drop(writer.take());
        });
        assert_eq!(sent, 0, "a drop after a failed or cancelled op writes nothing");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "buffered bytes")]
    fn dropping_buffered_bytes_after_an_ok_op_is_a_bug() {
        chunks_sent_by(|writer| {
            writer.as_mut().unwrap().write_all(&[1, 2, 3]).unwrap();
            drop(writer.take());
        });
    }

    #[test]
    fn empty_write_returns_zero() {
        let mut writer = CallbackWriter {
            buf: std::ptr::null_mut(),
            notify_fn: noop_notify,
            token: CancelFlags::unconnected(),
            pending: 0,
        };
        assert_eq!(writer.write(&[]).unwrap(), 0);
    }
}
