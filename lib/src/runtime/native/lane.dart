// NativeExecutor and NativeLane — the dumb native adapter: the verbs of
// Lane over FFI.
//
// One NativeExecutor = one Rust lane thread (spawned by lane_spawn); one
// NativeLane = one lease on it. The adapter translates Lane verbs into FFI
// calls and serves the thread's I/O on the MAIN isolate via NativeCallable
// listeners — no helper isolate exists; the event loop is the only
// Dart-side thread.
//
// ## The post-driven cleanup protocol (memory safety)
//
// Rust guarantees EVERY accepted job posts exactly one result to its
// port — success, engine error, or cancelled — and posts only after
// the job's channels have left the lane's wake registry. That post
// is therefore the one safe moment to free the job's buffers. Ending a
// lane completes pending futures instantly (UX), but resource cleanup
// always waits for each job's post (safety). The two are decoupled
// on purpose.
//
// ## The abandon protocol (callback safety)
//
// After lane_kill (an executor's terminate) returns, Rust guarantees no
// notify callback will ever fire for this thread — so callables can be
// closed immediately. Buffers still wait for their job's post (see above).
//
// INTERNAL — constructed by NativeLaneHost, driven by the pool.

import 'dart:async';
import 'dart:ffi' as ffi;
import 'dart:io' show Platform;
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart'
    show decodeResponse;
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/native/channel_buffers.dart';
import 'package:pdf_manipulator/src/runtime/native/bindings.dart' as bindings;
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

part 'host.dart';

/// Creates the native [LaneHost]. The platform conditional import selects this
/// where dart:io exists. [config] is unused natively — lane sizing comes from
/// the host.
LaneHost createLaneHost({PdfConfig? config}) => NativeLaneHost();

/// A read channel kept alive for a handle's lifetime (the engine
/// moved its source into the document). Opaque to the Router.
class _HeldChannel {
  _HeldChannel(this.buffer, this.callable);

  final ReadChannelBuffer buffer;
  final ffi.NativeCallable<ffi.Void Function()> callable;

  void free() {
    callable.close();
    bindings.channelDestroyRead(buffer.ptr);
    buffer.free();
  }
}

/// Everything one in-flight job owns. Freed exactly once, when the
/// job's result post arrives.
class _PendingJob {
  _PendingJob(this.completer, this.port, this.cleanup, this.buildResult);

  final Completer<LaneSubmitResult> completer;
  final RawReceivePort port;

  /// Closes the port, closes non-kept callables, destroys + frees
  /// non-kept buffers, frees the FFI argument arrays.
  final void Function({required bool freeKept}) cleanup;

  /// True once the lane was told to skip this job before it started: it
  /// never registered a channel, so the kept ones are freed with the rest.
  bool skipped = false;

  /// Builds the success-path result (collecting held tokens) from
  /// the posted bytes.
  final LaneSubmitResult Function(Uint8List bytes) buildResult;
}

/// One Rust lane thread with its engine state. Decision-free.
class NativeExecutor implements Executor {
  NativeExecutor._(this.laneKey);

  /// The thread's key in the Rust lane table. Internal: tests replay
  /// messages against it.
  final int laneKey;

  NativeLane? _lane;
  bool _terminated = false;

  @override
  int get heapBytes => bindings.laneHeapPeak(laneKey);

  @override
  Lane beginLane() {
    final lease = bindings.laneLeaseBegin(laneKey);
    if (lease == 0) {
      throw const LaneProtocolError(
        'the lane is gone, busy, holding a channel or dead',
      );
    }
    return _lane = NativeLane._(this, lease);
  }

  @override
  void terminate() {
    if (_terminated) return;
    _terminated = true;
    // Rust: flag + wake + close mailbox. After this returns, no notify
    // callback will ever fire for this thread.
    bindings.laneKill(laneKey);
    _lane?._abandon();
  }
}

