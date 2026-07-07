// WebLane — the dumb web adapter: four verbs over postMessage.
//
// One WebLane = one Web Worker running lane_worker.js (the lane body)
// with one WASM LaneState inside. The adapter translates Lane verbs
// into messages and serves the lane's I/O from the main isolate —
// every worker is a lane; no other worker kind exists.
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
//   queued  — completed cancelled here; never reaches the worker.
//   running — the job's NEXT I/O is answered with the cancelled
//             status (and a parked Atomics wait / outstanding JSPI
//             read is answered immediately); the engine unwinds at
//             that boundary, mirroring native exactly.
//   kill    — worker.terminate(): the browser frees the worker and
//             its WASM heap; every pending job completes cancelled.
//             The worker's OPFS directory is reclaimed by the host
//             once the worker's liveness lock confirms it is gone.
//
// ## Sequencing
//
// A lane runs ONE job at a time (same as the native mailbox); the
// queue lives here because a Worker cannot peek its own message
// queue. Queue order is submission order.
//
// INTERNAL — constructed by WebLaneHost, driven by the Router.
//
// Split: this library spans two files. The lane (job lifecycle,
// 3 I/O modes) lives here; WebLaneHost (worker boot handshake,
// page-global budget, pristine pool) lives in host.dart.

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

  /// OPFS filenames pre-copied for this job's sources (index-aligned;
  /// null where no pre-copy happened). Pure exec-message data — the
  /// WORKER owns the files themselves and their deletion.
  List<String?> opfsFiles = const [];
}

// ── Lane ────────────────────────────────────────────────────────────

/// One isolated web execution unit. Decision-free.
class WebLane implements Lane {
  WebLane._(this._host);

  final WebLaneHost _host;

  LaneWorker? _worker;
  Future<void>? _ready;
  StreamSubscription<web.MessageEvent>? _sub;

  final List<_WebJob> _queue = [];
  _WebJob? _running;
  bool _killed = false;

  /// True once anything was posted to the adopted worker — a used
  /// worker holds engine/OPFS state and must die with the lane; an
  /// unused one is still pristine and returns to the host pool.
  bool _workerUsed = false;
  static int _opfsCounter = 0;

  /// Outstanding JSPI read for the running job (the worker is
  /// suspended awaiting readAtResult).
  bool _awaitingJspiRead = false;

  /// Outstanding OPFS pre-copy ack.
  Completer<String?>? _opfsAck;

  /// heldToken (creating jobId) → that job's kept DataSources. On
  /// JSPI/Atomics a held reader's bytes still live here in Dart, so
  /// ops on the handle (which carry no sources of their own) must
  /// serve reads from the CREATING job's sources. Mount order
  /// mirrors the worker's reader map: held first, own sources after.
  final Map<int, List<DataSource>> _heldSources = {};

  // ── Lane verbs ──

  @override
  Future<LaneSubmitResult> submit(LaneJob job) {
    if (_killed) {
      return Future.value(LaneSubmitResult(buildCancelledResponse(), const {}));
    }
    final entry = _WebJob(job);
    _queue.add(entry);
    _pump();
    return entry.completer.future;
  }

  @override
  void cancelJob(int jobId) {
    if (_killed) return;

    for (final entry in _queue) {
      if (entry.job.jobId == jobId) {
        entry.cancelled = true; // completed cancelled when dequeued
        return;
      }
    }

    final running = _running;
    if (running == null || running.job.jobId != jobId) return;
    running.cancelled = true;

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
  }

  @override
  void releaseHeld(Object token) {
    if (_killed) return;
    final heldJobId = token as int;
    _heldSources.remove(heldJobId);
    // The worker drops the held readers AND deletes their OPFS files
    // — it owns them (same agent as the open handles: no race).
    _post({LaneMsgFields.type: LaneMsg.releaseHeld, 'heldJobId': heldJobId});
  }

  @override
  void kill() {
    if (_killed) return;
    _killed = true;

    // Browser kill: frees the worker, its WASM heap, and every open
    // SyncAccessHandle in one stroke. Nothing can call back after.
    // A never-used worker carries no state — recycle it instead. A
    // used one is retired: the host reclaims its OPFS directory once
    // the liveness lock confirms the agent is gone.
    unawaited(_sub?.cancel());
    _sub = null;
    final worker = _worker;
    _worker = null;
    if (worker != null) {
      if (_workerUsed) {
        _host.retire(worker);
      } else {
        _host.returnPristine(worker);
      }
    }

    final cancelledResult = LaneSubmitResult(
      buildCancelledResponse(),
      const {},
    );
    final running = _running;
    _running = null;
    if (running != null) {
      running.wake();
      if (!running.completer.isCompleted) {
        running.completer.complete(cancelledResult);
      }
    }
    for (final entry in _queue) {
      if (!entry.completer.isCompleted) {
        entry.completer.complete(cancelledResult);
      }
    }
    _opfsAck?.complete(null); // loop head re-checks _killed and exits
    _opfsAck = null;
    _queue.clear();
    _heldSources.clear();
  }

