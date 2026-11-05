// WebExecutor and WebLane — the dumb web adapter: the verbs of Lane over
// postMessage.
//
// One WebExecutor = one Web Worker running lane_worker.js (the lane body)
// with one WASM LaneState inside; one WebLane = one lease on it. The
// adapter translates Lane verbs into messages and serves the lane's I/O
// from the main isolate — every worker is an executor; no other worker
// kind exists.
//
// ## Mode physics (selected once by WebLaneHost, injected at init)
//
//   jspi    — worker's host_read_at returns a Promise; this side
//             answers with a readAtResult message.
//   atomics — per-job SharedArrayBuffers; this side fills data and
//             Atomics.store + notify (allowed on main — only wait
//             is forbidden here).
//   opfs    — sources pre-copied to OPFS via the worker (only
//             workers may open SyncAccessHandles); reads are local
//             to the worker afterwards.
//
// ## Cancellation
//
//   queued  — removed from the queue and completed cancelled at once;
//             never reaches the worker.
//   running — the job's NEXT I/O is answered with the cancelled
//             status (and a parked Atomics wait / outstanding JSPI
//             read is answered immediately); the engine unwinds at
//             that boundary, mirroring native exactly. The job also carries
//             a cancel channel the engine's CPU loops poll, so it stops
//             within one page even when it makes no I/O call: a cancel
//             word where the page has SharedArrayBuffer, otherwise a blob
//             URL the worker polls with a throttled sync XHR and this side
//             revokes.
//   drain   — the lane ends: queued jobs complete cancelled at once, the
//             running one is cancelled as above and its caller is
//             answered at once, but the worker stays busy until the job
//             posts. Only then is the lane drained, and the pool resets
//             the worker for its next lease. //   terminate — only for a dead worker or a pool that is going away:
//             worker.terminate() frees the worker and its WASM heap;
//             every pending job completes cancelled. The worker's OPFS
//             directory is reclaimed by the host once the worker's
//             liveness lock confirms it is gone.
//
// ## Lease
//
// The lane holds one lease on its worker: a number from the worker's own
// counter, taken when the pool hands the worker over. Every message the
// lane posts carries it, and the lane drops any message from the worker
// that does not echo it, so an answer meant for an earlier tenure never
// resolves a job of this one. A `result` or `error` also has to name the
// job that is running.
//
// ## Sequencing
//
// A lane runs ONE job at a time (same as the native mailbox); the
// queue lives here because a Worker cannot peek its own message
// queue. Queue order is submission order.
//
// INTERNAL — constructed by WebLaneHost, driven by the pool.
//
// Split: this library spans two files. The lane (job lifecycle,
// 3 I/O modes) lives here; WebLaneHost (worker boot handshake, OPFS
// session reclaim) and the executor live in host.dart.

import 'dart:async';
import 'dart:js_interop';
import 'dart:math' as math;
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:web/web.dart' as web;
import 'package:web/web.dart' show EventStreamProviders;

import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/web/protocol.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

part 'host.dart';

/// Creates the web [LaneHost]. The platform conditional import selects this
/// where dart:js_interop exists. [config] supplies the lane-worker URL and an
/// optional I/O-mode override.
LaneHost createLaneHost({PdfConfig? config}) => WebLaneHost(
  laneWorkerUrl: config?.webLaneWorkerUrl,
  forceMode: config?.webIoMode,
);

// ── JS interop: SAB + Atomics (main thread: store/notify only) ─────

@JS('SharedArrayBuffer')
extension type _Sab._(JSObject _) implements JSObject {
  external factory _Sab(int byteLength);
}

@JS('Int32Array')
extension type _I32._(JSObject _) implements JSObject {
  external factory _I32(JSObject buffer, int byteOffset, int length);
}

@JS('Uint8Array')
extension type _U8._(JSObject _) implements JSObject {
  external factory _U8(JSObject buffer, int byteOffset);
  external void set(JSUint8Array source, int offset);
}

@JS('Atomics.store')
external int _atomicsStore(JSObject typedArray, int index, int value);

@JS('Atomics.notify')
external int _atomicsNotify(JSObject typedArray, int index);

@JS('WebAssembly.compileStreaming')
external JSPromise<JSObject> _wasmCompileStreaming(JSAny source);

@JS('fetch')
external JSPromise<JSObject> _fetch(JSString url);

