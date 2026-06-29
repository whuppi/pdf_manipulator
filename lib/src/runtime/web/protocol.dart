// Lane protocol — the single source of truth for every constant the
// web lane body and the Dart side must agree on.
//
// The numeric codes are INJECTED into lane_worker.js at init (the
// worker declares no constants of its own — it echoes what it was
// handed, so a change here reaches the worker with no second edit).
// Message type strings appear literally in lane_worker.js for
// readability; the wire_sync parity test asserts they match this file.
//
// Rust mirrors:
//   hostIo* ↔ host/constants.rs HOST_IO_*
//   (SAB codes are JS↔JS only — produced by WebLane, consumed by the
//    worker's Atomics waits.)
//
// INTERNAL — used by the Router, WebLane, and the parity test.

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
/// WebLane stores these + notifies; the worker's Atomics.wait reads.
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
}

/// Message type tags between WebLane (Dart) and lane_worker.js.
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
  static const releaseHeld = 'releaseHeld';

  /// Dart → worker: OPFS pre-copy chunk.
  static const opfsWrite = 'opfsWrite';

  /// Dart → worker: a job is over — delete any pre-copy files still
  /// owned by it. The worker owns its OPFS files (it creates them,
  /// holds their handles, and is their only consumer), so deletion is
  /// same-agent and race-free. Files promoted to held readers survive
  /// until [releaseHeld]; a dead worker's directory is reclaimed by
  /// the host via its liveness lock.
  static const opfsDrop = 'opfsDrop';

  /// Worker → Dart: script evaluated and message handler attached.
  /// Dart must not post `init` before this — a message posted to a
  /// worker with no handler yet is silently dropped.
  static const booted = 'booted';

  /// Bootstrap → Dart: the blob bootstrap's dynamic import of
  /// lane_worker.js failed (unreachable URL, bad script). Posted by
  /// the BOOTSTRAP, not lane_worker.js — a rejected import is an
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

  /// Worker → Dart: OPFS pre-copy chunk written.
  static const opfsWriteAck = 'opfsWriteAck';
}

/// Envelope field names on every lane message (both directions).
abstract final class LaneMsgFields {
  /// The discriminator field on every message.
  static const type = 'type';
}

/// The protocol object injected into the worker's init message.
/// Keys are stable API for lane_worker.js.
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
};