  // ── Dispatch loop: one job at a time, queue order ──

  void _pump() {
    if (_killed || _running != null || _queue.isEmpty) return;
    final entry = _queue.removeAt(0);
    if (entry.cancelled) {
      entry.completer.complete(
        LaneSubmitResult(buildCancelledResponse(), const {}),
      );
      // Tail-call the next job on a microtask to keep the stack flat.
      scheduleMicrotask(_pump);
      return;
    }
    _running = entry;
    unawaited(_runJob(entry));
  }

  Future<void> _runJob(_WebJob entry) async {
    try {
      await _ensureReady();
      if (_killed || entry.cancelled) {
        _finish(entry, LaneSubmitResult(buildCancelledResponse(), const {}));
        return;
      }
      // Everything below posts to the worker (OPFS pre-copy or exec).
      _workerUsed = true;

      // OPFS mode: sources must exist on disk before exec (workers
      // read them via SyncAccessHandle).
      if (_host.mode == PdfIoMode.opfs) {
        entry.opfsFiles = await _preCopySources(entry);
        if (_killed || entry.cancelled) {
          _finish(entry, LaneSubmitResult(buildCancelledResponse(), const {}));
          return;
        }
      }

      final job = entry.job;
      final requestJs = Uint8List.fromList(job.request).buffer.toJS;
      final msg = JSObject()
        ..[LaneMsgFields.type] = LaneMsg.exec.toJS
        ..['jobId'] = job.jobId.toJS
        ..['requestBytes'] = requestJs
        ..['sourceLengths'] = [for (final s in job.sources) s.length.toJS].toJS
        ..['sinkCount'] = job.sinks.length.toJS
        ..['keepSources'] = [for (final i in job.keepSources) i.toJS].toJS;
      if (job.heldToken != null) {
        msg['heldJobId'] = (job.heldToken! as int).toJS;
      }
      if (entry.opfsFiles.any((f) => f != null)) {
        msg['opfsFiles'] = [for (final f in entry.opfsFiles) f?.toJS].toJS;
      }
      if (_host.mode == PdfIoMode.atomics) {
        entry.sab = _Sab(SabLayout.readSabBytes);
        entry.writeSab = _Sab(SabLayout.writeSabBytes);
        msg['sab'] = entry.sab;
        msg['writeSab'] = entry.writeSab;
      }

      _worker!.js.postMessage(msg, [requestJs].toJS);
      // Completion arrives via _onMessage (result | error).
    } catch (e) {
      _finish(
        entry,
        LaneSubmitResult(buildErrorResponse(e.toString()), const {}),
      );
    }
  }

  void _finish(_WebJob entry, LaneSubmitResult result) {
    // Every completion path lands here — success, engine error,
    // transport error, cancel. One verb closes the job's disk story:
    // the WORKER deletes any pre-copy files the job still owns (same
    // agent as the open handles — no race exists). Files promoted to
    // held readers survive until releaseHeld; a dead worker's whole
    // directory is reclaimed by the host via its liveness lock.
    if (_host.mode == PdfIoMode.opfs && _workerUsed) {
      _post({LaneMsgFields.type: LaneMsg.opfsDrop, 'jobId': entry.job.jobId});
    }
    if (_running == entry) {
      _running = null;
      _awaitingJspiRead = false;
    }
    if (!entry.completer.isCompleted) entry.completer.complete(result);
    _pump();
  }

  // ── Worker lifecycle ──

  Future<void> _ensureReady() {
    return _ready ??= () async {
      if (_killed) return; // disposed before the first job ran
      final worker = await _host.takeWorker(() => _killed);
      if (worker == null) return; // killed while queued for a slot
      if (_killed) {
        // Killed while adopting: nothing was posted to the worker, so
        // it is pristine by construction — back to the pool.
        _host.returnPristine(worker);
        return;
      }
      _worker = worker;
      _sub = EventStreamProviders.messageEvent
          .forTarget(worker.js)
          .listen(_onMessage);
    }();
  }

  // ── Inbound messages ──