void _sabSignal(JSObject sab, int status) {
  final view = _I32(sab, 0, 1);
  _atomicsStore(view, 0, status);
  _atomicsNotify(view, 0);
}

// ── One in-flight or queued job's full state ───────────────────────

class _WebJob {
  _WebJob(this.job);

  final LaneJob job;
  final completer = Completer<LaneSubmitResult>();
  bool cancelled = false;

  /// True once the exec message is on its way to the worker: from then on
  /// only the worker can end the job.
  bool posted = false;

  /// Wakes a job parked on host I/O outside the worker — the OPFS
  /// pre-copy awaits the caller's DataSource, which a cancel cannot
  /// reach any other way (same instant-wake contract as the native
  /// channel registry).
  final cancelWake = Completer<void>();

  void wake() {
    if (!cancelWake.isCompleted) cancelWake.complete();
  }

  /// Sequencer for sink writes — see the chunk handler.
  Future<void> sinkChain = Future.value();

  /// Atomics-mode SABs (created per job at dispatch).
  JSObject? sab;
  JSObject? writeSab;

  /// The per-job cancel word, where the page has SharedArrayBuffer.
  JSObject? cancelSab;

  /// Where the page has none: the per-job blob URL the worker polls.
  /// Revoking it is the cancel; null once revoked.
  String? cancelUrl;

  /// OPFS filenames pre-copied for this job's sources (index-aligned;
  /// null where no pre-copy happened). Pure exec-message data — the
  /// WORKER owns the files themselves and their deletion.
  List<String?> opfsFiles = const [];
}

// ── Lane ────────────────────────────────────────────────────────────

/// One lease on a [WebExecutor]. Decision-free.
class WebLane implements Lane {
  WebLane._(this._exec, this._lease);

  final WebExecutor _exec;

  /// This lane's lease on its worker; every message it posts carries it.
  final int _lease;

  PdfIoMode get _mode => _exec._mode;

  final List<_WebJob> _queue = [];
  _WebJob? _running;

  /// True once [drain] ran: nothing more is accepted, and the lane is done
  /// when the running job has posted.
  bool _ending = false;

  /// True once the executor was terminated: nothing more is served.
  bool _abandoned = false;

  final Completer<bool> _drained = Completer<bool>();

  /// True once the worker trapped: the lane serves nothing more. Queued and
  /// later jobs complete with the lane-dead error; the pool terminates the
  /// worker.
  bool _dead = false;
  static int _opfsCounter = 0;

  /// Outstanding JSPI read for the running job (the worker is
  /// suspended awaiting readAtResult).
  bool _awaitingJspiRead = false;

  /// Outstanding JSPI write for the running job (the worker is
  /// suspended awaiting chunkAck).
  bool _awaitingJspiWrite = false;

  /// Outstanding OPFS pre-copy ack.
  Completer<String?>? _opfsAck;

  /// The reset in flight; its answer is `resetDone` or an `error` that
  /// names no job.
  Completer<int?>? _reset;

  /// The lease every message of this lane carries. Internal: tests replay
  /// messages under it.
  int get lease => _lease;

  /// The worker this lane leases. Internal: tests read its directory.
  WebExecutor get executor => _exec;

  /// Held group (the handle's creating jobId) → the DataSources its
  /// jobs kept, in the order they were kept. On JSPI/Atomics a held
  /// reader's bytes still live here in Dart, so ops on the handle
  /// (which carry no sources of their own) must serve reads from the
  /// group's sources. Mount order mirrors the worker's reader map:
  /// held first, own sources after.
  final Map<int, List<DataSource>> _heldSources = {};

  // ── Lane verbs ──

  @override
  Future<LaneSubmitResult> submit(LaneJob job) {
    if (_ending || _abandoned) {
      return Future.value(LaneSubmitResult(buildCancelledResponse(), const {}));
    }
    if (_dead) return Future.value(_deadResult());
    final entry = _WebJob(job);
    _queue.add(entry);
    _pump();
    return entry.completer.future;
  }

  @override
  void cancelJob(int jobId) {
    if (_abandoned) return;

    final queued = _queue.indexWhere((entry) => entry.job.jobId == jobId);
    if (queued >= 0) {
      final entry = _queue.removeAt(queued);
      entry.cancelled = true;
      entry.completer.complete(
        LaneSubmitResult(buildCancelledResponse(), const {}),
      );
      return;
    }

    final running = _running;
    if (running == null || running.job.jobId != jobId) return;
    _cancelRunning(running);
  }

