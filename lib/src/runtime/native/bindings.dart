// FFI bindings for the Rust executor surface (executor_table.rs).
//
// A direct translation: every symbol is one lease-adapter verb or one
// channel-buffer helper. No logic lives on either side of this file.
//
// Executor keys are opaque u64s — Rust looks them up in a locked table,
// so a call racing a terminate finds nothing instead of touching freed
// memory. Keys are never reused.
//
// INTERNAL — used by lane.dart only.

@ffi.DefaultAsset('package:pdf_manipulator/src/ffi/native_bindings.g.dart')
library;

import 'dart:ffi' as ffi;

// ── allo-isolate bootstrap ───────────────────────────────────────

/// Registers the Dart post-C-object callback for allo-isolate.
/// Called once per process before the first executor boot.
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

// ── Executor lifecycle ───────────────────────────────────────────────

/// Creates an executor. Returns its opaque key. Never blocks: under
/// global thread-budget pressure the executor starts later, FIFO.
@ffi.Native<ffi.Uint64 Function()>(symbol: 'executor_boot')
external int executorBoot();

/// Starts a new lease on an executor and returns its id (from 1, per executor,
/// never reused), or 0 when the executor is gone, has a job outstanding or a
/// held channel registered, or died. From then on a message under an older
/// lease is dropped.
@ffi.Native<ffi.Uint64 Function(ffi.Uint64)>(symbol: 'executor_lease_begin')
external int executorLeaseBegin(int executorKey);

/// Replaces the executor's engine state with a fresh one. The answer is
/// posted to [resultPort] exactly once: a success once the executor has
/// reset, or the error that refused it.
@ffi.Native<ffi.Void Function(ffi.Uint64, ffi.Uint64, ffi.Int64)>(
  symbol: 'executor_reset',
)
external void executorReset(int executorKey, int lease, int resultPort);

/// Terminates an executor and everything on it. Instant; after it returns no
/// notify callback will ever fire for this executor.
@ffi.Native<ffi.Void Function(ffi.Uint64)>(symbol: 'executor_terminate')
external void executorTerminate(int executorKey);

/// Cancels one job on an executor. Instant, idempotent. Returns 1 when the
/// job had not started: the executor skips it, so it can complete at once.
/// Under a lease that is not current it changes nothing and returns 0.
@ffi.Native<ffi.Uint8 Function(ffi.Uint64, ffi.Uint64, ffi.Uint64)>(
  symbol: 'executor_job_cancel',
)
external int executorJobCancel(int executorKey, int lease, int jobId);

/// 1 once the executor's engine panicked and the executor stopped serving.
@ffi.Native<ffi.Uint8 Function(ffi.Uint64)>(symbol: 'executor_is_dead')
external int executorIsDead(int executorKey);

/// The most bytes the executor's thread has held since its last reset, as of the
/// last job or reset it answered; 0 once the executor is gone.
@ffi.Native<ffi.Uint64 Function(ffi.Uint64)>(symbol: 'executor_heap_peak')
external int executorHeapPeak(int executorKey);

/// Releases one held channel (its handle was disposed). The caller
/// frees the buffer only AFTER this returns.
@ffi.Native<ffi.Void Function(ffi.Uint64, ffi.Uint64, ffi.Pointer<ffi.Uint8>)>(
  symbol: 'executor_hold_release',
)
external void executorHoldRelease(
  int executorKey,
  int lease,
  ffi.Pointer<ffi.Uint8> buf,
);

/// Started executor threads, process-wide. Diagnostics and tests only.
@ffi.Native<ffi.Uint32 Function()>(symbol: 'executor_live_count')
external int executorLiveCount();

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
>(symbol: 'executor_submit')
external void executorSubmit(
  int executorKey,
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
