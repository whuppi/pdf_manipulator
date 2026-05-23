// WebBridge — implements PdfBridge for web platforms.
//
// Routes all operations through a coordinator worker which manages the
// WASM worker pool, I/O mode, and read/write/stream routing.
//
// Architecture: Main thread ←→ Coordinator Worker ←→ WASM Worker pool
// Symmetric with native: Main isolate ←→ Worker isolate ←→ Rust thread pool
//
// Uses shared protocol (protocol/) for op names, arg shapes, result parsing.
// The SAME EngineOp enum and builder functions are used by NativeBridge.
//
// INTERNAL — created by bridge_factory.dart.

import 'dart:async';
import 'dart:js_interop';
import 'package:web/web.dart' show EventStreamProviders;
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_sink.dart';
import 'package:pdf_manipulator/src/types/pdf_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/types/pdf_doc.dart';
import 'package:pdf_manipulator/src/protocol/bridge_ops.dart';
import 'package:pdf_manipulator/src/protocol/op.dart';
import 'package:pdf_manipulator/src/protocol/result.dart';

import 'package:web/web.dart' as web;

class WebBridge implements PdfBridge {
  WebBridge({String? coordinatorUrl, String? wasmWorkerUrl})
      : _coordinatorUrl = coordinatorUrl ?? 'pdf_manipulator/coordinator.js',
        _wasmWorkerUrl = wasmWorkerUrl ?? 'pdf_manipulator/wasm_worker.js';

  final String _coordinatorUrl;
  final String _wasmWorkerUrl;
  web.Worker? _coordinator;
  bool _disposed = false;
  bool _ready = false;
  String _ioMode = 'opfs';

  // Per-op tracking
  final _results = <int, Completer<Map<String, Object?>>>{};
  final _streams = <int, StreamController<Map<String, Object?>>>{};
  final _sinks = <int, PdfSink>{};
  final _sources = <int, PdfSource>{};
  final _submitQueue = <Completer<int>>[];
  int _lastOpId = 0;
  Completer<void>? _initCompleter;
  StreamSubscription<web.MessageEvent>? _msgSubscription;

  // ── Coordinator Lifecycle ─────────────────────────────────────────

  Future<void> _ensureReady() async {
    if (_ready) return;
    if (_disposed) throw StateError('WebBridge disposed');

    final completer = Completer<void>();
    _coordinator = web.Worker(_coordinatorUrl.toJS);
    _initCompleter = completer;

    // Use package:web's EventStreamProvider for proper Dart Stream integration.
    // This bridges JS addEventListener into Dart's async event loop correctly,
    // unlike raw onmessage assignment which can miss events during await in dart2js.
    _msgSubscription = EventStreamProviders.messageEvent
        .forTarget(_coordinator!)
        .listen(_handleCoordinatorMessage);

    _coordinator!.onerror = ((web.ErrorEvent e) {
      if (!completer.isCompleted) {
        completer.completeError(Exception('Coordinator failed to start'));
      }
    }).toJS;

    _post('init', {'wasmWorkerUrl': _wasmWorkerUrl});
    await completer.future.timeout(
      const Duration(seconds: 30),
      onTimeout: () => throw StateError(
        'WebBridge init timed out after 30s — coordinator or WASM worker failed to start'),
    );
  }

  // ── Message Handler ───────────────────────────────────────────────