/// One lease on a [NativeExecutor]. Decision-free.
class NativeLane implements Lane {
  NativeLane._(this._executor, this._lease);

  final NativeExecutor _executor;

  /// This lane's lease on its executor: every submit, cancel, release and
  /// reset carries it, and the executor drops one that is not the current
  /// lease. Read-channel and write-channel answers need no lease: each is
  /// numbered by its channel's request word and goes to a buffer one job
  /// owns, freed at that job's post.
  final int _lease;

  int get _laneKey => _executor.laneKey;

  /// The executor's key in the Rust lane table. Internal: tests replay
  /// messages against it.
  int get laneKey => _laneKey;

  /// The lease this lane holds. Internal: tests replay messages under it.
  int get lease => _lease;
  final Map<int, _PendingJob> _pending = {};
  final Set<_HeldChannel> _held = {};

  /// True once [drain] ran: no job is accepted, and the lane is done when
  /// the last pending job has posted.
  bool _draining = false;

  /// True once the executor was terminated: nothing more is served.
  bool _abandoned = false;
  final Completer<bool> _drained = Completer<bool>();

  @override
  Future<LaneSubmitResult> submit(LaneJob job) {
    if (_draining || _abandoned) {
      return Future.value(LaneSubmitResult(buildCancelledResponse(), const {}));
    }

    // ── Allocate the FFI argument block ──
    final requestPtr = calloc<ffi.Uint8>(job.request.length);
    requestPtr.asTypedList(job.request.length).setAll(0, job.request);

    final sourceCount = job.sources.length;
    final sinkCount = job.sinks.length;
    final srcBufs = calloc<ffi.Pointer<ffi.Uint8>>(sourceCount);
    final srcNotify =
        calloc<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>(
          sourceCount,
        );
    final srcLengths = calloc<ffi.Int64>(sourceCount);
    final srcKeeps = calloc<ffi.Uint8>(sourceCount);
    final snkBufs = calloc<ffi.Pointer<ffi.Uint8>>(sinkCount);
    final snkNotify =
        calloc<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>(sinkCount);

    final readChannels = <int, ReadChannelBuffer>{};
    final readCallables = <int, ffi.NativeCallable<ffi.Void Function()>>{};
    final writeChannels = <int, WriteChannelBuffer>{};
    final writeCallables = <int, ffi.NativeCallable<ffi.Void Function()>>{};

    for (var i = 0; i < sourceCount; i++) {
      final buffer = ReadChannelBuffer();
      bindings.channelInitRead(buffer.ptr);
      final callable = _readListener(buffer, job.sources[i]);
      readChannels[i] = buffer;
      readCallables[i] = callable;
      srcBufs[i] = buffer.ptr;
      srcNotify[i] = callable.nativeFunction;
      srcLengths[i] = job.sources[i].length;
      srcKeeps[i] = job.keepSources.contains(i) ? 1 : 0;
    }

    for (var i = 0; i < sinkCount; i++) {
      final buffer = WriteChannelBuffer();
      bindings.channelInitWrite(buffer.ptr);
      final callable = _writeListener(buffer, job.sinks[i]);
      writeChannels[i] = buffer;
      writeCallables[i] = callable;
      snkBufs[i] = buffer.ptr;
      snkNotify[i] = callable.nativeFunction;
    }

    // ── Result port + lifecycle ──
    final completer = Completer<LaneSubmitResult>();
    final port = RawReceivePort();

    void cleanup({required bool freeKept}) {
      port.close();
      for (var i = 0; i < sourceCount; i++) {
        if (!freeKept && job.keepSources.contains(i)) {
          continue; // promoted to held
        }
        readCallables[i]!.close();
        bindings.channelDestroyRead(readChannels[i]!.ptr);
        readChannels[i]!.free();
      }
      for (var i = 0; i < sinkCount; i++) {
        writeCallables[i]!.close();
        bindings.channelDestroyWrite(writeChannels[i]!.ptr);
        writeChannels[i]!.free();
      }
      calloc.free(requestPtr);
      calloc.free(srcBufs);
      calloc.free(srcNotify);
      calloc.free(srcLengths);
      calloc.free(srcKeeps);
      calloc.free(snkBufs);
      calloc.free(snkNotify);
    }

    LaneSubmitResult buildResult(Uint8List bytes) {
      final heldTokens = <int, Object>{};
      for (final i in job.keepSources) {
        final buffer = readChannels[i];
        final callable = readCallables[i];
        if (buffer == null || callable == null) continue;
        final held = _HeldChannel(buffer, callable);
        _held.add(held);
        heldTokens[i] = held;
      }
      return LaneSubmitResult(
        bytes,
        heldTokens,
        laneDied: bindings.laneIsDead(_laneKey) != 0,
      );
    }

    final pendingJob = _PendingJob(completer, port, cleanup, buildResult);
    _pending[job.jobId] = pendingJob;

    port.handler = (dynamic message) {
      final posted = _pending.remove(job.jobId);
      if (posted == null) return; // already finalized
      final bytes = message is Uint8List
          ? message
          : Uint8List.fromList(const []);
      // A job the lane skipped never registered a channel; every other job
      // may have moved its kept sources into the engine, so they become the
      // lane's held channels even when the caller was told "cancelled"
      // already. A reset or the executor's end frees them.
      final result = posted.skipped ? null : posted.buildResult(bytes);
      if (result != null && !posted.completer.isCompleted) {
        posted.completer.complete(result);
      }
      // This post is the lane's "channels deregistered" signal — free
      // everything the job owns.
      posted.cleanup(freeKept: posted.skipped);
      _settle();
      _freeHeldIfAbandoned();
    };

    bindings.laneSubmit(
      _laneKey,
      _lease,
      job.jobId,
      requestPtr,
      job.request.length,
      sourceCount,
      srcBufs,
      srcNotify,
      srcLengths,
      srcKeeps,
      sinkCount,
      snkBufs,
      snkNotify,
      port.sendPort.nativePort,
    );

    return completer.future;
  }