  void _cancelRunning(_WebJob running) {
    running.cancelled = true;

    // The engine's CPU loops poll this word between pages, so the job
    // stops within one page even while the worker never touches I/O.
    final cancelSab = running.cancelSab;
    if (cancelSab != null) _atomicsStore(_I32(cancelSab, 0, 1), 0, 1);
    _revokeCancelUrl(running);

    // Wake anything parked RIGHT NOW — same instant-wake contract as
    // the native channel registry.
    running.wake();
    if (running.sab != null) _sabSignal(running.sab!, SabStatus.cancelled);
    if (running.writeSab != null) {
      _sabSignal(running.writeSab!, SabStatus.cancelled);
    }
    if (_awaitingJspiRead) {
      _awaitingJspiRead = false;
      _post({LaneMsgFields.type: LaneMsg.readAtResult, 'cancelled': true});
    }
    if (_awaitingJspiWrite) {
      _awaitingJspiWrite = false;
      _post({LaneMsgFields.type: LaneMsg.chunkAck, 'cancelled': true});
    }
  }

  @override
  void releaseHeld(Object token) {
    if (_ending || _abandoned) return;
    final heldGroupId = token as int;
    _heldSources.remove(heldGroupId);
    // The worker drops the group's held readers AND deletes their OPFS
    // files — it owns them (same agent as the open handles: no race).
    // Every token of a handle names the same group, so a repeat is a no-op.
    _post({
      LaneMsgFields.type: LaneMsg.releaseHeld,
      'heldGroupId': heldGroupId,
    });
  }

  @override
  Future<int?> reset() async {
    if (_abandoned) throw const LaneProtocolError('the lane was abandoned');
    if (_dead) throw const LaneProtocolError(laneDeadMessage);
    // Refused here, not in the worker: a queued job or an OPFS pre-copy
    // has not reached the worker yet, and a reset would pull the files
    // from under it.
    if (_running != null || _queue.isNotEmpty || _reset != null) {
      throw const LaneProtocolError(laneBusyMessage);
    }
    final answer = _reset = Completer<int?>();
    try {
      _post({LaneMsgFields.type: LaneMsg.reset});
      final mem = await answer.future;
      _heldSources.clear();
      return mem;
    } finally {
      _reset = null;
    }
  }

  @override
  Future<bool> drain() {
    if (!_ending && !_abandoned) {
      _ending = true;
      final running = _running;
      if (running != null) {
        // The same cancel as the caller's: the engine unwinds at its next
        // I/O or cancel poll, and the worker is busy until it posts.
        _cancelRunning(running);
        if (!running.completer.isCompleted) {
          running.completer.complete(
            LaneSubmitResult(buildCancelledResponse(), const {}),
          );
        }
      }
      final cancelled = LaneSubmitResult(buildCancelledResponse(), const {});
      for (final entry in _queue) {
        entry.cancelled = true;
        if (!entry.completer.isCompleted) entry.completer.complete(cancelled);
      }
      _queue.clear();
    }
    _settle();
    return _drained.future;
  }

  /// Settles [drain] once no job is running: the worker then holds nothing
  /// of this lane, and says whether its engine is intact.
  void _settle() {
    if (!_ending || _running != null || _drained.isCompleted) return;
    _drained.complete(!_dead);
  }

  /// The executor was terminated: every pending job completes cancelled
  /// now and nothing more is served.
  void _abandon() {
    if (_abandoned) return;
    _abandoned = true;
    final cancelled = LaneSubmitResult(buildCancelledResponse(), const {});
    final running = _running;
    _running = null;
    if (running != null) {
      _revokeCancelUrl(running);
      running.wake();
      if (!running.completer.isCompleted) running.completer.complete(cancelled);
    }
    for (final entry in _queue) {
      if (!entry.completer.isCompleted) entry.completer.complete(cancelled);
    }
    _opfsAck?.complete(null); // loop head re-checks _abandoned and exits
    _opfsAck = null;
    final resetting = _reset;
    _reset = null;
    if (resetting != null && !resetting.isCompleted) {
      resetting.completeError(
        const LaneProtocolError('the lane was abandoned'),
      );
    }
    _queue.clear();
    _heldSources.clear();
    if (!_drained.isCompleted) _drained.complete(false);
  }

  // ── Dispatch loop: one job at a time, queue order ──