  void _handleCoordinatorMessage(web.MessageEvent e) {
    Map<String, Object?> data;
    String type;
    int opId;
    try {
      data = _jsToMap(e.data as JSAny);
      type = data['type']?.toString() ?? '';
      opId = (data['opId'] is int) ? data['opId'] as int : 0;
    } catch (_) {
      return;
    }

    switch (type) {
      case 'ready':
        _ready = true;
        _ioMode = data['ioMode']?.toString() ?? 'opfs';
        if (_initCompleter != null && !_initCompleter!.isCompleted) {
          _initCompleter!.complete();
        }

      case 'submitted':
        if (_submitQueue.isNotEmpty) {
          _submitQueue.removeAt(0).complete(opId);
        }

      case 'readAt':
        _fulfillRead(data);

      case 'chunk':
        final sink = _sinks[opId];
        if (sink != null) {
          final bytes = data['data'];
          if (bytes is ByteBuffer) {
            sink.write(Uint8List.view(bytes));
          } else if (bytes is Uint8List) {
            sink.write(bytes);
          }
        }

      case 'item':
        final sc = _streams[opId];
        if (sc != null && !sc.isClosed) {
          sc.add(_asStringMap(data['data']));
        }

      case 'itemDone':
        final sc = _streams.remove(opId);
        if (sc != null && !sc.isClosed) sc.close();

      case 'result':
        final c = _results.remove(opId);
        if (c != null && !c.isCompleted) {
          c.complete(_asStringMap(data['result']));
        }
        final sc = _streams.remove(opId);
        if (sc != null && !sc.isClosed) sc.close();
        _sinks.remove(opId);

      case 'error':
        final msg = data['error'] as String? ?? 'Unknown error';
        // Error might arrive before 'submitted' (e.g. worker acquire failed).
        // Complete the submit queue completer if there's one pending.
        if (_submitQueue.isNotEmpty) {
          _submitQueue.removeAt(0).completeError(Exception(msg));
        }
        final c = _results.remove(opId);
        if (c != null && !c.isCompleted) c.completeError(Exception(msg));
        final sc = _streams.remove(opId);
        if (sc != null && !sc.isClosed) {
          sc.addError(Exception(msg));
          sc.close();
        }
        _sinks.remove(opId);

      case 'opfs.writeAck':
        if (_opfsAckCompleters.isNotEmpty) {
          _opfsAckCompleters.removeAt(0).complete();
        }

      case 'opfs.finalizeAck':
        if (_opfsFinalizeCompleters.isNotEmpty) {
          _opfsFinalizeCompleters.removeAt(0).complete();
        }
    }
  }

  Future<void> _fulfillRead(Map<String, Object?> data) async {
    final readId = data['readId'] as String;
    final offset = data['offset'] as int;
    final count = data['count'] as int;
    final opId = data['opId'] as int;

    final source = _sources[opId];
    if (source == null) {
      _post('readAtResponse', {'readId': readId, 'error': 'No source for opId $opId'});
      return;
    }

    try {
      final bytes = await source.readAt(offset, count);
      _post('readAtResponse', {
        'readId': readId,
        'bytes': bytes.buffer,
      });
    } catch (e) {
      _post('readAtResponse', {'readId': readId, 'error': e.toString()});
    }
  }

  // ── Submit (shared transport) ─────────────────────────────────────

  /// Submit an EngineRequest. Returns the result map.
  // Pre-register source before submitting so readAt can be fulfilled
  // immediately when the WASM worker requests bytes. The coordinator
  // assigns opId monotonically starting from 1.
  int _nextExpectedOpId = 1;

  int _nextOpfsId = 0;

  Future<Map<String, Object?>> _submit(
    EngineRequest req, {
    PdfSource? source,
    PdfSink? sink,
  }) async {
    _checkDisposed();
    await _ensureReady();

    String? opfsFile;

    // Atomics mode: no pre-copy needed — reads go through SharedArrayBuffer.
    // Atomics mode: no pre-copy needed — reads go through SAB + Atomics.wait.
    // OPFS mode: pre-copy source to OPFS disk, engine reads via SyncAccessHandle.
    if (source != null && _ioMode == 'opfs') {
      opfsFile = '_pdf_op_${_nextOpfsId++}.tmp';
      await _streamSourceToOpfs(source, opfsFile);
    }

    // Pre-register source for Atomics readAt fulfillment
    final expectedOpId = _nextExpectedOpId++;
    if (source != null) _sources[expectedOpId] = source;
    if (sink != null) _sinks[expectedOpId] = sink;

    final sc = Completer<int>();
    _submitQueue.add(sc);

    final args = Map<String, Object?>.from(req.args);
    if (source != null) args['sourceLength'] = source.length;

    _post('submit', {
      'op': req.op.wire,
      'args': args,
      if (opfsFile != null) 'opfsFile': opfsFile,
    });

    final opId = await sc.future.timeout(const Duration(seconds: 15),
      onTimeout: () => throw StateError('submit queue timed out'));
    _lastOpId = opId;

    if (opId != expectedOpId) {
      if (source != null) {
        _sources[opId] = _sources.remove(expectedOpId)!;
      }
      if (sink != null) {
        _sinks[opId] = _sinks.remove(expectedOpId)!;
      }
    }

    final rc = Completer<Map<String, Object?>>();
    _results[opId] = rc;

    try {
      return await rc.future.timeout(
        const Duration(seconds: 15),
        onTimeout: () => throw StateError(
          'WebBridge op ${req.op.wire} timed out after 15s (opId=$opId)'),
      );
    } finally {
      _sources.remove(opId);
      _sinks.remove(opId);
    }
  }