  void _onMessage(web.MessageEvent event) {
    final obj = event.data as JSObject?;
    if (obj == null) return;
    final type = _str(obj, LaneMsgFields.type);
    final entry = _running;

    switch (type) {
      case LaneMsg.readAt:
        if (entry != null) unawaited(_serveReadAt(obj, entry));

      case LaneMsg.chunk:
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

      case LaneMsg.result:
        if (entry != null) {
          final bytes = _bytes(obj, 'data');
          _finish(entry, _buildResult(entry, bytes));
        }

      case LaneMsg.error:
        if (entry != null) {
          final message = _str(obj, 'message') ?? 'web lane error';
          final bytes = entry.cancelled
              ? buildCancelledResponse()
              : buildErrorResponse(message);
          _finish(entry, LaneSubmitResult(bytes, const {}));
        }
    }
  }

  LaneSubmitResult _buildResult(_WebJob entry, Uint8List bytes) {
    // Cancelled mid-op: the caller sees the typed status, identical
    // to native's run_job. (Set-once flag — cannot misfire.)
    if (entry.cancelled) {
      return LaneSubmitResult(buildCancelledResponse(), const {});
    }

    final heldTokens = <int, Object>{};
    if (entry.job.keepSources.isNotEmpty && bytes.isNotEmpty && bytes[0] == 1) {
      // Token = the creating job's id — the same id the worker used
      // to mark its promoted readers (heldJobId). Ascending index
      // order matches the worker's reader-creation order, which is
      // what held-read serving relies on.
      final kept = entry.job.keepSources.toList()..sort();
      for (final i in kept) {
        heldTokens[i] = entry.job.jobId;
      }
      _heldSources[entry.job.jobId] = [
        for (final i in kept)
          if (i < entry.job.sources.length) entry.job.sources[i],
      ];
    }

    return LaneSubmitResult(bytes, heldTokens);
  }

  // ── I/O serving ──

  Future<void> _serveReadAt(JSObject obj, _WebJob entry) async {
    final sourceIndex = _int(obj, 'sourceIndex') ?? 0;
    final offset = _int(obj, 'offset') ?? 0;
    final count = _int(obj, 'count') ?? 0;
    final isAtomics = _host.mode == PdfIoMode.atomics;
    if (!isAtomics) _awaitingJspiRead = true;

    // The worker mounts a handle's held readers FIRST (they sit
    // inside the document at the indices the engine knew them by),
    // then this job's own sources. Mirror that order exactly: a
    // held-reader index resolves to the CREATING job's kept sources;
    // anything past them is this job's own.
    final heldToken = entry.job.heldToken;
    final heldSources = heldToken == null
        ? const <DataSource>[]
        : _heldSources[heldToken as int] ?? const <DataSource>[];

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
        bytes = await Future<Uint8List>.value(source.readAt(offset, count));
      }
    } catch (e) {
      error = e.toString();
    }

    // Answer-time check: a cancel that landed during the read wins.
    // (The kill path never reaches here — the worker is dead.)
    if (_killed || _running != entry) return;
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
      if (sinkIndex < entry.job.sinks.length) {
        await Future<void>.value(entry.job.sinks[sinkIndex].write(data));
      } else {
        error = 'no sink[$sinkIndex]';
      }
    } catch (e) {
      error = e.toString();
    }

    if (_killed || _running != entry) return;
    final cancelled = entry.cancelled;

    if (_host.mode == PdfIoMode.atomics) {
      final sab = entry.writeSab;
      if (sab == null) return;
      _sabSignal(
        sab,
        cancelled
            ? SabStatus.cancelled
            : (error != null ? SabStatus.error : SabStatus.ready),
      );
    } else if (_host.mode == PdfIoMode.jspi) {
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
        if (_killed || entry.cancelled) return names;
        final count = (offset + SabLayout.maxChunk > source.length)
            ? source.length - offset
            : SabLayout.maxChunk;
        // Race the caller's read against the cancel wake: a parked
        // DataSource must never make a cancelled job unkillable.
        final chunk = await Future.any([
          Future<Uint8List>.value(source.readAt(offset, count)),
          entry.cancelWake.future.then((_) => Uint8List(0)),
        ]);
        if (_killed || entry.cancelled) return names;

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
        final ackError = await ack.future;
        _opfsAck = null;
        if (ackError != null && ackError != 'cancelled') {
          throw StateError('OPFS pre-copy failed: $ackError');
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
    _worker?.js.postMessage(obj);
  }

  void _postRaw(Map<String, JSAny?> fields, [List<JSObject>? transfer]) {
    final obj = JSObject();
    fields.forEach((k, v) => obj[k] = v);
    if (transfer != null) {
      _worker?.js.postMessage(obj, transfer.toJS);
    } else {
      _worker?.js.postMessage(obj);
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