  void _pump() {
    if (_ending || _abandoned || _running != null || _queue.isEmpty) return;
    if (_dead) {
      for (final entry in _queue) {
        entry.completer.complete(_deadResult());
      }
      _queue.clear();
      return;
    }
    final entry = _queue.removeAt(0);
    _running = entry;
    unawaited(_runJob(entry));
  }

  Future<void> _runJob(_WebJob entry) async {
    try {
      if (_abandoned || entry.cancelled) {
        _finish(entry, LaneSubmitResult(buildCancelledResponse(), const {}));
        return;
      }

      // OPFS mode: sources must exist on disk before exec (workers
      // read them via SyncAccessHandle).
      if (_mode == PdfIoMode.opfs) {
        entry.opfsFiles = await _preCopySources(entry);
        if (_abandoned || entry.cancelled) {
          _finish(entry, LaneSubmitResult(buildCancelledResponse(), const {}));
          return;
        }
      }

      final job = entry.job;
      final requestJs = Uint8List.fromList(job.request).buffer.toJS;
      final msg = JSObject()
        ..[LaneMsgFields.type] = LaneMsg.exec.toJS
        ..[LaneMsgFields.lease] = _lease.toJS
        ..[LaneMsgFields.jobId] = job.jobId.toJS
        ..['requestBytes'] = requestJs
        ..['sourceLengths'] = [for (final s in job.sources) s.length.toJS].toJS
        ..['sinkCount'] = job.sinks.length.toJS
        ..['keepSources'] = [for (final i in job.keepSources) i.toJS].toJS;
      if (job.heldGroupId != null) {
        msg['heldGroupId'] = job.heldGroupId!.toJS;
      }
      if (entry.opfsFiles.any((f) => f != null)) {
        msg['opfsFiles'] = [for (final f in entry.opfsFiles) f?.toJS].toJS;
      }
      if (globalContext.hasProperty('SharedArrayBuffer'.toJS).toDart) {
        entry.cancelSab = _Sab(SabLayout.cancelSabBytes);
        msg['cancelSab'] = entry.cancelSab;
      } else {
        entry.cancelUrl = web.URL.createObjectURL(web.Blob(<JSAny>[].toJS));
        msg['cancelUrl'] = entry.cancelUrl!.toJS;
      }
      if (_mode == PdfIoMode.atomics) {
        entry.sab = _Sab(SabLayout.readSabBytes);
        entry.writeSab = _Sab(SabLayout.writeSabBytes);
        msg['sab'] = entry.sab;
        msg['writeSab'] = entry.writeSab;
      }

      entry.posted = true;
      _exec.js.postMessage(msg, [requestJs].toJS);
      // Completion arrives via _onMessage (result | error).
    } catch (e) {
      _finish(
        entry,
        LaneSubmitResult(buildErrorResponse(e.toString()), const {}),
      );
    }
  }

  void _finish(_WebJob entry, LaneSubmitResult result) {
    _revokeCancelUrl(entry);
    // Every completion path lands here — success, engine error,
    // transport error, cancel. One verb closes the job's disk story:
    // the WORKER deletes any pre-copy files the job still owns (same
    // agent as the open handles — no race exists). Files promoted to
    // held readers survive until releaseHeld; a dead worker's whole
    // directory is reclaimed by the host via its liveness lock.
    if (_mode == PdfIoMode.opfs && !_abandoned) {
      _post({LaneMsgFields.type: LaneMsg.opfsDrop, 'jobId': entry.job.jobId});
    }
    if (_running == entry) {
      _running = null;
      _awaitingJspiRead = false;
      _awaitingJspiWrite = false;
    }
    if (!entry.completer.isCompleted) entry.completer.complete(result);
    _settle();
    _pump();
  }

  /// Revoking the URL is the cancel; once, and never reused.
  void _revokeCancelUrl(_WebJob entry) {
    final url = entry.cancelUrl;
    if (url == null) return;
    entry.cancelUrl = null;
    web.URL.revokeObjectURL(url);
  }

  LaneSubmitResult _deadResult() => LaneSubmitResult(
    buildErrorResponse(laneDeadMessage),
    const {},
    laneDied: true,
  );

  /// The worker trapped: the pool terminates it, never reuses it, and
  /// everything the lane held is gone with it.
  void _bury() {
    _dead = true;
    _exec._dead = true;
    _heldSources.clear();
  }