  /// Stream PdfSource bytes to coordinator's OPFS file (mode 3 only).
  Future<void> _streamSourceToOpfs(PdfSource source, String filename) async {
    const chunkSize = 256 * 1024; // 256KB chunks
    var offset = 0;
    final total = source.length;
    while (offset < total) {
      final count = (total - offset).clamp(0, chunkSize);
      final bytes = await source.readAt(offset, count);
      final ack = Completer<void>();
      _opfsAckCompleters.add(ack);
      _post('opfs.write', {
        'opId': 0,
        'filename': filename,
        'chunk': bytes.buffer,
        'offset': offset,
      });
      await ack.future.timeout(const Duration(seconds: 10),
        onTimeout: () => throw StateError('OPFS writeAck timed out'));
      offset += count;
    }
    final finalAck = Completer<void>();
    _opfsFinalizeCompleters.add(finalAck);
    _post('opfs.finalize', {'opId': 0});
    await finalAck.future.timeout(const Duration(seconds: 10),
      onTimeout: () => throw StateError('OPFS finalizeAck timed out'));
  }

  final _opfsAckCompleters = <Completer<void>>[];
  final _opfsFinalizeCompleters = <Completer<void>>[];

  /// Submit with source + sink. Fire and forget the result.
  Future<void> _submitEdit(
    EngineRequest req,
    PdfSource source,
    PdfSink sink,
  ) async {
    await _submit(req, source: source, sink: sink);
  }

  /// Submit a streaming op. Yields items until done.
  /// Uses a regular async helper for OPFS pre-copy, then returns the stream.
  Stream<Map<String, Object?>> _submitStream(
    EngineRequest req,
    PdfSource source,
  ) {
    // Can't use async* here — OPFS pre-copy needs the event loop free for
    // ack messages, and async* generators in dart2js may not yield to JS events
    // during await. Instead, use a StreamController and wire it up from a
    // regular async function.
    final controller = StreamController<Map<String, Object?>>();
    _submitStreamAsync(req, source, controller);
    return controller.stream;
  }

  Future<void> _submitStreamAsync(
    EngineRequest req,
    PdfSource source,
    StreamController<Map<String, Object?>> controller,
  ) async {
    try {
      _checkDisposed();
      await _ensureReady();

      String? opfsFile;
      if (_ioMode == 'opfs') {
        opfsFile = '_pdf_op_${_nextOpfsId++}.tmp';
        await _streamSourceToOpfs(source, opfsFile);
      }

      final expectedOpId = _nextExpectedOpId++;
      _sources[expectedOpId] = source;

      final sc = Completer<int>();
      _submitQueue.add(sc);

      final args = Map<String, Object?>.from(req.args);
      args['sourceLength'] = source.length;

      _post('submit', {
        'op': req.op.wire,
        'args': args,
        if (opfsFile != null) 'opfsFile': opfsFile,
      });

      final opId = await sc.future.timeout(const Duration(seconds: 15),
      onTimeout: () => throw StateError('submit queue timed out'));
      if (opId != expectedOpId) {
        _sources[opId] = _sources.remove(expectedOpId)!;
      }

      _streams[opId] = controller;
      _results[opId] = Completer<Map<String, Object?>>();

      // Stream items arrive via _handleCoordinatorMessage → 'item' / 'itemDone'
      // The controller is closed when 'itemDone' or 'result' arrives.
      await _results[opId]!.future;
    } catch (e) {
      if (!controller.isClosed) {
        controller.addError(e);
        controller.close();
      }
    } finally {
      _sources.remove(_lastOpId);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // PdfBridge implementation — uses shared protocol builders + parsers
  // ══════════════════════════════════════════════════════════════════

  // ── Inspect ──

  @override
  Future<PdfDoc> open(PdfSource source, {String? password}) async {
    final r = await _submit(openOp(password: password), source: source);
    return parseOpenResult(r);
  }

  // ── Structural ──

  @override
  Future<void> merge(List<PdfSource> inputs, PdfSink output) async {
    if (inputs.isEmpty) throw ArgumentError('inputs must not be empty');
    if (inputs.length == 1) {
      return extractPages(inputs[0], output,
          pages: List.generate((await open(inputs[0])).pageCount, (i) => i));
    }
    final secondaries = <ByteBuffer>[];
    for (var i = 1; i < inputs.length; i++) {
      final bytes = await inputs[i].readAt(0, inputs[i].length);
      secondaries.add(bytes.buffer);
    }
    await _submitEdit(mergeOp(secondaries: secondaries), inputs[0], output);
  }

  @override
  Future<void> split(PdfSource source, PdfSink Function(int) sinkFactory,
      {required int every}) async {
    _checkDisposed();
    if (every < 1) throw ArgumentError('every must be >= 1');
    final doc = await open(source);
    var chunkIndex = 0;
    for (var start = 0; start < doc.pageCount; start += every) {
      final end = (start + every).clamp(0, doc.pageCount);
      final pages = List.generate(end - start, (i) => start + i);
      await extractPages(source, sinkFactory(chunkIndex), pages: pages);
      chunkIndex++;
    }
  }

  @override
  Future<int> splitBySize(PdfSource source, PdfSink Function(int) sinkFactory,
      {required int maxBytes}) async {
    _checkDisposed();
    if (maxBytes < 1) throw ArgumentError('maxBytes must be >= 1');
    final doc = await open(source);
    var chunkIndex = 0;
    var chunkPages = <int>[];
    for (var i = 0; i < doc.pageCount; i++) {
      chunkPages.add(i);
      final trial = _ByteCounter();
      await extractPages(source, trial, pages: chunkPages);
      if (trial.length > maxBytes && chunkPages.length > 1) {
        chunkPages.removeLast();
        await extractPages(source, sinkFactory(chunkIndex), pages: chunkPages);
        chunkIndex++;
        chunkPages = [i];
      }
    }
    if (chunkPages.isNotEmpty) {
      await extractPages(source, sinkFactory(chunkIndex), pages: chunkPages);
      chunkIndex++;
    }
    return chunkIndex;
  }

  @override
  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages}) =>
    _submitEdit(extractPagesOp(pages: pages), source, output);

