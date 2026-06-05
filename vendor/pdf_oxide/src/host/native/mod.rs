//! Native platform infrastructure for the pdf_manipulator bridge.
//!
//! This module is part of the pdf_manipulator host layer (NOT upstream).
//! Provides threading, arena allocation, and condvar-based I/O for the
//! native FFI path. The WASM path does not use this module.
//!
//! - `arena`           — per-operation bumpalo allocator
//! - `callback_reader` — Read+Seek via condvar (O(1)-memory source I/O)
//! - `callback_writer` — Write via condvar (O(1)-memory output I/O)
//! - `shared_buffer`   — shared memory byte layout (Rust ↔ Dart)
//! - `thread_pool`     — fixed-size pool with cancellation

pub mod arena;
pub mod callback_reader;
pub mod callback_writer;
pub mod shared_buffer;
pub mod thread_pool;