  // ── Inbound messages ──

  /// Handles one message from the worker. Internal: the listener calls it,
  /// and tests call it to replay a captured message.
  void deliver(JSObject? obj) {
    if (obj == null) return;
    // A message of another tenure never acts here, whatever it says.
    if (_int(obj, LaneMsgFields.lease) != _lease) return;
    final type = _str(obj, LaneMsgFields.type);
    final entry = _running;
    // A `result` or `error` belongs to the job it names: one about any other
    // job (the refusal of a second exec) never ends the running one.
    final answered =
        entry != null && _int(obj, LaneMsgFields.jobId) == entry.job.jobId;
    // The engine's heap peak ranks the worker for reuse and retires it past
    // the keep line.
    final peak = _int(obj, LaneMsgFields.peakBytes);
    if (peak != null) _exec._heapBytes = peak;
    final mem = _int(obj, LaneMsgFields.memBytes);
    if (mem != null) _exec._memBytes = mem;

    switch (type) {
      case LaneMsg.readAt:
        if (entry != null) unawaited(_serveReadAt(obj, entry));

      case LaneMsg.chunk:
        if (_mode == PdfIoMode.jspi) _awaitingJspiWrite = true;
        // Chained, never parallel: chunk N+1's sink.write may not
        // start before chunk N's completes (ordering is the sink
        // contract; OPFS mode has no worker-side backpressure).
        if (entry != null) {
          entry.sinkChain = entry.sinkChain.then(
            (_) => _serveChunk(obj, entry),
          );
        }

      case LaneMsg.opfsWriteAck:
        _opfsAck?.complete(_str(obj, 'error'));

      case LaneMsg.resetDone:
        _reset?.complete(mem);

      case LaneMsg.result:
        if (entry != null && answered) {
          final bytes = _bytes(obj, 'data');
          final succeeded = bytes.isNotEmpty && bytes[0] == 1;
          if (entry.cancelled && !succeeded) {
            // The op stopped: a sink write still in flight must not hold
            // the answer back.
            _finish(entry, _buildResult(entry, bytes));
            return;
          }
          // A success ends after its last sink write, cancelled or not:
          // the caller must not be told "saved" while a chunk is still
          // being written. OPFS mode has no worker-side back-pressure, so
          // the result can arrive while chunks it follows are still being
          // written.
          unawaited(
            entry.sinkChain.then(
              (_) => _finish(entry, _buildResult(entry, bytes)),
            ),
          );
        }

      case LaneMsg.error:
        final resetting = _reset;
        if (_int(obj, LaneMsgFields.jobId) == null) {
          // No job named: the failure of a reset.
          if (resetting != null && !resetting.isCompleted) {
            resetting.completeError(
              LaneProtocolError(
                _str(obj, 'message') ?? 'web lane reset failed',
              ),
            );
          }
        } else if (entry != null && answered && obj['trap'] != null) {
          // A wasm trap is a Rust panic: the engine's state is gone, so
          // the lane ends here, exactly as a panic ends a native lane.
          _dead = true;
          _finish(
            entry,
            LaneSubmitResult(
              buildErrorResponse('internal error: engine operation panicked'),
              const {},
              laneDied: true,
            ),
          );
          _bury();
        } else if (entry != null && answered) {
          final message = _str(obj, 'message') ?? 'web lane error';
          final bytes = entry.cancelled
              ? buildCancelledResponse()
              : buildErrorResponse(message);
          _finish(entry, LaneSubmitResult(bytes, const {}));
        }
    }
  }

  LaneSubmitResult _buildResult(_WebJob entry, Uint8List bytes) {
    // Cancelled mid-op: the engine's I/O error reads as the typed
    // status, identical to native's run_job. A success is never
    // rewritten: it may have created a handle only the Router can name
    // and dispose.
    final succeeded = bytes.isNotEmpty && bytes[0] == 1;
    if (entry.cancelled && !succeeded) {
      return LaneSubmitResult(buildCancelledResponse(), const {});
    }

    final heldTokens = <int, Object>{};
    if (entry.job.keepSources.isNotEmpty && bytes.isNotEmpty && bytes[0] == 1) {
      // Token = the handle's group id (its creating job) — the same id
      // the worker used to mark its promoted readers (heldGroupId).
      // Ascending index order matches the worker's reader-creation
      // order, and a later job's sources append after the group's
      // earlier ones: that is what held-read serving relies on.
      final group = entry.job.heldGroupId ?? entry.job.jobId;
      final kept = entry.job.keepSources.toList()..sort();
      for (final i in kept) {
        heldTokens[i] = group;
      }
      (_heldSources[group] ??= []).addAll([
        for (final i in kept)
          if (i < entry.job.sources.length) entry.job.sources[i],
      ]);
    }

    return LaneSubmitResult(bytes, heldTokens);
  }

