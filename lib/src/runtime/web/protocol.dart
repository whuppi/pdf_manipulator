// Lease protocol — the single source of truth for every constant the
// web executor script and the Dart side must agree on.
//
// The numeric codes are INJECTED into executor_worker.js at init (the
// worker declares no constants of its own — it echoes what it was
// handed, so a change here reaches the worker with no second edit).
// Message type strings appear literally in executor_worker.js for
// readability; the wire_sync parity test asserts they match this file.
//
// Every message after the boot handshake (init, booted, ready, bootfailed)
// carries the lease id in `LaneMsgFields.lease`, a number from a counter per
// worker, never reused. The adapter
// drops an inbound message whose lease is not its own; the worker drops one
// that is neither the lease it serves nor a newer one while idle.
//
// Rust mirrors:
//   hostIo* ↔ host/constants.rs HOST_IO_*
//   (SAB codes are JS↔JS only — produced by WebLease, consumed by the
//    worker's Atomics waits.)
//
// INTERNAL — used by the Router, WebLease, and the parity test.

/// Host I/O return codes (host_read_at / host_write_chunk → WASM).
/// Must match `HOST_IO_ERROR` / `HOST_IO_CANCELLED` in constants.rs.
abstract final class HostIo {
  /// Host-side I/O failure.
  static const error = -1;

  /// Host-side cancellation — maps to the non-retryable cancelled
  /// error inside WASM, never to a plain failure.
  static const cancelled = -2;
}

/// Status words for the per-job SharedArrayBuffers (Atomics mode).
/// WebLease stores these + notifies; the worker's Atomics.wait reads.
abstract final class SabStatus {
  /// Request posted, host has not answered yet (the wait value).
  static const pending = 0;

  /// Host filled the data section (reads) / acknowledged (writes).
  static const ready = 1;

  /// Host failed.
  static const error = 2;

  /// Host cancelled the job.
  static const cancelled = 3;
}

/// Layout of the per-job read SAB: [status i32][len i32][data...].
abstract final class SabLayout {
  /// Byte offset where chunk data begins.
  static const headerBytes = 8;

  /// Byte offset of the response-length i32.
  static const lengthOffset = 4;

  /// Maximum chunk bytes per read — matches the engine's 64KB read
  /// channel capacity.
  static const maxChunk = 65536;

  /// Total read-SAB size.
  static const readSabBytes = headerBytes + maxChunk;

  /// Write-ack SAB is a single i32 status word.
  static const writeSabBytes = 4;

  /// Cancel SAB is a single i32: 0 until the job is cancelled, then 1.
  /// The worker reads it from inside the engine's CPU loops, so a
  /// cancel lands within one page without any I/O call.
  static const cancelSabBytes = 4;
}

/// Message type tags between WebLease (Dart) and executor_worker.js.
/// The worker uses these literally; the parity test pins them here.
abstract final class LaneMsg {
  /// Dart → worker: bootstrap (protocol, mode, wasm module).
  static const init = 'init';

  /// Dart → worker: run one job.
  static const exec = 'exec';

  /// Dart → worker: answer to a JSPI readAt.
  static const readAtResult = 'readAtResult';

  /// Dart → worker: JSPI write acknowledgement.
  static const chunkAck = 'chunkAck';

  /// Dart → worker: drop a handle's held readers.
  static const releaseHold = 'releaseHold';

  /// Dart → worker: replace the engine state with a fresh one (same wasm
  /// instance), clear the reader and job-file maps and delete the worker's
  /// own OPFS files. Refused with a typed protocol error while an exec or
  /// another reset runs.
  static const reset = 'reset';

  /// Dart → worker: OPFS pre-copy chunk.
  static const opfsWrite = 'opfsWrite';

  /// Dart → worker: a job is over — delete any pre-copy files still
  /// owned by it. The worker owns its OPFS files (it creates them,
  /// holds their handles, and is their only consumer), so deletion is
  /// same-agent and race-free. Files promoted to held readers survive
  /// until [releaseHold]; a dead worker's directory is reclaimed by
  /// the host via its liveness lock.
  static const opfsDrop = 'opfsDrop';

  /// Worker → Dart: script evaluated and message handler attached.
  /// Dart must not post `init` before this — a message posted to a
  /// worker with no handler yet is silently dropped.
  static const booted = 'booted';

  /// Bootstrap → Dart: the blob bootstrap's dynamic import of
  /// executor_worker.js failed (unreachable URL, bad script). Posted by
  /// the BOOTSTRAP, not executor_worker.js — a rejected import is an
  /// unhandled rejection in the worker, invisible to the Worker
  /// error event, so the bootstrap must report it itself.
  static const bootFailed = 'bootfailed';

