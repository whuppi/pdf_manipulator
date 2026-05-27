// NativeBridge — extends PdfBridge for native platforms.
//
// RULE: Encode request → send to coordinator → decode result. Zero PDF
// logic. Does not interpret results beyond wire decoding. Does not make
// decisions based on PDF content. Symmetric with web/bridge.dart — both
// bridges must encode the same args for the same ops.
//
// VIOLATIONS:
// - No PDF logic (page routing, format detection).
// - No direct FFI calls (coordinator handles those).
// - No conditional behavior that web/bridge.dart doesn't also have.
//
// Main-isolate side only. Spawns coordinator isolate that owns the Rust
// thread pool. Each operation: main creates SourceServer/SinkServer,
// sends a command to coordinator, coordinator does FFI, result comes back.
//
// Decoding: Rust returns binary Uint8List → wire.dart decodes to typed results.
//
// Do NOT import dart:ffi, package:ffi, or bindings.dart here.
// All FFI calls go through the worker isolate (coordinator.dart).
// Direct FFI from the main isolate causes deadlocks on the Rust
// editor mutex. The absence of the import makes this impossible.
//
// INTERNAL — created by bridge_factory.dart.

import 'dart:async';
import 'dart:convert';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/native/sink_server.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/transport/native/source_server.dart';
import 'package:pdf_manipulator/src/transport/native/wire.dart';
import 'package:pdf_manipulator/src/transport/native/coordinator.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/transport/protocol/op.dart';
import 'package:pdf_manipulator/src/types/pdf_doc.dart';

class NativeBridge extends PdfBridge {
  NativeBridge();

  bool _disposed = false;
  SendPort? _workerPort;
  Isolate? _workerIsolate;
  final _pending = <int, Completer<Object?>>{};
  final _pendingStreams = <int, StreamController<Uint8List>>{};
  int _nextId = 0;
  ReceivePort? _responsePort;
  bool _initialized = false;

  Future<void> _ensureWorker() async {
    if (_initialized) return;
    _initialized = true;

    final initPort = ReceivePort();
    _workerIsolate = await Isolate.spawn(
      coordinatorEntryPoint,
      initPort.sendPort,
      debugName: 'PdfBridgeWorker',
    );
    _workerPort = await initPort.first as SendPort;

    _responsePort = ReceivePort();
    _responsePort!.listen((message) {
      if (message is! List || message.length != 3) return;
      final id = message[0] as int;
      final tag = message[1];

      if (tag is bool) {
        final completer = _pending.remove(id);
        if (completer == null) return;
        if (tag) {
          final value = message[2];
          completer.completeError(
            value is String ? StateError(value) : StateError('Bridge error'));
        } else {
          completer.complete(message[2]);
        }
      } else if (tag is String) {
        final controller = _pendingStreams[id];
        if (controller == null) return;
        switch (tag) {
          case 'item':
            controller.add(message[2] as Uint8List);
          case 'done':
            _pendingStreams.remove(id);
            controller.close();
          case 'error':
            _pendingStreams.remove(id);
            controller.addError(StateError(message[2] as String? ?? 'Stream error'));
            controller.close();
        }
      }
    });

    _workerPort!.send(_responsePort!.sendPort);
  }

  Future<Object?> _send(String op, Map<String, Object?> args) async {
    await _ensureWorker();
    final id = _nextId++;
    final completer = Completer<Object?>();
    _pending[id] = completer;
    _workerPort!.send([id, op, args]);
    return completer.future.timeout(
      const Duration(seconds: 60),
      onTimeout: () {
        _pending.remove(id);
        throw TimeoutException('Native bridge op "$op" timed out after 60s');
      },
    );
  }

  // Result decoding: wire.dart (binary → typed results).

  Future<void> _submitEdit(
    String op,
    DataSource source,
    DataSink sink, {
    Uint8List? params,
    List<Uint8List>? secondaries,
  }) async {
    _checkDisposed();
    final srcServer = SourceServer(source);
    final srcPort = srcServer.start();
    final snkServer = SinkServer(sink);
    final snkPort = snkServer.start();
    try {
      final result = await _send(op, {
        'sourcePort': srcPort,
        'sourceLength': source.length,
        'sinkPort': snkPort,
        if (params != null) 'params': params,
        if (secondaries != null) 'secondaries': secondaries,
      });
      final bytes = result as Uint8List;
      if (bytes.isEmpty || bytes[0] == 0) {
        throw wireDecodeError(bytes);
      }
    } finally {
      srcServer.stop();
      snkServer.stop();
    }
  }

  Future<Uint8List> _submitRead(
    String op,
    DataSource source, {
    String? password,
    Uint8List? params,
    int? opCode,
  }) async {
    _checkDisposed();
    final server = SourceServer(source);
    final serverPort = server.start();
    try {
      final result = await _send(op, {
        'sourcePort': serverPort,
        'sourceLength': source.length,
        'password': password,
        if (opCode != null) 'opCode': opCode,
        'params': params,
      });
      return result as Uint8List;
    } finally {
      server.stop();
    }
  }

