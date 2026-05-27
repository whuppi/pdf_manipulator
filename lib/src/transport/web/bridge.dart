// WebBridge — extends PdfBridge for web platforms.
//
// RULE: Encode request → send to coordinator → decode result. Zero PDF
// logic. Does not interpret results beyond wire decoding. Does not make
// decisions based on PDF content. Symmetric with native/bridge.dart —
// both bridges must encode the same args for the same ops.
//
// VIOLATIONS:
// - No PDF logic (page routing, format detection).
// - No direct WASM calls (worker.js handles those).
// - No conditional behavior that native/bridge.dart doesn't also have.
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
// Do NOT import WASM bindings (pdf_oxide.js) or call WASM methods directly.
// All engine calls go through postMessage to the coordinator.
// The absence of WASM imports makes direct engine access impossible.
//
// INTERNAL — created by bridge_factory.dart.

import 'dart:async';
import 'dart:js_interop';
import 'package:web/web.dart' show EventStreamProviders;
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/types/pdf_doc.dart';
import 'package:pdf_manipulator/src/transport/protocol/codec.dart' hide decodeOpenResult, decodeExtractResult, decodeSearchResults, decodeSignatures, decodeVerifySignatures, decodeValidationResult, decodeValidatePdfUa, decodeBookmarkSplits, decodeClassifyPage, decodeClassifyDocument, decodeRenderedPage, decodePdfImage, decodeEditorMetadata;
import 'package:pdf_manipulator/src/transport/protocol/op.dart';
import 'package:pdf_manipulator/src/transport/web/wire.dart' as wire;
import 'package:pdf_manipulator/src/version.dart';

import 'package:web/web.dart' as web;

class WebBridge extends PdfBridge {
  WebBridge({String? coordinatorUrl, String? workerUrl})
      : _coordinatorUrl = coordinatorUrl ?? 'pdf_manipulator/coordinator.js',
        _workerUrl = workerUrl ?? 'pdf_manipulator/worker.js';

  final String _coordinatorUrl;
  final String _workerUrl;
  web.Worker? _coordinator;
  bool _disposed = false;
  bool _ready = false;
  String _ioMode = 'opfs';

  // Per-op tracking
  final _results = <int, Completer<Map<String, Object?>>>{};
  final _streams = <int, StreamController<Map<String, Object?>>>{};
  final _sinks = <int, DataSink>{};
  final _sources = <int, DataSource>{};
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

