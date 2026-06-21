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

// ── Host I/O status codes (web lane bodies return these from
//    host_read_at / host_write_chunk; Dart mirrors them in
//    lane_protocol.dart) ─────────────────────────────────────────

/// Host-side I/O failure (read error, missing source, dead sink).
pub const HOST_IO_ERROR: i32 = -1;

/// Host-side cancellation (job cancel or instance dispose). Mapped
/// to the non-retryable cancelled error — never to a plain failure.
pub const HOST_IO_CANCELLED: i32 = -2;
