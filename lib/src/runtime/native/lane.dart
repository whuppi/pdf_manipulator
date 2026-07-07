// NativeLane — the dumb native adapter: four verbs over FFI.
//
// One NativeLane = one Rust lane thread (spawned by lane_spawn). The
// adapter translates Lane verbs into FFI calls and serves the lane's
// I/O on the MAIN isolate via NativeCallable listeners — no helper
// isolate exists; the event loop is the only Dart-side thread.
//
// ## The post-driven cleanup protocol (memory safety)
//
// Rust guarantees EVERY accepted job posts exactly one result to its
// port — success, engine error, or cancelled — and posts only after
// the job's channels have left the lane's wake registry. That post
// is therefore the one safe moment to free the job's buffers. Kill
// completes pending futures instantly (UX), but resource cleanup
// always waits for each job's post (safety). The two are decoupled
// on purpose.
//
// ## The kill protocol (callback safety)
//
// After lane_kill returns, Rust guarantees no notify callback will
// ever fire for this lane — so callables can be closed immediately.
// Buffers still wait for their job's post (see above).
//
// INTERNAL — constructed by NativeLaneHost, driven by the Router.

import 'dart:async';
import 'dart:ffi' as ffi;
import 'dart:io' show Platform;
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/native/channel_buffers.dart';
import 'package:pdf_manipulator/src/runtime/native/bindings.dart' as bindings;
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
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
  final void Function() cleanup;

  /// Builds the success-path result (collecting held tokens) from
  /// the posted bytes.
  final LaneSubmitResult Function(Uint8List bytes) buildResult;
}

/// One isolated native execution unit. Decision-free.
class NativeLane implements Lane {
  NativeLane._(this._laneKey);

  final int _laneKey;
  final Map<int, _PendingJob> _pending = {};
  final Set<_HeldChannel> _held = {};
  bool _killed = false;

  @override
  Future<LaneSubmitResult> submit(LaneJob job) {
    if (_killed) {
      return Future.value(LaneSubmitResult(buildCancelledResponse(), const {}));
    }

    // Revive held channels: a previous job's cancel may have left
    // its sticky CANCELLED flag on them. Between jobs nothing can be
    // parked in a held channel, so this is the one safe moment.
    for (final held in _held) {
      held.buffer.clearCancelled();
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

    void cleanup() {
      port.close();
      for (var i = 0; i < sourceCount; i++) {
        if (job.keepSources.contains(i)) continue; // promoted to held
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
      return LaneSubmitResult(bytes, heldTokens);
    }

    final pendingJob = _PendingJob(completer, port, cleanup, buildResult);
    _pending[job.jobId] = pendingJob;

    port.handler = (dynamic message) {
      final posted = _pending.remove(job.jobId);
      if (posted == null) return; // already finalized
      final bytes = message is Uint8List
          ? message
          : Uint8List.fromList(const []);
      if (!posted.completer.isCompleted) {
        // Normal path: complete with the real result + held tokens.
        posted.completer.complete(posted.buildResult(bytes));
      }
      // Kill path: the future was completed at kill; this post is
      // the lane's "channels deregistered" signal — free everything.
      posted.cleanup();
      _maybeFreeHeldAfterKill();
    };

    bindings.laneSubmit(
      _laneKey,
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
    if (_killed) return;
    bindings.laneJobCancel(_laneKey, jobId);
  }

  @override
  void releaseHeld(Object token) {
    if (_killed) return; // kill owns held cleanup
    final held = token as _HeldChannel;
    if (!_held.remove(held)) return;
    bindings.laneChannelRelease(_laneKey, held.buffer.ptr);
    held.free();
  }

  @override
  void kill() {
    if (_killed) return;
    _killed = true;

    // 1. Rust: flag + wake + close mailbox. After this returns, no
    //    notify callback will ever fire for this lane.
    bindings.laneKill(_laneKey);

    // 2. Complete every pending future NOW (instant dispose UX).
    //    Their resources are freed by each job's guaranteed post.
    for (final pending in _pending.values) {
      if (!pending.completer.isCompleted) {
        pending.completer.complete(
          LaneSubmitResult(buildCancelledResponse(), const {}),
        );
      }
    }

    _maybeFreeHeldAfterKill();
  }

  /// Held buffers may still be read by the lane thread while its
  /// current job unwinds — safe to free only once every pending job
  /// has posted (= the lane is provably done with all channels).
  void _maybeFreeHeldAfterKill() {
    if (!_killed || _pending.isNotEmpty) return;
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
      try {
        final bytes = await Future<Uint8List>.value(
          source.readAt(buffer.requestOffset, buffer.requestCount),
        );
        buffer.responseLength = bytes.length;
        buffer.writeResponseData(bytes);
        buffer.setFlags(flagReady);
      } catch (_) {
        buffer.setFlags(flagError);
      }
      bindings.channelSignalRead(buffer.ptr);
    });
  }

  ffi.NativeCallable<ffi.Void Function()> _writeListener(
    WriteChannelBuffer buffer,
    DataSink sink,
  ) {
    return ffi.NativeCallable<ffi.Void Function()>.listener(() async {
      try {
        final chunk = buffer.readChunkData(buffer.chunkLength);
        await Future<void>.value(sink.write(chunk));
        buffer.setFlags(flagAck);
      } catch (_) {
        buffer.setFlags(flagError);
      }
      bindings.channelSignalWrite(buffer.ptr);
    });
  }
}
