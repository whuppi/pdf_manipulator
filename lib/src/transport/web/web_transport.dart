// WebTransport — web platform PdfTransport implementation.
//
// Symmetric with NativeTransport. Manages one coordinator JS Worker
// that owns a pool of WASM workers.
//
// ── Multi-source / multi-sink ────────────────────────────────────
//
// Each execute() can pass N sources + M sinks. Sources are served
// via readAt callbacks indexed by sourceIndex. Sinks receive chunks
// indexed by sinkIndex. OPFS mode pre-copies each source to disk.
//
// ── Three I/O modes (auto-detected: jspi > atomics > opfs) ──────
//
//   jspi    — JSPI Promise suspension (Chrome 137+ / Firefox 139+)
//   atomics — SAB + Atomics.wait/notify (needs COOP/COEP)
//   opfs    — Pre-copy to OPFS disk (universal)

import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:web/web.dart' as web;
import 'package:web/web.dart' show EventStreamProviders;

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/transport/pdf_transport.dart';

/// Web WASM transport — routes ops through a JS coordinator worker.
class WebTransport implements PdfTransport {
  /// Creates a transport using the given coordinator and worker URLs.
  WebTransport({String? coordinatorUrl, String? workerUrl, PdfIoMode? ioMode})
      : _coordinatorUrl = coordinatorUrl ?? _cachedCoordinatorUrl ?? 'pdf_manipulator/coordinator.js',
        // worker.js is resolved by the coordinator relative to its own URL.
        // Since both files live in the same directory, just the filename suffices.
        _workerUrl = workerUrl ?? _cachedWorkerUrl ?? 'worker.js',
        _forceIoMode = ioMode,
        _instanceId = _nextInstanceId++;

  // ── Configuration ──────────────────────────────────────────────

  final String _coordinatorUrl;
  final String _workerUrl;
  final PdfIoMode? _forceIoMode;
  final int _instanceId;
  static int _nextInstanceId = 0;

  // ── Process-global caches (shared across Pdf instances) ────────

  static JSObject? _cachedWasmModule;
  static String? _cachedCoordinatorUrl;
  static String? _cachedWorkerUrl;

  // ── Instance state ─────────────────────────────────────────────

  web.Worker? _coordinator;
  bool _disposed = false;
  bool _ready = false;
  Completer<void>? _initCompleter;
  PdfIoMode? _detectedIoMode;
  StreamSubscription<web.MessageEvent>? _msgSub;
  int _nextOpId = 0;

  // ── Per-op state (cleaned on result/error) ─────────────────────

  final _pending = <int, Completer<Uint8List>>{};
  final _streams = <int, StreamController<Uint8List>>{};
  final _sources = <int, List<DataSource>>{};
  final _sinks = <int, DataSink>{};
  final _opfsAcks = <String, Completer<void>>{};

  // ── Handle-lifetime state (cleaned on releaseSource/dispose) ───

  final _heldSources = <int, DataSource>{};
  final _sourceOpIds = <int, int>{};
  int _nextResourceId = 1;

  // ═════════════════════════════════════════════════════════════════
  // Public API
  // ═════════════════════════════════════════════════════════════════

  @override
  PdfIoMode? get ioMode => _detectedIoMode;

  @override
  Future<PdfIoMode> ensureInitialized() async {
    await _ensureReady();
    return _detectedIoMode!;
  }

