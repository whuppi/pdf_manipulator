//! Native platform infrastructure for the pdf_manipulator bridge.
//!
//! This module is part of the pdf_manipulator host layer (NOT upstream).
//! Provides the native lane implementation: detached lane threads with
//! mailboxes, and condvar-based O(1)-memory I/O. The WASM path does
//! not use this module (its lane body is the Web Worker itself).
//!
//! - `callback_reader` — Read+Seek via condvar (O(1)-memory source I/O)
//! - `callback_writer` — Write via condvar (O(1)-memory output I/O)
//! - `cancel`          — CancelToken (lane kill + per-job cancel)
//! - `lane`            — the lane body: thread + mailbox + owned state
//! - `lane_table`      — key registry, thread budget, FFI surface
//! - `shared_buffer`   — shared memory byte layout (Rust ↔ Dart)

pub mod callback_reader;
pub mod callback_writer;
pub mod cancel;
pub mod lane;
pub mod lane_table;
pub mod shared_buffer;