  Stream<Uint8List> _submitStream(
    String op,
    DataSource source, {
    String? password,
    Uint8List? params,
  }) async* {
    _checkDisposed();
    await _ensureWorker();

    final server = SourceServer(source);
    final serverPort = server.start();

    final id = _nextId++;
    final controller = StreamController<Uint8List>();
    _pendingStreams[id] = controller;

    _workerPort!.send([id, op, {
      'sourcePort': serverPort,
      'sourceLength': source.length,
      'password': password,
      'params': params,
    }]);

    try {
      await for (final item in controller.stream) {
        yield item;
      }
    } finally {
      server.stop();
    }
  }

  Uint8List _encodePages(PdfPages pages) {
    switch (pages) {
      case PdfAllPages():
        return Uint8List.fromList([0]);
      case PdfSinglePage(:final index):
        final buf = Uint8List(5);
        buf[0] = 1;
        ByteData.sublistView(buf).setInt32(1, index, Endian.little);
        return buf;
      case PdfPageList(:final indices):
        final buf = Uint8List(5 + indices.length * 4);
        buf[0] = 2;
        final bd = ByteData.sublistView(buf);
        bd.setInt32(1, indices.length, Endian.little);
        for (var i = 0; i < indices.length; i++) {
          bd.setInt32(5 + i * 4, indices[i], Endian.little);
        }
        return buf;
      case PdfPageRange(:final start, :final end):
        final buf = Uint8List(9);
        buf[0] = 3;
        final bd = ByteData.sublistView(buf);
        bd.setInt32(1, start, Endian.little);
        bd.setInt32(5, end, Endian.little);
        return buf;
    }
  }

  static Uint8List _packStrings(List<Uint8List> parts) {
    var totalLen = 0;
    for (final p in parts) {
      totalLen += 4 + p.length;
    }
    final buf = Uint8List(totalLen);
    final bd = ByteData.sublistView(buf);
    var off = 0;
    for (final p in parts) {
      bd.setInt32(off, p.length, Endian.little); off += 4;
      buf.setAll(off, p); off += p.length;
    }
    return buf;
  }

  // ── PdfBridge implementation ───────────────────────────────────────

  @override
  Future<PdfDoc> open(DataSource source, {String? password}) async {
    _checkDisposed();
    final server = SourceServer(source);
    final serverPort = server.start();
    try {
      final result = await _send(EngineOp.open.wire, {
        'sourcePort': serverPort,
        'sourceLength': source.length,
        'password': password,
      });
      return wireDecodeOpen(result as Uint8List);
    } finally {
      server.stop();
    }
  }

  @override
  Future<String> extract(DataSource source, {
    required PdfPages pages, String? password,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) async {
    final int pageIndex = switch (pages) {
      PdfAllPages() => -1,
      PdfSinglePage(:final index) => index,
      PdfPageList(:final indices) => indices.isEmpty ? -1 : indices.first,
      PdfPageRange(:final start) => start,
    };
    final int opCode = switch (format) {
      PdfExtractionFormat.text || PdfExtractionFormat.plainText || PdfExtractionFormat.auto => 1,
      PdfExtractionFormat.markdown || PdfExtractionFormat.html => 2,
    };
    final params = Uint8List(4);
    ByteData.sublistView(params).setInt32(0, pageIndex, Endian.little);
    final result = await _submitRead(EngineOp.extract.wire, source, password: password, params: params, opCode: opCode);
    return wireDecodeText(result);
  }

  @override
  Future<List<SearchResult>> search(DataSource source, {
    required String query, required PdfPages pages, String? password,
  }) async {
    _checkDisposed();
    final pageIndex = switch (pages) {
      PdfAllPages() => -1,
      PdfSinglePage(:final index) => index,
      PdfPageList(:final indices) => indices.first,
      PdfPageRange(:final start) => start,
    };
    final queryBytes = utf8.encode(query);
    final params = Uint8List(4 + 4 + queryBytes.length);
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, pageIndex, Endian.little);
    bd.setInt32(4, queryBytes.length, Endian.little);
    params.setAll(8, queryBytes);
    final result = await _submitRead(EngineOp.search.wire, source, params: params, password: password);
    return wireDecodeSearch(result);
  }