    _post('init', {'workerUrl': _workerUrl});
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
        // Version guard: detect stale web assets from a previous package version
        final webVersion = data['version']?.toString() ?? '';
        if (webVersion == '__VERSION__' || webVersion.isEmpty) {
          throw StateError(
            'pdf_manipulator web assets not set up. '
            'Run: dart run pdf_manipulator:setup');
        }
        if (webVersion != packageVersion) {
          throw StateError(
            'pdf_manipulator web assets are v$webVersion but package is v$packageVersion. '
            'Run: dart run pdf_manipulator:setup --force');
        }
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
        final bytes = data['data'];
        if (sink != null) {
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
    DataSource? source,
    DataSink? sink,
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

  /// Stream DataSource bytes to coordinator's OPFS file (mode 3 only).
  Future<void> _streamSourceToOpfs(DataSource source, String filename) async {
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
    DataSource source,
    DataSink sink,
  ) async {
    await _submit(req, source: source, sink: sink);
  }

  /// Submit a streaming op. Yields items until done.
  /// Uses a regular async helper for OPFS pre-copy, then returns the stream.
  Stream<Map<String, Object?>> _submitStream(
    EngineRequest req,
    DataSource source,
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
    DataSource source,
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
  Future<PdfDoc> open(DataSource source, {String? password}) async {
    final r = await _submit(openOp(password: password), source: source);
    return wire.wireDecodeOpen(r);
  }

  // ── Extraction ──

  @override
  Future<String> extract(DataSource source,
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
    return wire.wireDecodeText(r);
  }

  // ── Search ──

  @override
  Future<List<SearchResult>> search(DataSource source,
      {required String query, required PdfPages pages, String? password}) async {
    final pageIndex = switch (pages) {
      PdfSinglePage(:final index) => index,
      _ => null,
    };
    final r = await _submit(
      searchOp(query: query, page: pageIndex, password: password),
      source: source,
    );
    return wire.wireDecodeSearch(r);
  }

  // ── Security ──

  @override
  Future<void> sign(DataSource source, DataSink output,
      {required PdfSigningCredentials credentials,
       String? reason, String? location}) =>
    _submitEdit(signOp(
      credentials: credentials,
      reason: reason,
      location: location,
    ), source, output);

  // ── Creation ──

  @override
  Future<void> imagesToPdf(List<DataSource> images, DataSink output) async {
    final imageBytes = await Future.wait(images.map(readAllBytes));
    await _submit(imagesToPdfOp(images: imageBytes), sink: output);
  }

  // ── Rendering ──

  @override
  Stream<RenderedPage> render(DataSource source,
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
    ).map(wire.wireDecodeRenderedPage);
  }

  // ── Image extraction ──

  @override
  Stream<PdfImage> extractImages(DataSource source,
      {required PdfPages pages, String? password}) async* {
    final pageList = await _resolvePages(source, pages, password: password);
    yield* _submitStream(
      extractImagesOp(pageIndices: pageList, password: password),
      source,
    ).map(wire.wireDecodeImage);
  }

  // ── Signatures ──

  @override
  Future<List<PdfSignatureInfo>> getSignatures(DataSource source,
      {String? password}) async {
    final r = await _submit(getSignaturesOp(password: password), source: source);
    return wire.wireDecodeSignatures(r);
  }

  @override
  Future<bool> verifySignatures(DataSource source, {String? password}) async {
    final r = await _submit(verifySignaturesOp(password: password), source: source);
    return wire.wireDecodeVerifySignatures(r);
  }

  // ── Validation ──

  @override
  Future<PdfValidationResult> validatePdfA(DataSource source,
      {int level = 2, String? password}) async {
    final r = await _submit(validatePdfAOp(level: level, password: password), source: source);
    return wire.wireDecodeValidation(r);
  }

  @override
  Future<bool> validatePdfUa(DataSource source,
      {int level = 1, String? password}) async {
    final r = await _submit(validatePdfUaOp(level: level, password: password), source: source);
    return wire.wireDecodeValidatePdfUa(r);
  }

  @override
  Future<List<PdfBookmarkSplit>> planSplitByBookmarks(DataSource source, {String? password}) async {
    final r = await _submit(planSplitByBookmarksOp(password: password), source: source);
    return wire.wireDecodeBookmarkSplits(r);
  }


  @override
  Future<PdfPageClassification> classifyPage(DataSource source, int page, {String? password}) async {
    final r = await _submit(classifyPageOp(page: page, password: password), source: source);
    return wire.wireDecodeClassifyPage(r);
  }

  @override
  Future<PdfDocumentClassification> classifyDocument(DataSource source, {String? password}) async {
    final r = await _submit(classifyDocumentOp(password: password), source: source);
    return wire.wireDecodeClassifyDocument(r);
  }

  @override
  Future<void> convertTo(DataSource source, DataSink output,
      {required PdfDocumentFormat format, String? password}) =>
    _submitEdit(convertToOp(format: format, password: password), source, output);

  @override
  Future<void> convertToPdf(DataSource document, DataSink output,
      {required PdfDocumentFormat format}) =>
    _submitEdit(convertToPdfOp(format: format), document, output);

  // ── Editor + Builder ──

  @override
  Future<BridgeEditorHandle> openEditor(DataSource source, {String? password}) async {
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

  Future<List<int>> _resolvePages(DataSource source, PdfPages pages,
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
      wire.wireDecodeEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).pageCount;
  @override Future<String> get version async =>
      wire.wireDecodeEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).version;
  @override Future<bool> get isModified async {
    final r = await _op(EngineRequest(EngineOp.editorIsModified, {'handleId': _hid}));
    return (r as Map<String, dynamic>?)?['modified'] == true;
  }

  @override Future<String> getTitle() async =>
      wire.wireDecodeEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).title;
  @override Future<void> setTitle(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setTitle', extra: {'value': value}));
  @override Future<String> getAuthor() async =>
      wire.wireDecodeEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).author;
  @override Future<void> setAuthor(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setAuthor', extra: {'value': value}));
  @override Future<String> getSubject() async =>
      wire.wireDecodeEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).subject;
  @override Future<void> setSubject(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setSubject', extra: {'value': value}));
  @override Future<String> getKeywords() async =>
      wire.wireDecodeEditorMetadata(await _op(editorGetMetadataOp(handleId: _hid))).keywords;
  @override Future<void> setKeywords(String value) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'setKeywords', extra: {'value': value}));

  @override Future<void> rotatePage(int page, {required int degrees}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'rotatePages', extra: {'rotations': {page: degrees}}));
  @override Future<void> rotateAllPages({required int degrees}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'rotateAllPages', extra: {'degrees': degrees}));
  @override Future<PdfRect> getPageMediaBox(int page) async =>
      decodeMediaBox(await _op(editorPageMediaBoxOp(handleId: _hid, page: page)));
  @override Future<void> deletePage(int page) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'deletePages', extra: {'pages': [page]}));
  @override Future<void> movePage({required int from, required int to}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'movePage', extra: {'from': from, 'to': to}));

  @override Future<void> selectPages(List<int> pages) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'selectPages', extra: {'pages': pages}));

  @override Future<void> mergeFrom(DataSource otherPdf) async {
    final bytes = await otherPdf.readAt(0, otherPdf.length);
    await _op(editorMergeFromOp(handleId: _hid, otherBytes: Uint8List.view(bytes.buffer)));
  }

  @override Future<int> optimizeImages({int quality = 75}) async =>
      (await _op(editorMutateOp(handleId: _hid, editOp: 'optimizeImages', extra: {'quality': quality})))['count'] as int? ?? 0;
  @override Future<int> unembedStandardFonts() async =>
      (await _op(editorMutateOp(handleId: _hid, editOp: 'unembedStandardFonts')))['value'] as int? ?? 0;

  @override Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) => _op(editorMutateOp(handleId: _hid, editOp: 'watermark', extra: {
    'page': page, ...encodeWatermarkArgs(text, style, position, layer),
  }));

  @override Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect,
    double opacity = 1.0,
  }) => _op(editorMutateOp(handleId: _hid, editOp: 'addStamp', extra: {
    'page': page, 'stampType': type.index, ...encodeRectArgs(rect), 'opacity': opacity,
  }));

  @override Future<void> addImageStamp(int page, DataSource imageData, {
    required PdfRect rect, double opacity = 1.0,
  }) async {
    final imgBytes = await readAllBytes(imageData);
    await _op(editorMutateOp(handleId: _hid, editOp: 'addImageStamp', extra: {
      'page': page, 'imageBytes': imgBytes, ...encodeRectArgs(rect), 'opacity': opacity,
    }));
  }

  @override Future<void> embedFile(String name, DataSource data) async {
    final fileBytes = await readAllBytes(data);
    await _op(editorMutateOp(handleId: _hid, editOp: 'embedFile', extra: {'name': name, 'fileData': fileBytes}));
  }
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

  @override Future<void> addRedaction(int page, PdfRect region, {String? overlayText}) =>
      _op(editorMutateOp(handleId: _hid, editOp: 'addRedaction', extra: {
        'page': page, 'x': region.x, 'y': region.y, 'w': region.width, 'h': region.height,
        if (overlayText != null) 'overlayText': overlayText,
      }));
  @override Future<int> redactionCount(int page) async {
    final r = await _b._submit(editorMutateOp(handleId: _hid, editOp: 'redactionCount', extra: {'page': page}));
    return r['count'] as int? ?? 0;
  }
  @override Future<void> applyRedactions() =>
      _op(editorMutateOp(handleId: _hid, editOp: 'applyRedactions'));
  @override Future<void> scrubMetadata() =>
      _op(editorMutateOp(handleId: _hid, editOp: 'scrubMetadata'));

  @override Future<void> save(DataSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
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

  @override Future<void> save(DataSink output) async {
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
  @override Future<void> image(DataSource imageData, PdfRect rect, {String altText = ''}) async {
    final imgBytes = await readAllBytes(imageData);
    await _pgOp('image', {'imageBytes': imgBytes, ...encodeRectArgs(rect), 'altText': altText});
  }
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

