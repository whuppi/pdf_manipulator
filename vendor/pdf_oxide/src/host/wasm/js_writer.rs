//! JsCallbackWriter — O(1)-memory output I/O for WASM.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Implements `Write` by calling
//! `host_write_chunk` imported from the JS global scope via wasm_bindgen.
//! Byte offsets for xref tables come from a counting wrapper
//! (`CountingWriter`); no seek ever occurs on the output.
//!
//! executor_worker.js sets `self.host_write_chunk = hostWriteChunk` before loading WASM.
//!
//! Small writes accumulate in a fixed internal buffer (256KB, matching
//! the native condvar write channel). A round-trip to JS only happens
//! when the buffer is full or on flush. O(1) memory — the buffer
//! is fixed-size, never grows with PDF size.

use std::io::{self, Write};
use wasm_bindgen::prelude::*;
use crate::host::constants::WRITE_BUF_CAPACITY;

// Imported from the JS global scope (self.host_write_chunk in executor_worker.js).
// Uses u32 for buf_ptr because wasm_bindgen doesn't support raw pointers.
#[wasm_bindgen]
extern "C" {
    fn host_write_chunk(sink_index: u32, buf_ptr: u32, len: u32) -> i32;
}

/// Output writer backed by the `host_write_chunk` extern import.
///
/// Each writer carries a `sink_index` so the JS side routes
/// chunks to the correct DataSink when multiple sinks exist.
pub struct JsCallbackWriter {
    sink_index: u32,
    buffer: Vec<u8>,
}

impl JsCallbackWriter {
    /// Create a writer for the sink at `sink_index`.
    pub fn new(sink_index: u32) -> Self {
        Self {
            sink_index,
            buffer: Vec::with_capacity(WRITE_BUF_CAPACITY),
        }
    }

    fn flush_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let result = host_write_chunk(self.sink_index, self.buffer.as_ptr() as u32, self.buffer.len() as u32);
        if result == crate::host::constants::HOST_IO_CANCELLED {
            // Deliberately NOT ErrorKind::Interrupted — std combinators
            // retry that kind, which would spin forever on a cancel.
            self.buffer.clear();
            return Err(io::Error::other("operation cancelled"));
        }
        if result < 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "host_write_chunk failed"));
        }
        self.buffer.clear();
        Ok(())
    }
}

impl Write for JsCallbackWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut written = 0;
        while written < buf.len() {
            let remaining = WRITE_BUF_CAPACITY - self.buffer.len();
            let to_copy = (buf.len() - written).min(remaining);
            self.buffer.extend_from_slice(&buf[written..written + to_copy]);
            written += to_copy;

            if self.buffer.len() == WRITE_BUF_CAPACITY {
                self.flush_buffer()?;
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buffer()
    }
}

impl crate::host::bridge_api::BufferedSink for JsCallbackWriter {
    fn discard(&mut self) {
        self.buffer.clear();
    }
}

impl Drop for JsCallbackWriter {
    /// Writes nothing, so the drop glue never calls the pausing import:
    /// bytes still buffered are the request door's to flush after an ok op,
    /// or to discard after a failed or cancelled one.
    fn drop(&mut self) {
        debug_assert!(
            self.buffer.is_empty() || std::thread::panicking(),
            "a JsCallbackWriter was dropped with buffered bytes: flush or discard it first"
        );
    }
}