  @override
  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages}) =>
    _submitEdit(deletePagesOp(pages: pages), source, output);

  @override
  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order}) =>
    _submitEdit(reorderPagesOp(order: order), source, output);

  @override
  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to}) =>
    _submitEdit(movePageOp(from: from, to: to), source, output);

  @override
  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages}) =>
    _submitEdit(rotatePagesOp(rotations: pages), source, output);

  @override
  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees}) =>
    _submitEdit(rotateAllPagesOp(degrees: degrees), source, output);

  // ── Content ──

  @override
  Future<void> flattenForms(PdfSource source, PdfSink output) =>
    _submitEdit(flattenFormsOp(), source, output);

  @override
  Future<void> applyRedactions(PdfSource source, PdfSink output) =>
    _submitEdit(applyRedactionsOp(), source, output);

  @override
  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData}) =>
    _submitEdit(embedFileOp(name: name, fileData: fileData), source, output);

  @override
  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions}) =>
    _submitEdit(eraseRegionsOp(page: page, regions: regions), source, output);

  @override
  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75, bool garbageCollect = true,
       bool linearize = false}) =>
    _submitEdit(compressOp(
      imageQuality: imageQuality,
      garbageCollect: garbageCollect,
      linearize: linearize,
    ), source, output);

  // ── Extraction ──

  @override
  Future<String> extract(PdfSource source,
      {required PdfPages pages, String? password,
       PdfExtractionFormat format = PdfExtractionFormat.auto}) async {
    final pageIndex = switch (pages) {
      PdfSinglePage(:final index) => index,
      _ => null,
    };
    final r = await _submit(
      extractOp(format: format, page: pageIndex, password: password),
      source: source,
    );
    return r['text'] as String? ?? '';
  }

  // ── Search ──

  @override
  Future<List<SearchResult>> search(PdfSource source,
      {required String query, required PdfPages pages, String? password}) async {
    final pageIndex = switch (pages) {
      PdfSinglePage(:final index) => index,
      _ => null,
    };
    final r = await _submit(
      searchOp(query: query, page: pageIndex, password: password),
      source: source,
    );
    return parseSearchResults(r);
  }

  // ── Security ──

  @override
  Future<void> watermark(PdfSource source, PdfSink output,
      {required String text, PdfPages pages = const PdfPages.all(),
       PdfWatermarkStyle style = const PdfWatermarkStyle(),
       PdfWatermarkPosition? position}) =>
    _submitEdit(watermarkOp(text: text, style: style), source, output);

  @override
  Future<void> encrypt(PdfSource source, PdfSink output,
      {required PdfEncryptionConfig encryption}) =>
    _submitEdit(encryptOp(encryption: encryption), source, output);

  @override
  Future<void> decrypt(PdfSource source, PdfSink output,
      {required String password}) =>
    _submitEdit(decryptOp(password: password), source, output);

  @override
  Future<void> sign(PdfSource source, PdfSink output,
      {required PdfSigningCredentials credentials,
       String? reason, String? location}) =>
    _submitEdit(signOp(
      credentials: credentials,
      reason: reason,
      location: location,
    ), source, output);

  // ── Stamps ──

  @override
  Future<void> addStamp(PdfSource source, PdfSink output,
      {required int page, required PdfStampType type, required PdfRect rect,
       String? customName, double opacity = 1.0}) =>
    _submitEdit(addStampOp(page: page, type: type, rect: rect, opacity: opacity), source, output);

  @override
  Future<void> addImageStamp(PdfSource source, PdfSink output,
      {required int page, required Uint8List imageBytes, required PdfRect rect,
       double opacity = 1.0}) =>
    _submitEdit(addImageStampOp(
      page: page, imageBytes: imageBytes, rect: rect, opacity: opacity,
    ), source, output);

  // ── Creation ──

  @override
  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output) async {
    await _submit(imagesToPdfOp(images: images), sink: output);
  }

  // ── Rendering ──

  @override
  Stream<RenderedPage> render(PdfSource source,
      {required PdfPages pages, PdfRenderSize? size, String? password}) async* {
    final pageList = await _resolvePages(source, pages, password: password);
    yield* _submitStream(
      renderOp(
        pageIndices: pageList,
        maxWidth: size?.maxWidth,
        maxHeight: size?.maxHeight,
        password: password,
      ),
      source,
    ).map(parseRenderedPage);
  }

  // ── Image extraction ──

  @override
  Stream<PdfImage> extractImages(PdfSource source,
      {required PdfPages pages, String? password}) async* {
    final pageList = await _resolvePages(source, pages, password: password);
    yield* _submitStream(
      extractImagesOp(pageIndices: pageList, password: password),
      source,
    ).map(parsePdfImage);
  }

  // ── Signatures ──

  @override
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
      {String? password}) async {
    final r = await _submit(getSignaturesOp(password: password), source: source);
    return parseSignatures(r);
  }

  @override
  Future<bool> verifySignatures(PdfSource source, {String? password}) async {
    final r = await _submit(verifySignaturesOp(password: password), source: source);
    return r['valid'] as bool? ?? false;
  }

  // ── Validation ──

  @override
  Future<PdfValidationResult> validatePdfA(PdfSource source,
      {int level = 2, String? password}) async {
    final r = await _submit(validatePdfAOp(level: level, password: password), source: source);
    return parseValidationResult(r);
  }

  @override
  Future<bool> validatePdfUa(PdfSource source,
      {int level = 1, String? password}) async {
    final r = await _submit(validatePdfUaOp(level: level, password: password), source: source);
    return r['accessible'] as bool? ?? false;
  }

  // ── Editor + Builder ──

  @override
  Future<BridgeEditorHandle> openEditor(PdfSource source, {String? password}) async {
    final r = await _submit(editorOpenOp(password: password), source: source);
    return _WebEditorHandle(this, r['handleId'] as int);
  }

  @override
  Future<BridgeBuilderHandle> createBuilder() async {
    final r = await _submit(builderCreateOp());
    return _WebBuilderHandle(this, r['handleId'] as int);
  }

  // ── Lifecycle ──

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _msgSubscription?.cancel();
    _msgSubscription = null;
    if (_coordinator != null) {
      _post('dispose', {});
      await Future<void>.delayed(const Duration(milliseconds: 100));
      _coordinator!.terminate();
      _coordinator = null;
    }
    for (final c in _results.values) {
      if (!c.isCompleted) c.completeError(StateError('WebBridge disposed'));
    }
    _results.clear();
    for (final sc in _streams.values) {
      if (!sc.isClosed) sc.close();
    }
    _streams.clear();
    _sinks.clear();
    _sources.clear();
  }

  // ── Helpers ──

  Future<List<int>> _resolvePages(PdfSource source, PdfPages pages,
      {String? password}) async {
    final doc = await open(source, password: password);
    return resolvePageIndices(pages, doc.pageCount);
  }

  void _checkDisposed() {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
  }

  void _post(String type, Map<String, Object?> data) {
    if (_coordinator == null) return;
    // Build JS message and collect transferable ArrayBuffers in one pass.
    // Each ByteBuffer is converted to JSArrayBuffer ONCE and the SAME
    // reference goes into both the message and the transfer list.
    final transfers = <JSObject>[];
    final msg = JSObject();
    msg['type'] = type.toJS;
    for (final e in data.entries) {
      msg[e.key] = _toJSWithTransfers(e.value, transfers);
    }
    if (transfers.isNotEmpty) {
      _coordinator!.postMessage(msg, transfers.toJS);
    } else {
      _coordinator!.postMessage(msg);
    }
  }

  static Map<String, Object?> _jsToMap(JSAny? value) {
    if (value == null) return {};
    final d = value.dartify();
    if (d is Map) return d.map((k, v) => MapEntry(k.toString(), v));
    return {};
  }

  static Map<String, Object?> _asStringMap(Object? value) {
    if (value is Map) return value.map((k, v) => MapEntry(k.toString(), v));
    return {};
  }

  /// Convert a Dart value to JS AND collect transferable ArrayBuffers.
  /// Only TOP-LEVEL ByteBuffer/Uint8List values are transferred.
  /// Values inside Lists are cloned (not transferred) to avoid JS
  /// object identity issues where .toJS on a List creates new wrappers
  /// that don't match the transfer list references.
  static JSAny? _toJSWithTransfers(Object? value, List<JSObject> transfers) {
    if (value == null) return null;
    if (value is ByteBuffer) {
      return value.toJS;
    }
    if (value is Uint8List) {
      return value.buffer.toJS;
    }
    if (value is Map) {
      final obj = JSObject();
      for (final e in value.entries) {
        obj[e.key.toString()] = _toJSWithTransfers(e.value, transfers);
      }
      return obj;
    }
    if (value is List) {
      final arr = JSArray<JSAny?>();
      for (final v in value) {
        arr.add(_toJSWithTransfers(v, transfers));
      }
      return arr;
    }
    return value.jsify();
  }

}