  @override
  Future<void> sign(DataSource source, DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason, String? location,
  }) async {
    _checkDisposed();
    final reasonBytes = reason != null ? utf8.encode(reason) : Uint8List(0);
    final locBytes = location != null ? utf8.encode(location) : Uint8List(0);
    switch (credentials) {
      case PdfPkcs12Credentials(:final data, :final password):
        final pwBytes = utf8.encode(password);
        final params = _packStrings([pwBytes, reasonBytes, locBytes]);
        await _submitEdit(EngineOp.sign.wire, source, output,
            params: params, secondaries: [data]);
      case PdfPemCredentials(:final certPem, :final keyPem):
        final certBytes = utf8.encode(certPem);
        final keyBytes = utf8.encode(keyPem);
        final params = _packStrings([certBytes, keyBytes, reasonBytes, locBytes]);
        await _submitEdit('signPem', source, output, params: params);
    }
  }

  @override
  Future<void> imagesToPdf(List<DataSource> images, DataSink output) async {
    _checkDisposed();
    if (images.isEmpty) throw ArgumentError('images must not be empty');
    final imageBytes = await Future.wait(images.map(readAllBytes));
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      final result = await _send(EngineOp.imagesToPdf.wire, {
        'images': imageBytes,
        'sinkPort': sinkPort,
      });
      final bytes = result as Uint8List;
      if (bytes.isEmpty || bytes[0] == 0) {
        throw StateError('imagesToPdf failed');
      }
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> convertTo(DataSource source, DataSink output, {required PdfDocumentFormat format, String? password}) async {
    final formatBytes = utf8.encode(format.name);
    final pwBytes = password != null ? utf8.encode(password) : const <int>[];
    final params = Uint8List(4 + formatBytes.length + 4 + pwBytes.length);
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, formatBytes.length, Endian.little);
    params.setAll(4, formatBytes);
    bd.setInt32(4 + formatBytes.length, pwBytes.length, Endian.little);
    params.setAll(4 + formatBytes.length + 4, pwBytes);
    await _submitEdit(EngineOp.convertTo.wire, source, output, params: params);
  }

  @override
  Future<void> convertToPdf(DataSource document, DataSink output, {required PdfDocumentFormat format}) async {
    final formatBytes = utf8.encode(format.name);
    final params = Uint8List(4 + formatBytes.length);
    ByteData.sublistView(params).setInt32(0, formatBytes.length, Endian.little);
    params.setAll(4, formatBytes);
    await _submitEdit(EngineOp.convertToPdf.wire, document, output, params: params);
  }

  @override
  Stream<RenderedPage> render(DataSource source, {
    required PdfPages pages, PdfRenderSize? size, String? password,
  }) async* {
    _checkDisposed();
    final pagesBytes = _encodePages(pages);
    final params = Uint8List(pagesBytes.length + 8);
    params.setAll(0, pagesBytes);
    final bd = ByteData.sublistView(params);
    bd.setInt32(pagesBytes.length, size?.maxWidth ?? 0, Endian.little);
    bd.setInt32(pagesBytes.length + 4, size?.maxHeight ?? 0, Endian.little);

    await for (final itemBytes in _submitStream(EngineOp.render.wire, source, password: password, params: params)) {
      yield wireDecodeRenderedPage(itemBytes);
    }
  }

  @override
  Stream<PdfImage> extractImages(DataSource source, {
    required PdfPages pages, String? password,
  }) async* {
    _checkDisposed();
    final params = _encodePages(pages);

    await for (final itemBytes in _submitStream(EngineOp.extractImages.wire, source, password: password, params: params)) {
      yield wireDecodeImage(itemBytes);
    }
  }

  @override
  Future<List<PdfSignatureInfo>> getSignatures(DataSource source, {
    String? password,
  }) async {
    _checkDisposed();
    final result = await _submitRead(EngineOp.getSignatures.wire, source, password: password);
    return wireDecodeSignatures(result);
  }

  @override
  Future<bool> verifySignatures(DataSource source, {String? password}) async {
    _checkDisposed();
    final result = await _submitRead(EngineOp.verifySignatures.wire, source, password: password);
    return wireDecodeVerifySignatures(result);
  }

  @override
  Future<PdfValidationResult> validatePdfA(DataSource source, {
    int level = 2, String? password,
  }) async {
    final params = Uint8List(4);
    ByteData.sublistView(params).setInt32(0, level, Endian.little);
    final result = await _submitRead(EngineOp.validatePdfA.wire, source, password: password, params: params);
    return wireDecodeValidation(result);
  }

  @override
  Future<bool> validatePdfUa(DataSource source, {
    int level = 1, String? password,
  }) async {
    final params = Uint8List(4);
    ByteData.sublistView(params).setInt32(0, level, Endian.little);
    final result = await _submitRead(EngineOp.validatePdfUa.wire, source, password: password, params: params);
    return wireDecodeValidatePdfUa(result);
  }

  @override
  Future<List<PdfBookmarkSplit>> planSplitByBookmarks(DataSource source, {String? password}) async {
    _checkDisposed();
    final result = await _submitRead(EngineOp.planSplitByBookmarks.wire, source, password: password);
    return wireDecodeBookmarkSplits(result);
  }

  @override
  Future<PdfPageClassification> classifyPage(DataSource source, int page, {String? password}) async {
    _checkDisposed();
    final params = Uint8List(4);
    ByteData.sublistView(params).setInt32(0, page, Endian.little);
    final result = await _submitRead(EngineOp.classifyPage.wire, source, password: password, params: params);
    return wireDecodeClassifyPage(result);
  }

  @override
  Future<PdfDocumentClassification> classifyDocument(DataSource source, {String? password}) async {
    _checkDisposed();
    final result = await _submitRead(EngineOp.classifyDocument.wire, source, password: password);
    return wireDecodeClassifyDocument(result);
  }

  @override
  Future<BridgeEditorHandle> openEditor(DataSource source, {String? password}) async {
    _checkDisposed();
    final server = SourceServer(source);
    final serverPort = server.start();
    try {
      final result = await _send(EngineOp.editorOpen.wire, {
        'sourcePort': serverPort,
        'sourceLength': source.length,
        'password': password,
      });
      final bytes = result as Uint8List;
      if (bytes.isEmpty || bytes[0] != 1) {
        server.stop();
        throw wireDecodeError(bytes);
      }
      final bd = ByteData.sublistView(bytes);
      final handleId = bd.getUint64(1, Endian.little);
      return _NativeEditorHandle(this, handleId, server);
    } catch (_) {
      server.stop();
      rethrow;
    }
  }

  @override
  Future<BridgeBuilderHandle> createBuilder() async {
    _checkDisposed();
    await _ensureWorker();
    final result = await _send(EngineOp.builderCreate.wire, {});
    final handleId = result as int;
    return _NativeBuilderHandle(this, handleId);
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _responsePort?.close();
    _workerIsolate?.kill();
    _workerIsolate = null;
    _workerPort = null;
    for (final c in _pending.values) {
      c.completeError(StateError('Disposed'));
    }
    _pending.clear();
  }

  void _checkDisposed() {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
  }

}