  @override
  Future<({Uint8List bytes, Map<int, int> resourceIds})> execute(
    Uint8List request, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
    Set<int> keepSources = const {},
  }) async {
    if (_disposed) throw StateError('WebTransport disposed');
    await _ensureReady();

    final id = _nextOpId++;
    final completer = Completer<Uint8List>();
    _pending[id] = completer;
    if (sources.isNotEmpty) _sources[id] = sources;
    if (sinks.isNotEmpty) _sinks[id] = sinks.first;

    // OPFS pre-copy: all sources go to disk
    final opfsFileNames = await _preCopySourcesToOpfs(id, sources);

    // Build args object with source lengths
    final args = _buildSourceArgs(sources, sinks, keepSources);

    // Send to coordinator
    final jsBytes = request.buffer.toJS;
    _postRaw({
      'type': 'submit'.toJS,
      'opId': id.toJS,
      'requestBytes': jsBytes,
      if (opfsFileNames.isNotEmpty) 'opfsFile': opfsFileNames.first.toJS,
      if (opfsFileNames.length > 1) 'opfsFiles': opfsFileNames.join(',').toJS,
      'args': args,
    }, [jsBytes]);

    final bytes = await completer.future;

    // Register held sources AFTER success
    final resourceIds = <int, int>{};
    for (final idx in keepSources) {
      if (idx < sources.length) {
        final resourceId = _nextResourceId++;
        _heldSources[resourceId] = sources[idx];
        _sourceOpIds[resourceId] = id;
        resourceIds[idx] = resourceId;
      }
    }
    if (resourceIds.isNotEmpty) _sources[id] = sources;

    return (bytes: bytes, resourceIds: resourceIds);
  }

  @override
  Future<void> releaseSource(int resourceId) async {
    final opId = _sourceOpIds.remove(resourceId);
    _heldSources.remove(resourceId);
    if (opId != null) _sources.remove(opId);
  }

  @override
  Stream<Uint8List> executeStream(
    Uint8List request, {
    List<DataSource> sources = const [],
  }) async* {
    if (_disposed) throw StateError('WebTransport disposed');
    await _ensureReady();

    final id = _nextOpId++;
    final controller = StreamController<Uint8List>();
    _streams[id] = controller;
    if (sources.isNotEmpty) _sources[id] = sources;

    final args = JSObject();
    if (sources.isNotEmpty) args['sourceLength'] = sources.first.length.toJS;

    final jsBytes = request.buffer.toJS;
    _postRaw({
      'type': 'submitStream'.toJS,
      'opId': id.toJS,
      'requestBytes': jsBytes,
      'args': args,
    }, [jsBytes]);

    try {
      await for (final chunk in controller.stream) {
        yield chunk;
      }
    } finally {
      _streams.remove(id);
      _sources.remove(id);
      unawaited(controller.close());
    }
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;

    _pending.clear();
    _streams.clear();
    _sources.clear();
    _sinks.clear();
    _heldSources.clear();
    _sourceOpIds.clear();
    _opfsAcks.clear();

    // Tell coordinator to dispose, then terminate
    if (_coordinator != null) {
      final done = Completer<void>();
      void onDisposed(web.MessageEvent event) {
        final obj = event.data as JSObject?;
        if (obj != null && _str(obj, 'type') == 'disposed') done.complete();
      }
      _coordinator!.addEventListener('message', onDisposed.toJS);
      _post({'type': 'dispose'});
      await done.future.timeout(const Duration(seconds: 3), onTimeout: () {});
    }

    await _msgSub?.cancel();
    _coordinator?.terminate();
    _coordinator = null;
  }

  // ═════════════════════════════════════════════════════════════════
  // Coordinator lifecycle
  // ═════════════════════════════════════════════════════════════════

  Future<void> _ensureReady() async {
    if (_ready) return;
    if (_disposed) throw StateError('WebTransport disposed');
    if (_initCompleter != null) return _initCompleter!.future;

    _initCompleter = Completer<void>();
    _coordinator = _spawnWorker(_coordinatorUrl);
    _msgSub = EventStreamProviders.messageEvent
        .forTarget(_coordinator!)
        .listen(_onMessage);

    _post({
      'type': 'init',
      'workerUrl': _workerUrl,
      if (_forceIoMode != null) 'forceIoMode': _forceIoMode.name,
      if (_cachedWasmModule != null) 'wasmModule': _cachedWasmModule,
    });

    await _initCompleter!.future.timeout(
      const Duration(seconds: 10),
      onTimeout: () => throw StateError('WebTransport init timed out'),
    );
  }

  // ═════════════════════════════════════════════════════════════════
  // Inbound message dispatch
  // ═════════════════════════════════════════════════════════════════

  void _onMessage(web.MessageEvent event) {
    final obj = event.data as JSObject?;
    if (obj == null) return;
    final type = _str(obj, 'type');

    if (type == 'ready') return _onReady(obj);

    final id = _int(obj, 'opId');

    // Broadcast error (no opId) — fail all pending
    if (type == 'error' && id == null) {
      final msg = _str(obj, 'message') ?? 'Coordinator error';
      for (final c in _pending.values) {
        c.completeError(StateError(msg));
      }
      _pending.clear();
      return;
    }
    if (id == null) return;

    switch (type) {
      case 'submitted':       break;
      case 'opfs.writeAck':
      case 'opfs.finalizeAck': _opfsAcks.remove('$id:$type')?.complete();
      case 'readAt':           _handleReadAt(obj, id);
      case 'chunk':
        final writeResult = _sinks[id]?.write(_bytes(obj, 'data'));
        if (writeResult is Future) unawaited(writeResult);
        _post({'type': 'chunkAck', 'opId': id});
      case 'result':           _cleanupOp(id); _pending.remove(id)?.complete(_bytes(obj, 'data'));
      case 'error':            _cleanupOp(id); _pending.remove(id)?.completeError(StateError(_str(obj, 'message') ?? 'Bridge error'));
      case 'item':             _streams[id]?.add(_bytes(obj, 'data'));
      case 'done':             _cleanupOp(id); unawaited(_streams.remove(id)?.close());
      case 'streamError':      _cleanupOp(id); final c = _streams.remove(id); c?.addError(StateError(_str(obj, 'message') ?? 'Stream error')); unawaited(c?.close());
    }
  }

  void _onReady(JSObject obj) {
    _ready = true;
    _detectedIoMode = _parseIoMode(_str(obj, 'ioMode'));
    final mod = obj['wasmModule'];
    if (mod != null && _cachedWasmModule == null) _cachedWasmModule = mod as JSObject;
    _cachedCoordinatorUrl ??= _coordinatorUrl;
    _cachedWorkerUrl ??= _workerUrl;
    if (_initCompleter != null && !_initCompleter!.isCompleted) _initCompleter!.complete();
  }

  void _cleanupOp(int opId) {
    if (!_sourceOpIds.values.contains(opId)) _sources.remove(opId);
    _sinks.remove(opId);
  }

  // ═════════════════════════════════════════════════════════════════
  // readAt handling
  // ═════════════════════════════════════════════════════════════════

  void _handleReadAt(JSObject obj, int opId) {
    final readId = _str(obj, 'readId');
    final offset = _int(obj, 'offset') ?? 0;
    final count = _int(obj, 'count') ?? 0;
    final sourceOpId = _int(obj, 'sourceOpId');
    final sourceIndex = _int(obj, 'sourceIndex') ?? 0;

    // Resolve source list: held (pinned) + this op's new sources
    final sources = _resolveSourceList(opId, sourceOpId);

    if (sources == null || sourceIndex >= sources.length || readId == null) {
      _sendReadAtError(readId ?? '', 'No source[$sourceIndex] for opId=$opId');
      return;
    }

    final source = sources[sourceIndex];
    try {
      final result = source.readAt(offset, count);
      if (result is Future<Uint8List>) {
        unawaited(result
            .then((b) => _sendReadAtResponse(readId, b))
            .catchError((Object e) => _sendReadAtError(readId, e)));
      } else {
        _sendReadAtResponse(readId, result);
      }
    } catch (e) {
      _sendReadAtError(readId, e);
    }
  }

  List<DataSource>? _resolveSourceList(int opId, int? sourceOpId) {
    if (sourceOpId != null && _sources.containsKey(sourceOpId)) {
      final held = _sources[sourceOpId]!;
      final opSources = _sources[opId];
      return opSources != null ? [...held, ...opSources] : held;
    }
    return _sources[opId];
  }

  void _sendReadAtResponse(String readId, Uint8List bytes) {
    // Ensure we transfer only the view's slice, not the entire backing buffer.
    // A sublistView shares the parent's ArrayBuffer — transferring .buffer
    // would send the wrong byte range to the worker.
    final slice = Uint8List.fromList(bytes);
    final jsBytes = slice.buffer.toJS;
    _postRaw({
      'type': 'readAtResponse'.toJS,
      'readId': readId.toJS,
      'bytes': jsBytes,
    }, [jsBytes]);
  }

  void _sendReadAtError(String readId, Object error) {
    _post({'type': 'readAtResponse', 'readId': readId, 'error': error.toString()});
  }

  // ═════════════════════════════════════════════════════════════════
  // OPFS pre-copy (OPFS mode only)
  // ═════════════════════════════════════════════════════════════════

  bool get _needsOpfsPreCopy {
    final mode = _detectedIoMode ?? _forceIoMode;
    return mode == null || mode == PdfIoMode.opfs;
  }

  Future<List<String>> _preCopySourcesToOpfs(int opId, List<DataSource> sources) async {
    if (!_needsOpfsPreCopy) return const [];

    final names = <String>[];
    for (var i = 0; i < sources.length; i++) {
      if (sources[i].length > 0) {
        final name = 'pdf_${_instanceId}_${opId}_${i}_${DateTime.now().millisecondsSinceEpoch}';
        await _writeSourceToOpfs(opId, name, sources[i]);
        names.add(name);
      }
    }
    return names;
  }

  Future<void> _writeSourceToOpfs(int opId, String filename, DataSource source) async {
    // Must match SAB_MAX_CHUNK in coordinator.js and read cap in worker.js.
    const chunkSize = 65536;
    final length = source.length;
    var offset = 0;

    while (offset < length) {
      final count = (offset + chunkSize > length) ? length - offset : chunkSize;
      final result = source.readAt(offset, count);
      final Uint8List chunk = result is Future<Uint8List> ? await result : result;

      final ackKey = '$opId:opfs.writeAck';
      final ack = Completer<void>();
      _opfsAcks[ackKey] = ack;

      // Copy before transfer — postMessage with transfer list detaches
      // the source ArrayBuffer. Without the copy, subsequent reads from the
      // same DataSource (e.g. reusing a MemorySource across Pdf instances)
      // would see a zero-length detached buffer.
      final owned = Uint8List.fromList(chunk);
      final jsChunk = owned.buffer.toJS;
      _postRaw({
        'type': 'opfs.write'.toJS,
        'opId': opId.toJS,
        'filename': filename.toJS,
        'chunk': jsChunk,
        'offset': offset.toJS,
      }, [jsChunk]);

      await ack.future;
      offset += count;
    }

    final finalKey = '$opId:opfs.finalizeAck';
    final finalAck = Completer<void>();
    _opfsAcks[finalKey] = finalAck;
    _post({'type': 'opfs.finalize', 'opId': opId});
    await finalAck.future;
  }

  // ═════════════════════════════════════════════════════════════════
  // Outbound message helpers
  // ═════════════════════════════════════════════════════════════════

  JSObject _buildSourceArgs(List<DataSource> sources, List<DataSink> sinks, Set<int> keepSources) {
    final args = JSObject();
    for (var i = 0; i < sources.length; i++) {
      final key = i == 0 ? 'sourceLength' : 'source${i}Length';
      args[key] = sources[i].length.toJS;
    }
    if (sinks.isNotEmpty) {
      args['hasSink'] = true.toJS;
    }
    if (keepSources.isNotEmpty) {
      args['keepSource'] = true.toJS;
    }
    return args;
  }

  void _post(Map<String, Object?> fields) {
    final obj = JSObject();
    for (final e in fields.entries) {
      obj[e.key] = _toJS(e.value);
    }
    _coordinator?.postMessage(obj);
  }

  void _postRaw(Map<String, JSAny?> fields, [List<JSObject>? transfer]) {
    final obj = JSObject();
    for (final e in fields.entries) {
      obj[e.key] = e.value;
    }
    if (transfer != null) {
      _coordinator?.postMessage(obj, transfer.toJS);
    } else {
      _coordinator?.postMessage(obj);
    }
  }

  // ═════════════════════════════════════════════════════════════════
  // Cross-origin worker spawning
  // ═════════════════════════════════════════════════════════════════

  static web.Worker _spawnWorker(String url) {
    if (url.startsWith('http://') || url.startsWith('https://')) {
      final script = "importScripts('${url.replaceAll("'", "\\'")}');";
      final blob = web.Blob(
        [script.toJS].toJS,
        web.BlobPropertyBag(type: 'application/javascript'),
      );
      return web.Worker(web.URL.createObjectURL(blob).toJS);
    }
    return web.Worker(url.toJS);
  }

  // ═════════════════════════════════════════════════════════════════
  // Enum ↔ string at the JS boundary
  // ═════════════════════════════════════════════════════════════════

  static PdfIoMode? _parseIoMode(String? s) => switch (s) {
        'jspi' => PdfIoMode.jspi,
        'atomics' => PdfIoMode.atomics,
        'opfs' => PdfIoMode.opfs,
        _ => null,
      };

  // ═════════════════════════════════════════════════════════════════
  // JS interop helpers
  // ═════════════════════════════════════════════════════════════════

  static JSAny? _toJS(Object? value) => switch (value) {
        null => null,
        final String s => s.toJS,
        final int n => n.toJS,
        final bool b => b.toJS,
        final JSAny js => js,
      };

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
    // Always copy — transferred ArrayBuffers share backing store
    if (v.isA<JSArrayBuffer>()) return Uint8List.fromList((v as JSArrayBuffer).toDart.asUint8List());
    if (v.isA<JSUint8Array>()) return Uint8List.fromList((v as JSUint8Array).toDart);
    return Uint8List(0);
  }
}