  // ── I/O serving ──

  Future<void> _serveReadAt(JSObject obj, _WebJob entry) async {
    final sourceIndex = _int(obj, 'sourceIndex') ?? 0;
    final offset = _int(obj, 'offset') ?? 0;
    final count = _int(obj, 'count') ?? 0;
    final isAtomics = _mode == PdfIoMode.atomics;
    if (!isAtomics) _awaitingJspiRead = true;

    // The worker mounts a handle's held readers FIRST (they sit
    // inside the document at the indices the engine knew them by),
    // then this job's own sources. Mirror that order exactly: a
    // held-reader index resolves to the CREATING job's kept sources;
    // anything past them is this job's own.
    final heldGroupId = entry.job.heldGroupId;
    final heldSources = heldGroupId == null
        ? const <DataSource>[]
        : _heldSources[heldGroupId] ?? const <DataSource>[];

    Uint8List? bytes;
    String? error;
    try {
      final DataSource? source;
      if (sourceIndex < heldSources.length) {
        source = heldSources[sourceIndex];
      } else {
        final own = sourceIndex - heldSources.length;
        source = own < entry.job.sources.length ? entry.job.sources[own] : null;
      }
      if (source == null) {
        error = 'no source[$sourceIndex]';
      } else {
        // A cancelled job asks nothing of the caller's source: the answer
        // below is "cancelled" either way, and a draining worker may still
        // ask after its Pdf was disposed.
        if (!entry.cancelled) {
          bytes = await Future<Uint8List>.value(source.readAt(offset, count));
        }
      }
    } catch (e) {
      error = e.toString();
    }

    // Answer-time check: a cancel that landed during the read wins.
    // (A terminated worker never reaches here.)
    if (_abandoned || _running != entry) return;
    final cancelled = entry.cancelled;

    if (isAtomics) {
      final sab = entry.sab;
      if (sab == null) return;
      if (cancelled) {
        _sabSignal(sab, SabStatus.cancelled);
      } else if (error != null || bytes == null) {
        _sabSignal(sab, SabStatus.error);
      } else {
        _U8(sab, SabLayout.headerBytes).set(bytes.toJS, 0);
        _atomicsStore(_I32(sab, SabLayout.lengthOffset, 1), 0, bytes.length);
        _sabSignal(sab, SabStatus.ready);
      }
    } else {
      if (!_awaitingJspiRead) return;
      _awaitingJspiRead = false;
      if (cancelled) {
        _post({LaneMsgFields.type: LaneMsg.readAtResult, 'cancelled': true});
      } else if (error != null || bytes == null) {
        _post({
          LaneMsgFields.type: LaneMsg.readAtResult,
          'error': error ?? 'read failed',
        });
      } else {
        final js = Uint8List.fromList(bytes).buffer.toJS;
        _postRaw(
          {LaneMsgFields.type: LaneMsg.readAtResult.toJS, 'bytes': js},
          [js],
        );
      }
    }
  }

  Future<void> _serveChunk(JSObject obj, _WebJob entry) async {
    final sinkIndex = _int(obj, 'sinkIndex') ?? 0;
    final data = _bytes(obj, 'data');

    String? error;
    try {
      if (entry.cancelled && _mode != PdfIoMode.opfs) {
        // The answer below is "cancelled", so the engine cannot finish
        // this job: a draining worker's chunk is never written to a sink
        // whose Pdf was disposed. OPFS has no answer channel, so its writes
        // always land.
      } else if (sinkIndex < entry.job.sinks.length) {
        await Future<void>.value(entry.job.sinks[sinkIndex].write(data));
      } else {
        error = 'no sink[$sinkIndex]';
      }
    } catch (e) {
      error = e.toString();
    }

    if (_abandoned || _running != entry) return;
    final cancelled = entry.cancelled;

    if (_mode == PdfIoMode.atomics) {
      final sab = entry.writeSab;
      if (sab == null) return;
      _sabSignal(
        sab,
        cancelled
            ? SabStatus.cancelled
            : (error != null ? SabStatus.error : SabStatus.ready),
      );
    } else if (_mode == PdfIoMode.jspi) {
      // A cancel already answered this write.
      if (!_awaitingJspiWrite) return;
      _awaitingJspiWrite = false;
      _post({
        LaneMsgFields.type: LaneMsg.chunkAck,
        if (cancelled) 'cancelled': true,
        if (error != null) 'error': error,
      });
    }
    // OPFS mode: fire-and-forget writes, no ack channel.
  }