// ── NativeEditorHandle ───────────────────────────────────────────────

class _NativeEditorHandle implements BridgeEditorHandle {
  _NativeEditorHandle(this._bridge, this._handleId, this._sourceServer);

  final NativeBridge _bridge;
  final int _handleId;
  final SourceServer _sourceServer;

  Future<Object?> _send(String op, Map<String, Object?> args) => _bridge._send(op, args);

  Future<Uint8List> _mutate(int opCode, {Uint8List? params, List<Uint8List>? secondaries}) async {
    final result = await _send(EngineOp.editorMutate.wire, {
      'handleId': _handleId,
      'opCode': opCode,
      'params': params,
      'secondaries': secondaries,
    });
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1) throw wireDecodeError(bytes);
    return bytes;
  }

  @override Future<int> get pageCount async =>
      wireDecodeEditorMetadata(await _send(EngineOp.editorGetMetadata.wire, {'handleId': _handleId}) as Uint8List).pageCount;
  @override Future<bool> get isModified async {
    final result = await _send(EngineOp.editorIsModified.wire, {'handleId': _handleId});
    final bytes = result as Uint8List;
    return bytes.length > 1 && bytes[1] == 1;
  }

  Future<_Metadata> _getMetadata() async {
    final result = await _send(EngineOp.editorGetMetadata.wire, {'handleId': _handleId});
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1) throw wireDecodeError(bytes);
    final m = wireDecodeEditorMetadata(bytes);
    return _Metadata(
      version: m.version,
      title: m.title,
      author: m.author,
      subject: m.subject,
      keywords: m.keywords,
    );
  }

  @override Future<String> get version async => (await _getMetadata()).version;
  @override Future<String> getTitle() async => (await _getMetadata()).title;
  @override Future<void> setTitle(String value) => _mutate(19, params: _encodeString(value));
  @override Future<String> getAuthor() async => (await _getMetadata()).author;
  @override Future<void> setAuthor(String value) => _mutate(20, params: _encodeString(value));
  @override Future<String> getSubject() async => (await _getMetadata()).subject;
  @override Future<void> setSubject(String value) => _mutate(21, params: _encodeString(value));
  @override Future<String> getKeywords() async => (await _getMetadata()).keywords;
  @override Future<void> setKeywords(String value) => _mutate(22, params: _encodeString(value));

  @override Future<void> rotatePage(int page, {required int degrees}) =>
      _mutate(5, params: _encodePageDegrees(page, degrees));
  @override Future<void> rotateAllPages({required int degrees}) =>
      _mutate(6, params: _encodeInt(degrees));
  @override Future<PdfRect> getPageMediaBox(int page) async {
    final result = await _send(EngineOp.editorPageMediaBox.wire, {'handleId': _handleId, 'page': page});
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1) return const PdfRect(x: 0, y: 0, width: 595, height: 842);
    final bd = ByteData.sublistView(bytes);
    return PdfRect(
      x: bd.getFloat64(1, Endian.little),
      y: bd.getFloat64(9, Endian.little),
      width: bd.getFloat64(17, Endian.little),
      height: bd.getFloat64(25, Endian.little),
    );
  }
  @override Future<void> deletePage(int page) =>
      _mutate(3, params: _encodeDeletePages([page]));
  @override Future<void> movePage({required int from, required int to}) =>
      _mutate(10, params: _encodeFromTo(from, to));
  @override Future<void> selectPages(List<int> pages) =>
      _mutate(2, params: _encodeDeletePages(pages));

  @override Future<void> mergeFrom(DataSource otherPdf) async {
    final bytes = await otherPdf.readAt(0, otherPdf.length);
    await _mutate(1, secondaries: [bytes]);
  }

  @override Future<int> optimizeImages({int quality = 75}) async {
    final result = await _send(EngineOp.editorQuery.wire, {
      'handleId': _handleId, 'queryCode': 4, 'param': quality,
    });
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1 || bytes.length < 5) return 0;
    return ByteData.sublistView(bytes).getInt32(1, Endian.little);
  }
  @override Future<int> unembedStandardFonts() async {
    final result = await _send(EngineOp.editorQuery.wire, {
      'handleId': _handleId, 'queryCode': 5, 'param': 0,
    });
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1 || bytes.length < 5) return 0;
    return ByteData.sublistView(bytes).getInt32(1, Endian.little);
  }

  @override Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) => _mutate(15, params: _encodeWatermark(page, text, style, position, layer));

  @override Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect,
    double opacity = 1.0,
  }) => _mutate(17, params: _encodeStamp(page, type.index, rect, opacity));

  @override Future<void> addImageStamp(int page, DataSource imageData, {
    required PdfRect rect, double opacity = 1.0,
  }) async {
    final imgBytes = await readAllBytes(imageData);
    await _mutate(18, params: _encodeImageStamp(page, rect, opacity), secondaries: [imgBytes]);
  }

  @override Future<void> embedFile(String name, DataSource data) async {
    final fileBytes = await readAllBytes(data);
    await _mutate(11, params: _encodeString(name), secondaries: [fileBytes]);
  }
  @override Future<void> eraseRegions(int page, List<PdfRect> regions) =>
      _mutate(12, params: _encodeEraseRegions(page, regions));
  @override Future<void> flattenForms() => _mutate(7);
  @override Future<void> flattenAllAnnotations() => _mutate(24);
  @override Future<void> setFormFieldValue(String fieldName, String value) =>
      _mutate(25, params: _encodeTwoStrings(fieldName, value));
  @override Future<void> cropMargins({
    double left = 0, double right = 0, double top = 0, double bottom = 0,
  }) => _mutate(26, params: _encodeFourDoubles(left, right, top, bottom));
  @override Future<void> convertToPdfA({int level = 1}) => _mutate(27, params: _encodeInt(level));
  @override Future<void> resizeImage(int page, String imageName, {
    required double width, required double height,
  }) => _mutate(28, params: _encodeResizeImage(page, imageName, width, height));

  @override
  Future<void> addRedaction(int page, PdfRect region, {String? overlayText}) {
    final textBytes = overlayText != null ? utf8.encode(overlayText) : Uint8List(0);
    final params = Uint8List(4 + 8 * 4 + 4 + textBytes.length);
    final bd = ByteData.sublistView(params);
    var off = 0;
    bd.setInt32(off, page, Endian.little); off += 4;
    bd.setFloat64(off, region.x, Endian.little); off += 8;
    bd.setFloat64(off, region.y, Endian.little); off += 8;
    bd.setFloat64(off, region.width, Endian.little); off += 8;
    bd.setFloat64(off, region.height, Endian.little); off += 8;
    bd.setInt32(off, textBytes.length, Endian.little); off += 4;
    params.setAll(off, textBytes);
    return _mutate(30, params: params);
  }

  @override Future<int> redactionCount(int page) async {
    final result = await _send(EngineOp.editorRedactionCount.wire, {
      'handleId': _handleId,
      'page': page,
    });
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1) return 0;
    if (bytes.length < 5) return 0;
    return ByteData.sublistView(bytes).getInt32(1, Endian.little);
  }

  @override Future<void> applyRedactions() => _mutate(32);

  @override Future<void> scrubMetadata() => _mutate(33);

  @override
  Future<void> save(DataSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
    final server = SinkServer(output);
    final serverPort = server.start();
    try {
      final saveMode = options.mode.index;
      final encryptMode = switch (options.encryption) {
        PdfEncryptionKeep() => 0,
        PdfEncryptionRemove() => 1,
        PdfEncryptionConfig() => 2,
      };
      int encAlgo = 0;
      String encUserPw = '';
      String encOwnerPw = '';
      int encPerms = -1;
      if (options.encryption case PdfEncryptionConfig c) {
        encAlgo = c.algorithm.index + 1;
        encUserPw = c.userPassword;
        encOwnerPw = c.ownerPassword;
        encPerms = c.permissions.toBits();
      }
      final result = await _send(EngineOp.editorSave.wire, {
        'handleId': _handleId,
        'sinkPort': serverPort,
        'compress': options.compress,
        'garbageCollect': options.garbageCollect,
        'saveMode': saveMode,
        'encryptMode': encryptMode,
        'encryptAlgo': encAlgo,
        'encryptUserPw': encUserPw,
        'encryptOwnerPw': encOwnerPw,
        'encryptPermissions': encPerms,
      });
      final bytes = result as Uint8List;
      if (bytes.isEmpty || bytes[0] != 1) throw wireDecodeError(bytes);
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> dispose() async {
    await _send(EngineOp.editorDispose.wire, {'handleId': _handleId});
    _sourceServer.stop();
  }

  // ── Param encoding helpers ──

  Uint8List _encodeInt(int value) {
    final bd = ByteData(4);
    bd.setInt32(0, value, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeString(String value) {
    final bytes = utf8.encode(value);
    final bd = ByteData(4 + bytes.length);
    bd.setInt32(0, bytes.length, Endian.little);
    bd.buffer.asUint8List().setRange(4, 4 + bytes.length, bytes);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeDeletePages(List<int> pages) {
    final bd = ByteData(4 + pages.length * 4);
    bd.setInt32(0, pages.length, Endian.little);
    for (var i = 0; i < pages.length; i++) {
      bd.setInt32(4 + i * 4, pages[i], Endian.little);
    }
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeFromTo(int from, int to) {
    final bd = ByteData(8);
    bd.setInt32(0, from, Endian.little);
    bd.setInt32(4, to, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeWatermark(int page, String text, PdfWatermarkStyle style,
      PdfWatermarkPosition position, PdfWatermarkLayer layer) {
    final textBytes = utf8.encode(text);
    // Layout: page(i32) textLen(i32) text fontSize(f64) rotation(f64) opacity(f64)
    //         r(f64) g(f64) b(f64) layer(i32) posType(i32) [pos-specific fields as f64]
    final posFields = switch (position) {
      PdfWatermarkCenter() => <double>[],
      PdfWatermarkCorner(:final corner, :final marginX, :final marginY) =>
        [corner.index.toDouble(), marginX, marginY],
      PdfWatermarkTiled(:final columns, :final rows) =>
        [columns.toDouble(), rows.toDouble()],
      PdfWatermarkExact(:final x, :final y, :final width, :final height) =>
        [x, y, width, height],
    };
    final posType = switch (position) {
      PdfWatermarkCenter() => 0,
      PdfWatermarkCorner() => 1,
      PdfWatermarkTiled() => 2,
      PdfWatermarkExact() => 3,
    };
    final size = 8 + textBytes.length + 48 + 8 + posFields.length * 8;
    final bd = ByteData(size);
    bd.setInt32(0, page, Endian.little);
    bd.setInt32(4, textBytes.length, Endian.little);
    bd.buffer.asUint8List().setRange(8, 8 + textBytes.length, textBytes);
    var off = 8 + textBytes.length;
    bd.setFloat64(off, style.fontSize, Endian.little); off += 8;
    bd.setFloat64(off, style.rotation, Endian.little); off += 8;
    bd.setFloat64(off, style.opacity, Endian.little); off += 8;
    bd.setFloat64(off, style.color.r, Endian.little); off += 8;
    bd.setFloat64(off, style.color.g, Endian.little); off += 8;
    bd.setFloat64(off, style.color.b, Endian.little); off += 8;
    bd.setInt32(off, layer.index, Endian.little); off += 4;
    bd.setInt32(off, posType, Endian.little); off += 4;
    for (final f in posFields) {
      bd.setFloat64(off, f, Endian.little); off += 8;
    }
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeStamp(int page, int stampType, PdfRect rect, double opacity) {
    final bd = ByteData(48);
    bd.setInt32(0, page, Endian.little);
    bd.setInt32(4, stampType, Endian.little);
    bd.setFloat64(8, rect.x, Endian.little);
    bd.setFloat64(16, rect.y, Endian.little);
    bd.setFloat64(24, rect.width, Endian.little);
    bd.setFloat64(32, rect.height, Endian.little);
    bd.setFloat64(40, opacity, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeImageStamp(int page, PdfRect rect, double opacity) {
    final bd = ByteData(44);
    bd.setInt32(0, page, Endian.little);
    bd.setFloat64(4, rect.x, Endian.little);
    bd.setFloat64(12, rect.y, Endian.little);
    bd.setFloat64(20, rect.width, Endian.little);
    bd.setFloat64(28, rect.height, Endian.little);
    bd.setFloat64(36, opacity, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeEraseRegions(int page, List<PdfRect> regions) {
    final bd = ByteData(8 + regions.length * 32);
    bd.setInt32(0, page, Endian.little);
    bd.setInt32(4, regions.length, Endian.little);
    for (var i = 0; i < regions.length; i++) {
      final base = 8 + i * 32;
      bd.setFloat64(base, regions[i].x, Endian.little);
      bd.setFloat64(base + 8, regions[i].y, Endian.little);
      bd.setFloat64(base + 16, regions[i].width, Endian.little);
      bd.setFloat64(base + 24, regions[i].height, Endian.little);
    }
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeTwoStrings(String a, String b) {
    final aBytes = utf8.encode(a);
    final bBytes = utf8.encode(b);
    final bd = ByteData(8 + aBytes.length + bBytes.length);
    bd.setInt32(0, aBytes.length, Endian.little);
    bd.buffer.asUint8List().setRange(4, 4 + aBytes.length, aBytes);
    bd.setInt32(4 + aBytes.length, bBytes.length, Endian.little);
    bd.buffer.asUint8List().setRange(8 + aBytes.length, 8 + aBytes.length + bBytes.length, bBytes);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeFourDoubles(double a, double b, double c, double d) {
    final bd = ByteData(32);
    bd.setFloat64(0, a, Endian.little);
    bd.setFloat64(8, b, Endian.little);
    bd.setFloat64(16, c, Endian.little);
    bd.setFloat64(24, d, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeResizeImage(int page, String imageName, double width, double height) {
    final nameBytes = utf8.encode(imageName);
    final bd = ByteData(8 + nameBytes.length + 16);
    bd.setInt32(0, page, Endian.little);
    bd.setInt32(4, nameBytes.length, Endian.little);
    bd.buffer.asUint8List().setRange(8, 8 + nameBytes.length, nameBytes);
    bd.setFloat64(8 + nameBytes.length, width, Endian.little);
    bd.setFloat64(16 + nameBytes.length, height, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodePageDegrees(int page, int degrees) {
    final bd = ByteData(12);
    bd.setInt32(0, 1, Endian.little);
    bd.setInt32(4, page, Endian.little);
    bd.setInt32(8, degrees, Endian.little);
    return bd.buffer.asUint8List();
  }
}

class _Metadata {
  final String version, title, author, subject, keywords;
  const _Metadata({
    required this.version, required this.title, required this.author,
    required this.subject, required this.keywords,
  });
}

// ── NativeBuilderHandle ─────────────────────────────────────────────

class _NativeBuilderHandle implements BridgeBuilderHandle {
  _NativeBuilderHandle(this._bridge, this._handleId);
  final NativeBridge _bridge;
  final int _handleId;

  Future<Object?> _send(String op, Map<String, Object?> args) =>
      _bridge._send(op, args);

  @override
  Future<void> setTitle(String value) =>
      _send(EngineOp.builderSetMetadata.wire, {'handleId': _handleId, 'op': 1, 'value': value});

  @override
  Future<void> setAuthor(String value) =>
      _send(EngineOp.builderSetMetadata.wire, {'handleId': _handleId, 'op': 2, 'value': value});

  @override
  Future<void> setSubject(String value) =>
      _send(EngineOp.builderSetMetadata.wire, {'handleId': _handleId, 'op': 3, 'value': value});

  @override
  Future<void> setKeywords(String value) =>
      _send(EngineOp.builderSetMetadata.wire, {'handleId': _handleId, 'op': 4, 'value': value});

  @override
  Future<BridgePageBuilderHandle> addA4Page() async {
    await _send(EngineOp.builderAddPage.wire, {'handleId': _handleId, 'pageType': 0});
    return _NativePageBuilderHandle(_bridge, _handleId);
  }

  @override
  Future<BridgePageBuilderHandle> addLetterPage() async {
    await _send(EngineOp.builderAddPage.wire, {'handleId': _handleId, 'pageType': 1});
    return _NativePageBuilderHandle(_bridge, _handleId);
  }

  @override
  Future<BridgePageBuilderHandle> addPage({
    required double width, required double height,
  }) async {
    await _send(EngineOp.builderAddPage.wire, {
      'handleId': _handleId, 'pageType': 2, 'width': width, 'height': height,
    });
    return _NativePageBuilderHandle(_bridge, _handleId);
  }

  @override
  Future<void> save(DataSink output) async {
    final snkServer = SinkServer(output);
    final snkPort = snkServer.start();
    try {
      final result = await _send(EngineOp.builderSave.wire, {
        'handleId': _handleId,
        'sinkPort': snkPort,
      });
      final bytes = result as Uint8List;
      if (bytes.isEmpty || bytes[0] == 0) {
        throw StateError('Builder save failed');
      }
    } finally {
      snkServer.stop();
    }
  }

  @override
  Future<void> dispose() async {
    await _send(EngineOp.builderDispose.wire, {'handleId': _handleId});
  }
}

class _NativePageBuilderHandle implements BridgePageBuilderHandle {
  _NativePageBuilderHandle(this._bridge, this._builderHandleId);
  final NativeBridge _bridge;
  final int _builderHandleId;

  Future<void> _pageOp(int opCode, {Uint8List? params, Uint8List? secondary}) =>
      _bridge._send(EngineOp.builderPageOp.wire, {
        'handleId': _builderHandleId,
        'opCode': opCode,
        if (params != null) 'params': params,
        if (secondary != null) 'secondary': secondary,
      });

  Uint8List _encodeF32(double v) {
    final b = ByteData(4);
    b.setFloat32(0, v, Endian.little);
    return b.buffer.asUint8List();
  }

  Uint8List _encodeF32x2(double a, double b) {
    final bd = ByteData(8);
    bd.setFloat32(0, a, Endian.little);
    bd.setFloat32(4, b, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeF32x4(double a, double b, double c, double d) {
    final bd = ByteData(16);
    bd.setFloat32(0, a, Endian.little);
    bd.setFloat32(4, b, Endian.little);
    bd.setFloat32(8, c, Endian.little);
    bd.setFloat32(12, d, Endian.little);
    return bd.buffer.asUint8List();
  }

  @override
  Future<void> font(String name, double size) async {
    final sizeBytes = _encodeF32(size);
    final nameBytes = utf8.encode(name);
    final params = Uint8List(4 + nameBytes.length);
    params.setAll(0, sizeBytes);
    params.setAll(4, nameBytes);
    await _pageOp(1, params: params);
  }

  @override Future<void> at(double x, double y) => _pageOp(2, params: _encodeF32x2(x, y));
  @override Future<void> text(String text) => _pageOp(3, params: Uint8List.fromList(utf8.encode(text)));

  @override
  Future<void> heading(int level, String text) {
    final textBytes = utf8.encode(text);
    final params = Uint8List(1 + textBytes.length);
    params[0] = level;
    params.setAll(1, textBytes);
    return _pageOp(4, params: params);
  }

  @override Future<void> paragraph(String text) => _pageOp(5, params: Uint8List.fromList(utf8.encode(text)));
  @override Future<void> space(double points) => _pageOp(6, params: _encodeF32(points));
  @override Future<void> horizontalRule() => _pageOp(7);

  @override
  Future<void> image(DataSource imageData, PdfRect rect, {String altText = ''}) async {
    final imgBytes = await readAllBytes(imageData);
    final rectBytes = _encodeF32x4(rect.x, rect.y, rect.width, rect.height);
    final altBytes = utf8.encode(altText);
    final params = Uint8List(16 + altBytes.length);
    params.setAll(0, rectBytes);
    params.setAll(16, altBytes);
    await _pageOp(8, params: params, secondary: imgBytes);
  }

  @override Future<void> watermark(String text) => _pageOp(9, params: Uint8List.fromList(utf8.encode(text)));

  @override
  Future<void> textField(String name, PdfRect rect, {String? defaultValue}) {
    final rectBytes = _encodeF32x4(rect.x, rect.y, rect.width, rect.height);
    final nameStr = defaultValue != null ? '$name\x00$defaultValue' : name;
    final nameBytes = utf8.encode(nameStr);
    final params = Uint8List(16 + nameBytes.length);
    params.setAll(0, rectBytes);
    params.setAll(16, nameBytes);
    return _pageOp(10, params: params);
  }

  @override
  Future<void> checkbox(String name, PdfRect rect, {bool checked = false}) {
    final rectBytes = _encodeF32x4(rect.x, rect.y, rect.width, rect.height);
    final nameBytes = utf8.encode(name);
    final params = Uint8List(17 + nameBytes.length);
    params.setAll(0, rectBytes);
    params[16] = checked ? 1 : 0;
    params.setAll(17, nameBytes);
    return _pageOp(11, params: params);
  }

  @override
  Future<void> comboBox(String name, PdfRect rect, List<String> options, {String? selected}) {
    final rectBytes = _encodeF32x4(rect.x, rect.y, rect.width, rect.height);
    final countBd = ByteData(4);
    countBd.setInt32(0, options.length, Endian.little);
    // String payload: name\0option1\0option2\0...\0optionN\0selected_or_empty
    final strPayload = '$name\x00${options.join('\x00')}\x00${selected ?? ''}';
    final strBytes = utf8.encode(strPayload);
    final params = Uint8List(16 + 4 + strBytes.length);
    params.setAll(0, rectBytes);
    params.setAll(16, countBd.buffer.asUint8List());
    params.setAll(20, strBytes);
    return _pageOp(12, params: params);
  }

  @override
  Future<void> pushButton(String name, PdfRect rect, String caption) {
    final rectBytes = _encodeF32x4(rect.x, rect.y, rect.width, rect.height);
    final str = '$name\x00$caption';
    final strBytes = utf8.encode(str);
    final params = Uint8List(16 + strBytes.length);
    params.setAll(0, rectBytes);
    params.setAll(16, strBytes);
    return _pageOp(13, params: params);
  }

  @override
  Future<void> signatureField(String name, PdfRect rect) {
    final rectBytes = _encodeF32x4(rect.x, rect.y, rect.width, rect.height);
    final nameBytes = utf8.encode(name);
    final params = Uint8List(16 + nameBytes.length);
    params.setAll(0, rectBytes);
    params.setAll(16, nameBytes);
    return _pageOp(14, params: params);
  }

  @override Future<void> newline() => _pageOp(15);
  @override Future<void> newPageSameSize() => _pageOp(16);
  @override Future<void> done() => _pageOp(17);

  @override
  Future<void> radioGroup(String name, List<({String value, PdfRect rect})> options, {String? selected}) {
    final bb = BytesBuilder();
    final countBd = ByteData(4);
    countBd.setInt32(0, options.length, Endian.little);
    bb.add(countBd.buffer.asUint8List());
    for (final opt in options) {
      bb.add(_encodeF32x4(opt.rect.x, opt.rect.y, opt.rect.width, opt.rect.height));
      final vBytes = utf8.encode(opt.value);
      final lenBd = ByteData(2);
      lenBd.setUint16(0, vBytes.length, Endian.little);
      bb.add(lenBd.buffer.asUint8List());
      bb.add(vBytes);
    }
    final nameBytes = utf8.encode(name);
    final nameLenBd = ByteData(2);
    nameLenBd.setUint16(0, nameBytes.length, Endian.little);
    bb.add(nameLenBd.buffer.asUint8List());
    bb.add(nameBytes);
    if (selected != null) {
      final selBytes = utf8.encode(selected);
      final selLenBd = ByteData(2);
      selLenBd.setUint16(0, selBytes.length, Endian.little);
      bb.add(selLenBd.buffer.asUint8List());
      bb.add(selBytes);
    }
    return _pageOp(18, params: bb.takeBytes());
  }

  @override Future<void> fieldKeystroke(String script) => _pageOp(19, params: Uint8List.fromList(utf8.encode(script)));
  @override Future<void> fieldFormat(String script) => _pageOp(20, params: Uint8List.fromList(utf8.encode(script)));
  @override Future<void> fieldValidate(String script) => _pageOp(21, params: Uint8List.fromList(utf8.encode(script)));
  @override Future<void> fieldCalculate(String script) => _pageOp(22, params: Uint8List.fromList(utf8.encode(script)));
  @override Future<void> linkUrl(String url) => _pageOp(23, params: Uint8List.fromList(utf8.encode(url)));

  @override
  Future<void> linkPage(int targetPage) {
    final bd = ByteData(4);
    bd.setInt32(0, targetPage, Endian.little);
    return _pageOp(24, params: bd.buffer.asUint8List());
  }

  @override
  Future<void> footnote(String refMark, String noteText) {
    final rmBytes = utf8.encode(refMark);
    final ntBytes = utf8.encode(noteText);
    final bb = BytesBuilder();
    final lenBd = ByteData(2);
    lenBd.setUint16(0, rmBytes.length, Endian.little);
    bb.add(lenBd.buffer.asUint8List());
    bb.add(rmBytes);
    bb.add(ntBytes);
    return _pageOp(25, params: bb.takeBytes());
  }

  @override
  Future<void> columns(int columnCount, double gapPt, String text) {
    final textBytes = utf8.encode(text);
    final bd = ByteData(8);
    bd.setInt32(0, columnCount, Endian.little);
    bd.setFloat32(4, gapPt, Endian.little);
    final params = Uint8List(8 + textBytes.length);
    params.setAll(0, bd.buffer.asUint8List());
    params.setAll(8, textBytes);
    return _pageOp(26, params: params);
  }
}
