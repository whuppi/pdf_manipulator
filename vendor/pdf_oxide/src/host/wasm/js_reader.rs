//! JsCallbackReader — O(1)-memory source I/O for WASM.
//!
//! Implements `Read + Seek` by calling `host_read_at` imported from
//! the JS global scope via wasm_bindgen.
//!
//! worker.js sets `self.host_read_at = hostReadAt` before loading WASM.
//! wasm_bindgen resolves the import via global lookup — no bare "env"
//! import specifier that would break in ES module Workers.
//!
//! O(1)-memory guarantee: at most one 64KB chunk in WASM linear memory.
//! The source file is never fully buffered.
//!
//! Three JS-side implementations (transparent to this reader):
//!   Atomics:  host_read_at blocks via Atomics.wait, coordinator fills SAB
//!   JSPI:     V8 suspends WASM stack natively when import returns Promise
//!   OPFS:     host_read_at calls SyncAccessHandle.read (data on disk)

use std::io::{self, Read, Seek, SeekFrom};
use wasm_bindgen::prelude::*;

// Imported from the JS global scope (self.host_read_at in worker.js).
// Uses u32 for buf_ptr because wasm_bindgen doesn't support raw pointers.
// WASM pointers are u32. Cast at the call site.
#[wasm_bindgen]
extern "C" {
    fn host_read_at(source_index: u32, offset: u32, count: u32, buf_ptr: u32) -> i32;
}

/// Source reader backed by the `host_read_at` extern import.
///
/// Each reader carries a `source_index` so the JS side routes
/// readAt to the correct DataSource when multiple sources exist.
pub struct JsCallbackReader {
    source_index: u32,
    length: u64,
    position: u64,
}

impl JsCallbackReader {
    /// Create a reader for the source at `source_index`.
    pub fn new(source_index: u32, length: u64) -> Self {
        Self { source_index, length, position: 0 }
    }
}

impl Read for JsCallbackReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.length {
            return Ok(0);
        }

        let remaining = self.length - self.position;
        let to_read = buf.len().min(remaining as usize).min(crate::host::constants::READ_BUF_CAPACITY);

        let n = host_read_at(
            self.source_index,
            self.position as u32,
            to_read as u32,
            buf.as_mut_ptr() as u32,
        );

        if n < 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "host_read_at failed"));
        }

        let n = n as usize;
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