  /// Worker → Dart: bootstrap complete.
  static const ready = 'ready';

  /// Worker → Dart: the engine wants source bytes.
  static const readAt = 'readAt';

  /// Worker → Dart: the engine produced an output chunk.
  static const chunk = 'chunk';

  /// Worker → Dart: job finished with these response bytes.
  static const result = 'result';

  /// Worker → Dart: job (or init) failed at the transport layer.
  static const error = 'error';

  /// Worker → Dart: the reset finished; carries the worker's wasm memory
  /// size in [LaneMsgFields.memBytes] and its heap peak in
  /// [LaneMsgFields.peakBytes].
  static const resetDone = 'resetDone';

  /// Worker → Dart: OPFS pre-copy chunk written.
  static const opfsWriteAck = 'opfsWriteAck';
}

/// Envelope field names on every lane message (both directions).
abstract final class LaneMsgFields {
  /// The discriminator field on every message.
  static const type = 'type';

  /// The lease id on every message after the boot handshake.
  static const lease = 'lease';

  /// The job a `result` or `error` answers.
  static const jobId = 'jobId';

  /// Worker → Dart, on an `error` the worker raised against the protocol
  /// itself: an exec or reset refused because the worker was busy. The
  /// refused message never started.
  static const protocolError = 'protocol';

  /// Worker → Dart, on `opfsWriteAck`: the `offset` of the `opfsWrite` it
  /// answers, beside the [jobId] it echoes. The lane completes a write only
  /// with the ack that names it.
  static const writeOffset = 'offset';

  /// Worker → Dart, on `result`, `error` and `resetDone`: the worker's
  /// committed WASM linear memory in bytes. Linear memory never shrinks, so
  /// it is the worker's lifetime peak, a diagnostic and no keep decision.
  static const memBytes = 'mem';

  /// Worker → Dart, on `result`, `error` and `resetDone`: the most bytes the
  /// engine's heap held since the last reset, the figure an executor is kept
  /// or retired by.
  static const peakBytes = 'peak';

  /// Worker → Dart, on `result` and `resetDone`: the bytes of the readers that
  /// are backed by a copy of their source in the worker. A diagnostic: the
  /// lane counts the budget itself.
  static const heldBytes = 'held';

  /// Worker → Dart, on `resetDone`: the names of the pre-copy files the
  /// worker set aside because the browser never answered their handle
  /// creation, and has not removed yet. A file leaves the list when its
  /// pending call settles.
  static const abandonedFiles = 'abandoned';

  /// Worker → Dart, on `opfsWriteAck`: present when the write had to create
  /// its handle a second time because the first call passed
  /// [kCreateHandleWait] unanswered.
  static const retriedCreate = 'retried';
}

/// How long `createSyncAccessHandle` may stay unanswered before the worker
/// sets its file aside and creates the handle on a fresh file once more.
/// The slowest create in the OPFS runner, with `stress-ng --cpu 2` running,
/// measured 40.8 ms over 12330 creates on a desktop CPU. The bound only has
/// to tell "never answers" (a Chrome fault) from slow, and a slow phone with
/// busy storage must never fail a healthy job, so it is about a hundred times
/// that: a job the fault hits fails after twice this, never hangs.
const kCreateHandleWait = Duration(seconds: 5);

/// Bytes the Asyncify mode saves an unwound wasm stack in: whole wasm pages,
/// taken from fresh linear memory outside the engine's heap. The pass writes
/// the stack without a bound, so the worker compares the fill with this after
/// each unwind and fails the job on an overflow.
const kAsyncifyStackBytes = 4 * 1024 * 1024;

/// The most source bytes one lane may hold as copies in its worker, its own
/// sources and its held ones together: a source whose bytes fit in what is
/// left is read whole on the main thread and transferred with its job. Half of
/// the pool's keep line, so the copies never decide that a worker is retired.
const kTransferBytes = 16 * 1024 * 1024;

/// The protocol object injected into the worker's init message.
/// Keys are stable API for executor_worker.js.
Map<String, int> laneProtocolCodes() => {
  'hostIoError': HostIo.error,
  'hostIoCancelled': HostIo.cancelled,
  'sabPending': SabStatus.pending,
  'sabReady': SabStatus.ready,
  'sabError': SabStatus.error,
  'sabCancelled': SabStatus.cancelled,
  'sabHeaderBytes': SabLayout.headerBytes,
  'sabLengthOffset': SabLayout.lengthOffset,
  'sabMaxChunk': SabLayout.maxChunk,
  'createHandleWaitMs': kCreateHandleWait.inMilliseconds,
  'asyncifyStackBytes': kAsyncifyStackBytes,
};