// ══════════════════════════════════════════════════════════════════════
// Editor Handle — uses shared protocol builders + parsers
// ══════════════════════════════════════════════════════════════════════

class _WebEditorHandle implements BridgeEditorHandle {
  _WebEditorHandle(this._b, this._hid);
  final WebBridge _b;
  final int _hid;

  Future<Map<String, Object?>> _op(EngineRequest req) => _b._submit(req);

  @override Future<int> get pageCount async =>
      parseEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).pageCount;
  @override Future<String> get version async =>
      parseEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).version;

  @override Future<String> getTitle() async =>
      parseEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).title;
  @override Future<void> setTitle(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setTitle', extra: {'value': value}));
  @override Future<String> getAuthor() async =>
      parseEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).author;
  @override Future<void> setAuthor(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setAuthor', extra: {'value': value}));
  @override Future<String> getSubject() async =>
      parseEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).subject;
  @override Future<void> setSubject(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setSubject', extra: {'value': value}));
  @override Future<String> getKeywords() async =>
      parseEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).keywords;
  @override Future<void> setKeywords(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setKeywords', extra: {'value': value}));

  @override Future<void> rotatePage(int page, {required int degrees}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'rotatePages', extra: {'rotations': {page: degrees}}));
  @override Future<void> rotateAllPages({required int degrees}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'rotateAllPages', extra: {'degrees': degrees}));
  @override Future<PdfRect> getPageMediaBox(int page) async =>
      parseMediaBox(await _op(editorPageMediaBoxOp(handleId: _hid, page: page)));
  @override Future<void> deletePage(int page) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'deletePages', extra: {'pages': [page]}));
  @override Future<void> movePage({required int from, required int to}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'movePage', extra: {'from': from, 'to': to}));

  @override Future<void> extractPages(List<int> pages, PdfSink output) async {
    await _b._submit(editorExtractPagesOp(handleId: _hid, pages: pages), sink: output);
  }

  @override Future<void> mergeFrom(PdfSource otherPdf) async {
    final bytes = await otherPdf.readAt(0, otherPdf.length);
    await _op(editorMergeFromOp(handleId: _hid, otherBytes: Uint8List.view(bytes.buffer)));
  }

  @override Future<int> optimizeImages({int quality = 75}) async =>
      (await _op(editorMutateOp(handleId: _hid, editOp: 'compress', extra: {'imageQuality': quality})))['value'] as int? ?? 0;
  @override Future<int> unembedStandardFonts() async =>
      (await _op(editorMutateOp(handleId: _hid, editOp: 'unembedStandardFonts')))['value'] as int? ?? 0;

  @override Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  }) => _op(editorMutateOp(handleId: _hid, editOp: 'watermark', extra: {
    'page': page, ...encodeWatermarkArgs(text, style),
  }));

  @override Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect,
    String? customName, double opacity = 1.0,
  }) => _op(editorMutateOp(handleId: _hid, editOp: 'addStamp', extra: {
    'page': page, 'stampType': type.index, ...encodeRectArgs(rect), 'opacity': opacity,
  }));

  @override Future<void> addImageStamp(int page, Uint8List imageBytes, {
    required PdfRect rect, double opacity = 1.0,
  }) => _op(editorMutateOp(handleId: _hid, editOp: 'addImageStamp', extra: {
    'page': page, 'imageBytes': imageBytes, ...encodeRectArgs(rect), 'opacity': opacity,
  }));

  @override Future<void> embedFile(String name, Uint8List data) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'embedFile', extra: {'name': name, 'fileData': data}));
  @override Future<void> eraseRegions(int page, List<PdfRect> regions) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'eraseRegions', extra: {
        'page': page, 'regions': encodeRegions(regions),
      }));
  @override Future<void> flattenForms() =>
      _op(editorMutateOp(handleId: _hid, editOp: 'flattenForms'));
  @override Future<void> flattenAllAnnotations() =>
      _op(editorMutateOp(handleId: _hid, editOp: 'flattenAllAnnotations'));
  @override Future<void> setFormFieldValue(String fieldName, String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setFormFieldValue', extra: {'fieldName': fieldName, 'value': value}));
  @override Future<void> cropMargins({double left = 0, double right = 0,
      double top = 0, double bottom = 0}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'cropMargins', extra: {
        'left': left, 'right': right, 'top': top, 'bottom': bottom,
      }));
  @override Future<void> convertToPdfA({int level = 1}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'convertToPdfA', extra: {'level': level}));
  @override Future<void> resizeImage(int page, String imageName,
      {required double width, required double height}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'resizeImage', extra: {
        'page': page, 'imageName': imageName, 'width': width, 'height': height,
      }));

  @override Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
    await _b._submit(editorSaveOp(handleId: _hid, options: options), sink: output);
  }

  @override Future<void> dispose() => _op(editorDisposeOp(handleId: _hid));
}

