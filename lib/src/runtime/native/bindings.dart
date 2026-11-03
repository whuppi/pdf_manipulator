// FFI bindings for the Rust lane surface (lane_table.rs).
//
// Dumb by design — shared brain, dumb edges: every symbol is one
// Router/Lane verb or one channel-buffer helper. No logic lives on
// either side of this file.
//
// Lane keys are opaque u64s — Rust looks them up in a locked table,
// so a call racing a kill finds nothing instead of touching freed
// memory. Keys are never reused.
//
// INTERNAL — used by lane.dart only.

@ffi.DefaultAsset('package:pdf_manipulator/src/ffi/native_bindings.g.dart')
library;

import 'dart:ffi' as ffi;

// ── allo-isolate bootstrap ───────────────────────────────────────

/// Registers the Dart post-C-object callback for allo-isolate.
/// Called once per process before the first lane spawn.
@ffi.Native<
  ffi.Void Function(
    ffi.Pointer<
      ffi.NativeFunction<ffi.Bool Function(ffi.Int64, ffi.Pointer<ffi.Void>)>
    >,
  )
>(symbol: 'store_dart_post_cobject')
external void storeDartPostCobject(
  ffi.Pointer<
    ffi.NativeFunction<ffi.Bool Function(ffi.Int64, ffi.Pointer<ffi.Void>)>
  >
  postCObject,
);

// ── Lane lifecycle ───────────────────────────────────────────────

/// Creates a lane. Returns its opaque key. Never blocks: under
/// global thread-budget pressure the lane starts later, FIFO.
@ffi.Native<ffi.Uint64 Function()>(symbol: 'lane_spawn')
external int laneSpawn();

/// Starts a new lease on a lane and returns its id (from 1, per lane,
/// never reused), or 0 when the lane is gone, has a job outstanding or a
/// held channel registered, or died. From then on a message under an older
/// lease is dropped.
@ffi.Native<ffi.Uint64 Function(ffi.Uint64)>(symbol: 'lane_lease_begin')
external int laneLeaseBegin(int laneKey);

/// Replaces the lane's engine state with a fresh one. The answer is
/// posted to [resultPort] exactly once: a success once the lane has
/// reset, or the error that refused it.
@ffi.Native<ffi.Void Function(ffi.Uint64, ffi.Uint64, ffi.Int64)>(
  symbol: 'lane_reset',
)
external void laneReset(int laneKey, int lease, int resultPort);

/// Kills a lane and everything on it. Instant; after it returns no
/// notify callback will ever fire for this lane.
@ffi.Native<ffi.Void Function(ffi.Uint64)>(symbol: 'lane_kill')
external void laneKill(int laneKey);

/// Cancels one job on a lane. Instant, idempotent. Returns 1 when the
/// job had not started: the lane skips it, so it can complete at once.
/// Under a lease that is not current it changes nothing and returns 0.
@ffi.Native<ffi.Uint8 Function(ffi.Uint64, ffi.Uint64, ffi.Uint64)>(
  symbol: 'lane_job_cancel',
)
external int laneJobCancel(int laneKey, int lease, int jobId);

/// 1 once the lane's engine panicked and the lane stopped serving.
@ffi.Native<ffi.Uint8 Function(ffi.Uint64)>(symbol: 'lane_is_dead')
external int laneIsDead(int laneKey);

/// Releases one held channel (its handle was disposed). The caller
/// frees the buffer only AFTER this returns.
@ffi.Native<ffi.Void Function(ffi.Uint64, ffi.Uint64, ffi.Pointer<ffi.Uint8>)>(
  symbol: 'lane_channel_release',
)
external void laneChannelRelease(
  int laneKey,
  int lease,
  ffi.Pointer<ffi.Uint8> buf,
);

/// Started lane threads, process-wide. Diagnostics and tests only.
@ffi.Native<ffi.Uint32 Function()>(symbol: 'lane_live_count')
external int laneLiveCount();

// ── Job submission ───────────────────────────────────────────────

/// Submits one job under [lease]. The result is posted to [resultPort] exactly
/// once — success bytes, an engine error, or a cancelled error.
/// That post is the ONLY signal after which the job's buffers may be
/// freed.
///
/// Sources: parallel arrays (buffer, notify fn, length, keep flag),
/// [sourceCount] entries. Sinks: parallel arrays (buffer, notify fn),
/// [sinkCount] entries.
@ffi.Native<
  ffi.Void Function(
    ffi.Uint64,
    ffi.Uint64,
    ffi.Uint64,
    ffi.Pointer<ffi.Uint8>,
    ffi.Int32,
    ffi.Int32,
    ffi.Pointer<ffi.Pointer<ffi.Uint8>>,
    ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>,
    ffi.Pointer<ffi.Int64>,
    ffi.Pointer<ffi.Uint8>,
    ffi.Int32,
    ffi.Pointer<ffi.Pointer<ffi.Uint8>>,
    ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>,
    ffi.Int64,
  )
>(symbol: 'lane_submit')
external void laneSubmit(
  int laneKey,
  int lease,
  int jobId,
  ffi.Pointer<ffi.Uint8> requestPtr,
  int requestLen,
  int sourceCount,
  ffi.Pointer<ffi.Pointer<ffi.Uint8>> sourceBufs,
  ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>
  sourceNotifyFns,
  ffi.Pointer<ffi.Int64> sourceLengths,
  ffi.Pointer<ffi.Uint8> sourceKeeps,
  int sinkCount,
  ffi.Pointer<ffi.Pointer<ffi.Uint8>> sinkBufs,
  ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>
  sinkNotifyFns,
  int resultPort,
);

// ── Channel buffer helpers ───────────────────────────────────────

/// Required size of a read-channel buffer, in bytes.
@ffi.Native<ffi.Int32 Function()>(symbol: 'channel_read_buffer_size')
external int channelReadBufferSize();

/// Required size of a write-channel buffer, in bytes.
@ffi.Native<ffi.Int32 Function()>(symbol: 'channel_write_buffer_size')
external int channelWriteBufferSize();

/// Initializes the sync pair inside a read-channel buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
  symbol: 'channel_init_read',
)
external void channelInitRead(ffi.Pointer<ffi.Uint8> buf);

/// Destroys the sync pair inside a read-channel buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
  symbol: 'channel_destroy_read',
)
external void channelDestroyRead(ffi.Pointer<ffi.Uint8> buf);

/// Signals the read-channel condvar (request fulfilled).
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
  symbol: 'channel_signal_read',
)
external void channelSignalRead(ffi.Pointer<ffi.Uint8> buf);

/// Initializes the sync pair inside a write-channel buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
  symbol: 'channel_init_write',
)
external void channelInitWrite(ffi.Pointer<ffi.Uint8> buf);

/// Destroys the sync pair inside a write-channel buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
  symbol: 'channel_destroy_write',
)
external void channelDestroyWrite(ffi.Pointer<ffi.Uint8> buf);

/// Signals the write-channel condvar (chunk acknowledged).
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
  symbol: 'channel_signal_write',
)
external void channelSignalWrite(ffi.Pointer<ffi.Uint8> buf);
