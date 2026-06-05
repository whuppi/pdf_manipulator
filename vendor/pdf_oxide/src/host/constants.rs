//! Shared I/O buffer capacities for the pdf_manipulator bridge.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! BufReader wraps the host-provided reader to batch small engine reads
//! into fewer cross-boundary calls. BufWriter wraps the host-provided
//! writer to batch the engine's byte-by-byte serialization output.
//! Both sizes match the shared-buffer data capacities so one buffered
//! flush fills exactly one cross-boundary trip.

/// BufReader capacity (64KB) — one shared-buffer read trip.
pub const READ_BUF_CAPACITY: usize = 64 * 1024;

/// BufWriter capacity (256KB) — one shared-buffer write trip.
pub const WRITE_BUF_CAPACITY: usize = 256 * 1024;