// ══════════════════════════════════════════════════════════════════════
// Builder Handle — uses shared protocol builders
// ══════════════════════════════════════════════════════════════════════

class _WebBuilderHandle implements BridgeBuilderHandle {
  _WebBuilderHandle(this._b, this._hid);
  final WebBridge _b;
  final int _hid;

  Future<Map<String, Object?>> _op(EngineRequest req) => _b._submit(req);

  @override Future<void> setTitle(String value) =>
      _op(builderSetMetadataOp(handleId: _hid, title: value));
  @override Future<void> setAuthor(String value) =>
      _op(builderSetMetadataOp(handleId: _hid, author: value));
  @override Future<void> setSubject(String value) =>
      _op(builderSetMetadataOp(handleId: _hid, subject: value));
  @override Future<void> setKeywords(String value) =>
      _op(builderSetMetadataOp(handleId: _hid, keywords: value));

  @override Future<BridgePageBuilderHandle> addA4Page() async {
    final r = await _op(builderAddPageOp(handleId: _hid, pageType: 'a4'));
    return _WebPageHandle(_b, r['handleId'] as int);
  }
  @override Future<BridgePageBuilderHandle> addLetterPage() async {
    final r = await _op(builderAddPageOp(handleId: _hid, pageType: 'letter'));
    return _WebPageHandle(_b, r['handleId'] as int);
  }
  @override Future<BridgePageBuilderHandle> addPage({required double width, required double height}) async {
    final r = await _op(builderAddPageOp(handleId: _hid, width: width, height: height));
    return _WebPageHandle(_b, r['handleId'] as int);
  }