  // ── OPFS pre-copy (through the worker — SyncAccessHandle is
  //    worker-only) ──

  Future<List<String?>> _preCopySources(_WebJob entry) async {
    // Bookkeeping-free by design: the worker registers each file
    // before its first byte hits disk, so whatever exists is always
    // accounted for — by its owner.
    final names = List<String?>.filled(entry.job.sources.length, null);
    for (var i = 0; i < entry.job.sources.length; i++) {
      final source = entry.job.sources[i];
      if (source.length <= 0) continue;
      final name = 'pdf_lane_${entry.job.jobId}_${i}_${_opfsCounter++}';
      names[i] = name;

      var offset = 0;
      while (offset < source.length) {
        if (_abandoned || entry.cancelled) return names;
        final count = (offset + SabLayout.maxChunk > source.length)
            ? source.length - offset
            : SabLayout.maxChunk;
        // Race the caller's read against the cancel wake: a parked
        // DataSource must never make a cancelled job unkillable.
        final chunk = await Future.any([
          Future<Uint8List>.value(source.readAt(offset, count)),
          entry.cancelWake.future.then((_) => Uint8List(0)),
        ]);
        if (_abandoned || entry.cancelled) return names;

        final ack = Completer<String?>();
        _opfsAck = ack;
        final owned = Uint8List.fromList(chunk);
        final js = owned.buffer.toJS;
        _postRaw(
          {
            LaneMsgFields.type: LaneMsg.opfsWrite.toJS,
            'jobId': entry.job.jobId.toJS,
            'filename': name.toJS,
            'chunk': js,
            'offset': offset.toJS,
            'last': (offset + count >= source.length).toJS,
          },
          [js],
        );
        // Raced like the read above: a worker that never answers (or a
        // drain that cancels mid-write) must not leave the job running.
        final String? ackError;
        try {
          ackError = await Future.any([
            ack.future,
            entry.cancelWake.future.then((_) => 'cancelled'),
          ]);
        } finally {
          _opfsAck = null;
        }
        if (ackError != null && ackError != 'cancelled') {
          throw PdfIoError('OPFS pre-copy failed: $ackError');
        }
        offset += count;
      }
    }
    return names;
  }

  // ── Outbound + interop helpers ──

  void _post(Map<String, Object?> fields) {
    final obj = JSObject();
    // jsify: canonical Dart->JS conversion, identical on dart2js and dart2wasm.
    // Already-JS values (buffers, ports) go through _postRaw, not here.
    fields.forEach((k, v) => obj[k] = v.jsify());
    obj[LaneMsgFields.lease] = _lease.toJS;
    if (!_abandoned) _exec.js.postMessage(obj);
  }

  void _postRaw(Map<String, JSAny?> fields, [List<JSObject>? transfer]) {
    final obj = JSObject();
    fields.forEach((k, v) => obj[k] = v);
    obj[LaneMsgFields.lease] = _lease.toJS;
    if (_abandoned) return;
    if (transfer != null) {
      _exec.js.postMessage(obj, transfer.toJS);
    } else {
      _exec.js.postMessage(obj);
    }
  }

  static String? _str(JSObject obj, String key) {
    final v = obj[key];
    return v == null ? null : (v as JSString).toDart;
  }

  static int? _int(JSObject obj, String key) {
    final v = obj[key];
    return v == null ? null : (v as JSNumber).toDartInt;
  }

  static Uint8List _bytes(JSObject obj, String key) {
    final v = obj[key];
    if (v == null) return Uint8List(0);
    if (v.isA<JSArrayBuffer>()) {
      return Uint8List.fromList((v as JSArrayBuffer).toDart.asUint8List());
    }
    if (v.isA<JSUint8Array>()) {
      return Uint8List.fromList((v as JSUint8Array).toDart);
    }
    return Uint8List(0);
  }
}