  @override
  void cancelJob(int jobId) {
    if (_draining || _abandoned) return;
    final queued = bindings.laneJobCancel(_laneKey, _lease, jobId) != 0;
    // A job the lane has not started never reaches the engine, so its
    // caller need not wait for the running job ahead of it. The lane
    // still posts for it, and that post frees its buffers.
    if (!queued) return;
    final pending = _pending[jobId];
    if (pending == null) return;
    pending.skipped = true;
    if (!pending.completer.isCompleted) {
      pending.completer.complete(
        LaneSubmitResult(buildCancelledResponse(), const {}),
      );
    }
  }

  @override
  void releaseHeld(Object token) {
    if (_draining || _abandoned) return; // the end owns held cleanup
    final held = token as _HeldChannel;
    if (!_held.remove(held)) return;
    bindings.laneChannelRelease(_laneKey, _lease, held.buffer.ptr);
    held.free();
  }

  @override
  Future<int?> reset() async {
    if (_abandoned) throw const LaneProtocolError('the lane was abandoned');
    final answer = Completer<Uint8List>();
    final port = RawReceivePort((dynamic message) {
      if (answer.isCompleted) return;
      answer.complete(
        message is Uint8List ? message : Uint8List.fromList(const []),
      );
    });
    final Uint8List bytes;
    try {
      bindings.laneReset(_laneKey, _lease, port.sendPort.nativePort);
      bytes = await answer.future;
    } finally {
      port.close();
    }
    try {
      decodeResponse(bytes);
    } on PdfEngineFailed catch (refused) {
      throw LaneProtocolError(refused.message);
    } on PdfCancelled {
      throw const LaneProtocolError('the lane was abandoned');
    }
    // The engine dropped its readers and forgot the held channels before it
    // answered, so their buffers are ours to free now.
    for (final held in _held) {
      held.free();
    }
    _held.clear();
    return null;
  }

