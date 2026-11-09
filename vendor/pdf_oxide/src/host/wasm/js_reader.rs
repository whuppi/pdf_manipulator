//! JsCallbackReader — O(1)-memory source I/O for WASM.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Implements `Read + Seek` by calling `host_read_at` imported from
//! the JS global scope via wasm_bindgen.
//!
//! executor_worker.js sets `self.host_read_at = hostReadAt` before loading WASM.
//! wasm_bindgen resolves the import via global lookup — no bare "env"
//! import specifier that would break in ES module Workers.
//!
//! O(1)-memory guarantee: one 64KB window per reader in WASM linear memory.
//! The source file is never fully buffered. A read inside the last window
//! is a copy; only a miss calls `host_read_at`, for a whole chunk.
//!
//! Three JS-side implementations (transparent to this reader):
//!   Atomics:  host_read_at blocks via Atomics.wait, main thread fills SAB
//!   JSPI:     V8 suspends WASM stack natively when import returns Promise
//!   OPFS:     host_read_at calls SyncAccessHandle.read (data on disk)

use std::io::{self, Read, Seek, SeekFrom};
use wasm_bindgen::prelude::*;

// Imported from the JS global scope (self.host_read_at in executor_worker.js).
// Uses u32 for buf_ptr because wasm_bindgen doesn't support raw pointers.
// WASM pointers are u32. Cast at the call site. The offset is an f64, a JS
// number: exact to 2^53, where a u32 would wrap a read past 4 GiB.
#[wasm_bindgen]
extern "C" {
    fn host_read_at(source_index: u32, offset: f64, count: u32, buf_ptr: u32) -> i32;
}

/// Source reader backed by the `host_read_at` extern import.
///
/// Each reader carries a `source_index` so the JS side routes
/// readAt to the correct DataSource when multiple sources exist.
///
/// The engine reads through a small `BufReader` that drops its buffer on
/// every seek, and a `host_read_at` call is a suspended wasm stack or an
/// `Atomics.wait`. So a miss asks for a whole chunk, kept as the window,
/// and every read inside the window is a copy.
pub struct JsCallbackReader {
    source_index: u32,
    length: u64,
    position: u64,
    window: Vec<u8>,
    window_start: u64,
    window_len: usize,
}

impl JsCallbackReader {
    /// Create a reader for the source at `source_index`.
    pub fn new(source_index: u32, length: u64) -> Self {
        Self {
            source_index,
            length,
            position: 0,
            window: Vec::new(),
            window_start: 0,
            window_len: 0,
        }
    }
}

impl Read for JsCallbackReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.length || buf.is_empty() {
            return Ok(0);
        }

        // A read the host failed ends the op: the engine reads on only to be
        // told the same thing, and never to be given bytes it would take for
        // a source that recovered.
        if let Some(error) = crate::host::bridge_api::source_failure() {
            return Err(error);
        }

        let in_window = self.position >= self.window_start
            && self.position < self.window_start + self.window_len as u64;
        if !in_window {
            if self.window.is_empty() {
                self.window = vec![0; crate::host::constants::READ_BUF_CAPACITY];
            }
            self.window_len = 0;
            let remaining = self.length - self.position;
            let to_read = remaining.min(self.window.len() as u64) as usize;

            let n = host_read_at(
                self.source_index,
                self.position as f64,
                to_read as u32,
                self.window.as_mut_ptr() as u32,
            );

            if n == crate::host::constants::HOST_IO_CANCELLED {
                // Deliberately NOT ErrorKind::Interrupted — std combinators
                // retry that kind, which would spin forever on a cancel.
                return Err(io::Error::other("operation cancelled"));
            }
            if n < 0 {
                return Err(crate::host::bridge_api::fail_source("host_read_at failed"));
            }
            self.window_start = self.position;
            self.window_len = n as usize;
        }

        let at = (self.position - self.window_start) as usize;
        let n = buf.len().min(self.window_len - at);
        buf[..n].copy_from_slice(&self.window[at..at + n]);
        self.position += n as u64;
        Ok(n)
    }
}

impl Seek for JsCallbackReader {
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
