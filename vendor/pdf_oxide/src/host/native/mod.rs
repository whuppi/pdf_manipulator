//! Native platform infrastructure for the pdf_manipulator bridge.
//!
//! This module is part of the pdf_manipulator host layer (NOT upstream).
//! Provides the native executor implementation: detached executor threads with
//! mailboxes, and condvar-based O(1)-memory I/O. The WASM path does
//! not use this module (its executor body is the Web Worker itself).
//!
//! - `callback_reader` — Read+Seek via condvar (O(1)-memory source I/O)
//! - `callback_writer` — Write via condvar (O(1)-memory output I/O)
//! - `cancel`          — CancelFlags (executor terminate + per-job cancel)
//! - `executor`        — the executor body: thread + mailbox + owned state
//! - `executor_table`  — key registry, thread budget, FFI surface
//! - `shared_buffer`   — shared memory byte layout (Rust ↔ Dart)

pub mod callback_reader;
pub mod callback_writer;
pub mod cancel;
pub mod executor;
pub mod executor_table;
pub mod shared_buffer;
