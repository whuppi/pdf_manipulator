//! WASM platform infrastructure for the pdf_manipulator bridge.
//!
//! This module is part of the pdf_manipulator host layer (NOT upstream).
//! Provides JS-callback-based I/O for the WASM path. The native path
//! uses `host::native` instead (condvar-based I/O).
//!
//! - `js_reader` — Read+Seek via JS callback (O(1)-memory source I/O)
//! - `js_writer` — Write+Seek via JS callback (O(1)-memory output I/O)

pub mod js_reader;
pub mod js_writer;