  @override Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
    await _b._submit(builderSaveOp(handleId: _hid), sink: output);
  }

  @override Future<void> dispose() => _op(builderDisposeOp(handleId: _hid));
}

// ══════════════════════════════════════════════════════════════════════
// Page Builder Handle — uses shared protocol builders
// ══════════════════════════════════════════════════════════════════════

class _WebPageHandle implements BridgePageBuilderHandle {
  _WebPageHandle(this._b, this._hid);
  final WebBridge _b;
  final int _hid;

  Future<Map<String, Object?>> _pgOp(String op, [Map<String, Object?> extra = const {}]) =>
      _b._submit(builderPageOpReq(handleId: _hid, pageOp: op, extra: extra));

  @override Future<void> font(String name, double size) => _pgOp('font', {'name': name, 'size': size});
  @override Future<void> at(double x, double y) => _pgOp('at', {'x': x, 'y': y});
  @override Future<void> text(String text) => _pgOp('text', {'text': text});
  @override Future<void> heading(int level, String text) => _pgOp('heading', {'level': level, 'text': text});
  @override Future<void> paragraph(String text) => _pgOp('paragraph', {'text': text});
  @override Future<void> space(double points) => _pgOp('space', {'points': points});
  @override Future<void> horizontalRule() => _pgOp('horizontalRule');
  @override Future<void> image(Uint8List imageBytes, PdfRect rect, {String altText = ''}) =>
      _pgOp('image', {'imageBytes': imageBytes, ...encodeRectArgs(rect), 'altText': altText});
  @override Future<void> watermark(String text) => _pgOp('watermark', {'text': text});
  @override Future<void> textField(String name, PdfRect rect, {String? defaultValue}) =>
      _pgOp('textField', {'name': name, ...encodeRectArgs(rect), 'defaultValue': defaultValue});
  @override Future<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _pgOp('checkbox', {'name': name, ...encodeRectArgs(rect), 'checked': checked});
  @override Future<void> comboBox(String name, PdfRect rect, List<String> options, {String? selected}) =>
      _pgOp('comboBox', {'name': name, ...encodeRectArgs(rect), 'options': options, 'selected': selected});
  @override Future<void> pushButton(String name, PdfRect rect, String caption) =>
      _pgOp('pushButton', {'name': name, ...encodeRectArgs(rect), 'caption': caption});
  @override Future<void> signatureField(String name, PdfRect rect) =>
      _pgOp('signatureField', {'name': name, ...encodeRectArgs(rect)});
  @override Future<void> radioGroup(String name,
      List<({String value, PdfRect rect})> options, {String? selected}) =>
      _pgOp('radioGroup', {
        'name': name,
        'values': options.map((o) => o.value).toList(),
        'xs': options.map((o) => o.rect.x).toList(),
        'ys': options.map((o) => o.rect.y).toList(),
        'ws': options.map((o) => o.rect.width).toList(),
        'hs': options.map((o) => o.rect.height).toList(),
        'selected': selected,
      });
  @override Future<void> fieldKeystroke(String script) => _pgOp('fieldKeystroke', {'script': script});
  @override Future<void> fieldFormat(String script) => _pgOp('fieldFormat', {'script': script});
  @override Future<void> fieldValidate(String script) => _pgOp('fieldValidate', {'script': script});
  @override Future<void> fieldCalculate(String script) => _pgOp('fieldCalculate', {'script': script});
  @override Future<void> linkUrl(String url) => _pgOp('linkUrl', {'url': url});
  @override Future<void> linkPage(int targetPage) => _pgOp('linkPage', {'targetPage': targetPage});
  @override Future<void> footnote(String refMark, String noteText) =>
      _pgOp('footnote', {'refMark': refMark, 'noteText': noteText});
  @override Future<void> columns(int columnCount, double gapPt, String text) =>
      _pgOp('columns', {'columnCount': columnCount, 'gapPt': gapPt, 'text': text});
  @override Future<void> newline() => _pgOp('newline');
  @override Future<void> newPageSameSize() => _pgOp('newPageSameSize');
  @override Future<void> done() => _b._submit(builderPageDoneOp(handleId: _hid));
}

// ── Internal helpers ──

class _ByteCounter implements PdfSink {
  int length = 0;
  @override
  void write(Uint8List data) { length += data.length; }
}