  @override
  Future<bool> drain() {
    if (!_draining && !_abandoned) {
      _draining = true;
      // Cancel every job: a queued one is skipped and completes at once,
      // a running one unwinds at its next I/O or cancel poll. Completing
      // every future now is the instant-dispose UX; resources are freed by
      // each job's guaranteed post.
      for (final jobId in _pending.keys.toList()) {
        final queued = bindings.laneJobCancel(_laneKey, _lease, jobId) != 0;
        if (queued) _pending[jobId]?.skipped = true;
      }
      for (final pending in _pending.values) {
        if (!pending.completer.isCompleted) {
          pending.completer.complete(
            LaneSubmitResult(buildCancelledResponse(), const {}),
          );
        }
      }
    }
    _settle();
    return _drained.future;
  }

  /// Settles [drain] once every job has posted: the thread then holds
  /// nothing of this lane, and says whether its engine is intact.
  void _settle() {
    if (!_draining || _pending.isNotEmpty || _drained.isCompleted) return;
    _drained.complete(bindings.laneIsDead(_laneKey) == 0);
  }

  /// The executor was terminated: every pending future completes cancelled
  /// now, and the lane's resources are freed as its jobs post.
  void _abandon() {
    _abandoned = true;
    for (final pending in _pending.values) {
      if (!pending.completer.isCompleted) {
        pending.completer.complete(
          LaneSubmitResult(buildCancelledResponse(), const {}),
        );
      }
    }
    if (!_drained.isCompleted) _drained.complete(false);
    _freeHeldIfAbandoned();
  }

  /// Held buffers may still be read by the thread while its current job
  /// unwinds — safe to free only once every pending job has posted (= the
  /// thread is provably done with all channels).
  void _freeHeldIfAbandoned() {
    if (!_abandoned || _pending.isNotEmpty) return;
    for (final held in _held) {
      held.free();
    }
    _held.clear();
  }

  // ── I/O serving (main isolate, async, never blocks the lane) ──

  ffi.NativeCallable<ffi.Void Function()> _readListener(
    ReadChannelBuffer buffer,
    DataSource source,
  ) {
    return ffi.NativeCallable<ffi.Void Function()>.listener(() async {
      final request = buffer.requestSeq;
      // The job may have ended, or the engine may have moved on to a newer
      // request, while the source was busy: either way this answer is not
      // wanted, and the newer request has its own listener.
      bool stale() => buffer.freed || buffer.requestSeq != request;
      try {
        final bytes = await Future<Uint8List>.value(
          source.readAt(buffer.requestOffset, buffer.requestCount),
        );
        if (stale()) return;
        buffer.responseLength = bytes.length;
        buffer.writeResponseData(bytes);
        buffer.respond(request, flagReady);
      } catch (_) {
        if (stale()) return;
        buffer.respond(request, flagError);
      }
      bindings.channelSignalRead(buffer.ptr);
    });
  }

  ffi.NativeCallable<ffi.Void Function()> _writeListener(
    WriteChannelBuffer buffer,
    DataSink sink,
  ) {
    return ffi.NativeCallable<ffi.Void Function()>.listener(() async {
      final request = buffer.requestSeq;
      // As for reads: a freed buffer, or a newer chunk already posted,
      // makes this acknowledgement unwanted.
      bool stale() {
        if (buffer.freed) {
          WriteChannelBuffer.writesAfterFree++;
          return true;
        }
        return buffer.requestSeq != request;
      }

      try {
        final chunk = buffer.readChunkData(buffer.chunkLength);
        await Future<void>.value(sink.write(chunk));
        if (stale()) return;
        buffer.respond(request, flagAck);
      } catch (_) {
        if (stale()) return;
        buffer.respond(request, flagError);
      }
      bindings.channelSignalWrite(buffer.ptr);
    });
  }
}
