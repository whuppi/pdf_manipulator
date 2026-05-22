// WebBridge — implements PdfBridge for web platforms.
//
// Uses a Web Worker pool. Each worker has its own WASM instance.
// Input streams to OPFS, engine reads from OPFS via JsCallbackReader.
// Output streams back via postMessage chunks.
// Cancel = Worker.terminate(). Dispose = terminate all + OPFS cleanup.
//
// INTERNAL — created by bridge_factory.dart.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_sink.dart';
import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/api/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/api/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/api/types/pdf_params.dart';
import 'package:pdf_manipulator/src/bridge/bridge.dart';
import 'package:pdf_manipulator/src/bridge/web/opfs.dart';
import 'package:pdf_manipulator/src/bridge/web/worker_pool.dart';
import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';
import 'package:pdf_manipulator/src/page/pdf_page_info.dart';

class WebBridge implements PdfBridge {
  WebBridge({String? workerUrl})
      : _pool = WebWorkerPool(workerUrl: workerUrl ?? 'pdf_manipulator/worker.js'),
        _opfs = OpfsRegistry();

  final WebWorkerPool _pool;
  final OpfsRegistry _opfs;
  bool _disposed = false;

  /// Acquire a session, stream source to OPFS, run op, release session.
  /// One session = one worker = one OPFS file lifecycle. No cross-worker lock.
  Future<Map<Object?, Object?>> _sendWithSource(
    String op,
    PdfSource source,
    Map<String, Object?> extraArgs,
  ) async {
    _checkDisposed();
    final session = await _pool.acquire();
    try {
      final filename = await streamSourceToOpfs(source, session, _opfs);
      return await session.send(op, {'opfsFile': filename, ...extraArgs});
    } finally {
      _pool.release(session);
    }
  }

  /// Same as _sendWithSource but writes output chunks to sink.
  Future<void> _sendWithSourceAndSink(
    String op,
    PdfSource source,
    PdfSink sink,
    Map<String, Object?> extraArgs,
  ) async {
    _checkDisposed();
    final session = await _pool.acquire();
    try {
      final filename = await streamSourceToOpfs(source, session, _opfs);
      try {
        // TODO: wire output chunk streaming via session.onChunk callback
        final r = await session.send(op, {'opfsFile': filename, ...extraArgs});
        if (r['bytes'] != null) {
          final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
          await sink.write(bytes);
        }
      } finally {
        await cleanupOpfsFile(filename, session, _opfs);
      }
    } finally {
      _pool.release(session);
    }
  }

  // ── Inspect ──

  @override
  Future<PdfDoc> open(PdfSource source, {String? password}) async {
    final r = await _sendWithSource('open', source, {'password': password});
    final pagesRaw = r['pages'] as List? ?? [];
    final pages = pagesRaw.map((p) {
      final m = p as Map<Object?, Object?>;
      return PdfPageInfo(
        index: m['index'] as int,
        width: (m['width'] as num).toDouble(),
        height: (m['height'] as num).toDouble(),
        rotation: (m['rotation'] as num?)?.toInt() ?? 0,
      );
    }).toList();
    return PdfDoc(
      pageCount: r['pageCount'] as int,
      version: r['version'] as String? ?? '2.0',
      pages: pages,
      title: r['title'] as String?,
      author: r['author'] as String?,
      isTagged: r['isTagged'] as bool? ?? false,
      isEncrypted: r['isEncrypted'] as bool? ?? false,
    );
  }

  // ── Structural ──

  @override
  Future<void> merge(List<PdfSource> inputs, PdfSink output) async {
    _checkDisposed();
    final session = await _pool.acquire();
    try {
      // Stream first input to OPFS on this session's worker
      final filename = await streamSourceToOpfs(inputs[0], session, _opfs);
      // Read remaining inputs as bytes (secondary data)
      final remaining = <ByteBuffer>[];
      for (var i = 1; i < inputs.length; i++) {
        final bytes = await inputs[i].readAt(0, inputs[i].length);
        remaining.add(bytes.buffer);
      }
      try {
        final r = await session.send('merge', {
          'opfsFile': filename,
          'inputs': remaining,
        });
        if (r['bytes'] != null) {
          await output.write(Uint8List.view(r['bytes'] as ByteBuffer));
        }
      } finally {
        await cleanupOpfsFile(filename, session, _opfs);
      }
    } finally {
      _pool.release(session);
    }
  }

  @override
  Future<void> split(PdfSource source, PdfSink Function(int) sinkFactory,
      {required int every}) async {
    _checkDisposed();
    if (every < 1) throw ArgumentError('every must be >= 1');
    final doc = await open(source);
    final total = doc.pageCount;
    var chunkIndex = 0;
    for (var start = 0; start < total; start += every) {
      final end = (start + every).clamp(0, total);
      final pages = List.generate(end - start, (i) => start + i);
      final sink = sinkFactory(chunkIndex);
      await extractPages(source, sink, pages: pages);
      chunkIndex++;
    }
  }

  @override
  Future<int> splitBySize(PdfSource source, PdfSink Function(int) sinkFactory,
      {required int maxBytes}) async {
    _checkDisposed();
    if (maxBytes < 1) throw ArgumentError('maxBytes must be >= 1');
    final doc = await open(source);
    final total = doc.pageCount;
    var chunkIndex = 0;
    var chunkPages = <int>[];
    for (var i = 0; i < total; i++) {
      chunkPages.add(i);
      final trialSink = ByteCountSink();
      await extractPages(source, trialSink, pages: chunkPages);
      if (trialSink.length > maxBytes && chunkPages.length > 1) {
        chunkPages.removeLast();
        final sink = sinkFactory(chunkIndex);
        await extractPages(source, sink, pages: chunkPages);
        chunkIndex++;
        chunkPages = [i];
      }
    }
    if (chunkPages.isNotEmpty) {
      final sink = sinkFactory(chunkIndex);
      await extractPages(source, sink, pages: chunkPages);
      chunkIndex++;
    }
    return chunkIndex;
  }

  @override
  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages}) =>
    _sendWithSourceAndSink('extractPages', source, output, {'pages': pages});

  @override
  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages}) =>
    _sendWithSourceAndSink('deletePages', source, output, {'pages': pages});

  @override
  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order}) =>
    _sendWithSourceAndSink('reorderPages', source, output, {'order': order});

  @override
  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to}) =>
    _sendWithSourceAndSink('movePage', source, output, {'from': from, 'to': to});

  @override
  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages}) =>
    _sendWithSourceAndSink('rotatePages', source, output, {'pages': pages});

  @override
  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees}) =>
    _sendWithSourceAndSink('rotateAllPages', source, output, {'degrees': degrees});

  // ── Content ──

  @override
  Future<void> flattenForms(PdfSource source, PdfSink output) =>
    _sendWithSourceAndSink('flattenForms', source, output, {});

  @override
  Future<void> applyRedactions(PdfSource source, PdfSink output) =>
    _sendWithSourceAndSink('applyRedactions', source, output, {});

  @override
  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData}) =>
    _sendWithSourceAndSink('embedFile', source, output, {
      'name': name, 'fileData': fileData.buffer,
    });

  @override
  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions}) =>
    _sendWithSourceAndSink('eraseRegions', source, output, {
      'page': page,
      'regions': regions.map((r) => [r.x, r.y, r.width, r.height]).toList(),
    });

  @override
  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75, bool garbageCollect = true,
       bool linearize = false}) =>
    _sendWithSourceAndSink('compress', source, output, {
      'imageQuality': imageQuality,
      'garbageCollect': garbageCollect,
      'linearize': linearize,
    });

  // ── Extraction ──

  @override
  Future<String> extract(PdfSource source,
      {required PdfPages pages, String? password,
       PdfExtractionFormat format = PdfExtractionFormat.auto}) async {
    _checkDisposed();
    final opName = switch (format) {
      PdfExtractionFormat.markdown => 'toMarkdown',
      PdfExtractionFormat.html => 'toHtml',
      PdfExtractionFormat.plainText => 'toPlainText',
      _ => 'extractText',
    };
    final pageIndex = switch (pages) {
      PdfSinglePage(:final index) => index,
      _ => null,
    };
    final r = await _sendWithSource(opName, source, {
      'password': password,
      if (pageIndex != null) 'page': pageIndex,
    });
    return r['value'] as String? ?? '';
  }

  // ── Search ──

  @override
  Future<List<SearchResult>> search(PdfSource source,
      {required String query, required PdfPages pages, String? password}) async {
    _checkDisposed();
    final pageIndex = switch (pages) {
      PdfSinglePage(:final index) => index,
      _ => null,
    };
    final op = pageIndex != null ? 'searchPage' : 'searchAll';
    final r = await _sendWithSource(op, source, {
      'query': query,
      'password': password,
      if (pageIndex != null) 'page': pageIndex,
    });
    final hits = r['hits'] as List? ?? [];
    return hits.map((h) {
      final m = h as Map<Object?, Object?>;
      return SearchResult(
        page: m['page'] as int,
        text: m['text'] as String? ?? '',
        rect: PdfRect(
          x: (m['x'] as num).toDouble(),
          y: (m['y'] as num).toDouble(),
          width: (m['width'] as num).toDouble(),
          height: (m['height'] as num).toDouble(),
        ),
      );
    }).toList();
  }

  // ── Security ──

  @override
  Future<void> watermark(PdfSource source, PdfSink output,
      {required String text, PdfPages pages = const PdfPages.all(),
       PdfWatermarkStyle style = const PdfWatermarkStyle(),
       PdfWatermarkPosition? position}) =>
    _sendWithSourceAndSink('watermark', source, output, {
      'text': text,
      'opacity': style.opacity,
      'fontSize': style.fontSize,
      'rotation': style.rotation,
      'r': style.color.r, 'g': style.color.g, 'b': style.color.b,
    });

  @override
  Future<void> encrypt(PdfSource source, PdfSink output,
      {required PdfEncryptionConfig encryption}) =>
    _sendWithSourceAndSink('encrypt', source, output, {
      'ownerPassword': encryption.ownerPassword,
      'userPassword': encryption.userPassword,
    });

  @override
  Future<void> decrypt(PdfSource source, PdfSink output,
      {required String password}) =>
    _sendWithSourceAndSink('decrypt', source, output, {'password': password});

  @override
  Future<void> sign(PdfSource source, PdfSink output,
      {required Uint8List certificate,
       required String certificatePassword,
       String? reason, String? location}) async {
    _checkDisposed();
    final session = await _pool.acquire();
    try {
      final filename = await streamSourceToOpfs(source, session, _opfs);
      try {
        final r = await session.send('sign', {
          'opfsFile': filename,
          'certificate': certificate.buffer,
          'certificatePassword': certificatePassword,
          'reason': reason,
          'location': location,
        });
        if (r['bytes'] != null) {
          await output.write(Uint8List.view(r['bytes'] as ByteBuffer));
        }
      } finally {
        await cleanupOpfsFile(filename, session, _opfs);
      }
    } finally {
      _pool.release(session);
    }
  }

  // ── Stamps ──

  @override
  Future<void> addStamp(PdfSource source, PdfSink output,
      {required int page, required PdfStampType type, required PdfRect rect,
       String? customName, double opacity = 1.0}) =>
    _sendWithSourceAndSink('addStamp', source, output, {
      'page': page,
      'stampType': type.index,
      'customName': customName,
      'x': rect.x, 'y': rect.y, 'width': rect.width, 'height': rect.height,
      'opacity': opacity,
    });

  @override
  Future<void> addImageStamp(PdfSource source, PdfSink output,
      {required int page, required Uint8List imageBytes, required PdfRect rect,
       double opacity = 1.0}) async {
    _checkDisposed();
    final session = await _pool.acquire();
    try {
      final filename = await streamSourceToOpfs(source, session, _opfs);
      try {
        final r = await session.send('addImageStamp', {
          'opfsFile': filename,
          'page': page,
          'imageBytes': imageBytes.buffer,
          'x': rect.x, 'y': rect.y, 'width': rect.width, 'height': rect.height,
          'opacity': opacity,
        });
        if (r['bytes'] != null) {
          await output.write(Uint8List.view(r['bytes'] as ByteBuffer));
        }
      } finally {
        await cleanupOpfsFile(filename, session, _opfs);
      }
    } finally {
      _pool.release(session);
    }
  }

  // ── Creation ──

  @override
  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output) async {
    _checkDisposed();
    final session = await _pool.acquire();
    try {
      final r = await session.send('imagesToPdf', {
        'images': images.map((i) => i.buffer).toList(),
      });
      if (r['bytes'] != null) {
        await output.write(Uint8List.view(r['bytes'] as ByteBuffer));
      }
    } finally {
      _pool.release(session);
    }
  }

  // ── Rendering ──

  @override
  Stream<RenderedPage> render(PdfSource source,
      {required PdfPages pages, PdfRenderSize? size, String? password}) async* {
    _checkDisposed();
    final pageList = await _resolvePages(source, pages, password: password);
    for (final pageIndex in pageList) {
      final r = await _sendWithSource('renderPage', source, {
        'pageIndex': pageIndex,
        'password': password,
        if (size != null) 'width': size.maxWidth,
        if (size != null) 'height': size.maxHeight,
      });
      final data = r['data'];
      if (data is ByteBuffer) {
        yield RenderedPage(
          width: r['width'] as int,
          height: r['height'] as int,
          data: Uint8List.view(data),
        );
      }
    }
  }

  // ── Image extraction ──

  @override
  Stream<PdfImage> extractImages(PdfSource source,
      {required PdfPages pages, String? password}) async* {
    _checkDisposed();
    final pageList = await _resolvePages(source, pages, password: password);
    for (final pageIndex in pageList) {
      final r = await _sendWithSource('extractImages', source, {
        'pageIndex': pageIndex,
        'password': password,
      });
      final imagesRaw = r['images'] as List<Object?>? ?? [];
      for (final item in imagesRaw) {
        final m = item as Map<Object?, Object?>;
        yield PdfImage(
          width: m['width'] as int,
          height: m['height'] as int,
          format: m['format'] as String? ?? '',
          colorSpace: m['colorSpace'] as String? ?? '',
          bitsPerComponent: m['bitsPerComponent'] as int? ?? 8,
          data: Uint8List.view(m['data'] as ByteBuffer),
        );
      }
    }
  }

  // ── Signatures ──

  @override
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
      {String? password}) async {
    _checkDisposed();
    final r = await _sendWithSource('getSignatures', source, {
      'password': password,
    });
    final sigsRaw = r['signatures'] as List<Object?>? ?? [];
    return sigsRaw.map((s) {
      final m = s as Map<Object?, Object?>;
      final timeStr = m['signingTime'] as String?;
      return PdfSignatureInfo(
        signerName: m['signerName'] as String?,
        reason: m['reason'] as String?,
        location: m['location'] as String?,
        signingTime: timeStr != null ? DateTime.tryParse(timeStr) : null,
        isValid: m['isValid'] as bool? ?? false,
      );
    }).toList();
  }

  @override
  Future<bool> verifySignatures(PdfSource source, {String? password}) async {
    _checkDisposed();
    final r = await _sendWithSource('verifySignatures', source, {
      'password': password,
    });
    return r['valid'] as bool? ?? false;
  }

  // ── Validation ──

  @override
  Future<PdfValidationResult> validatePdfA(PdfSource source,
      {int level = 2, String? password}) async {
    _checkDisposed();
    final r = await _sendWithSource('validatePdfA', source, {
      'level': level, 'password': password,
    });
    return PdfValidationResult(
      compliant: r['compliant'] as bool? ?? false,
      errors: r['errors'] as int? ?? 0,
      warnings: r['warnings'] as int? ?? 0,
    );
  }

  @override
  Future<bool> validatePdfUa(PdfSource source,
      {int level = 1, String? password}) async {
    _checkDisposed();
    final r = await _sendWithSource('validatePdfUa', source, {
      'level': level, 'password': password,
    });
    return r['accessible'] as bool? ?? false;
  }

  // ── Helpers ──

  /// Resolve sealed PdfPages into a list of 0-based page indices.
  Future<List<int>> _resolvePages(PdfSource source, PdfPages pages,
      {String? password}) async {
    switch (pages) {
      case PdfAllPages():
        final doc = await open(source, password: password);
        return List.generate(doc.pageCount, (i) => i);
      case PdfSinglePage(:final index):
        return [index];
      case PdfPageList(:final indices):
        return indices;
      case PdfPageRange(:final start, :final end):
        return List.generate(end - start, (i) => start + i);
    }
  }

  // ── Editor + Builder ──

  @override
  Future<BridgeEditorHandle> openEditor(PdfSource source, {String? password}) async {
    _checkDisposed();
    final session = await _pool.acquire();
    // Stream source to OPFS on this session's worker
    final filename = await streamSourceToOpfs(source, session, _opfs);
    // Open editor — session stays acquired for editor's lifetime
    final r = await session.send('editorOpen', {
      'opfsFile': filename, 'password': password,
    });
    final handleId = r['handleId'] as int;
    return _WebEditorHandle(this, session, handleId);
  }

  @override
  Future<BridgeBuilderHandle> createBuilder() async {
    _checkDisposed();
    final session = await _pool.acquire();
    final r = await session.send('builderCreate', {});
    final handleId = r['handleId'] as int;
    return _WebBuilderHandle(this, session, handleId);
  }

  // ── Lifecycle ──

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _pool.dispose();
    // OPFS files cleaned up by pool.dispose terminating workers
    // Any remaining tracked files are orphaned — browser cleans on origin clear
  }

  void _checkDisposed() {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
  }
}

// ── WebEditorHandle ─────────────────────────────────────────────────

class _WebEditorHandle implements BridgeEditorHandle {
  _WebEditorHandle(this._bridge, this._session, this._handleId);
  final WebBridge _bridge;
  final WebWorkerSession _session;
  final int _handleId;

  Future<Map<Object?, Object?>> _op(String method, [Map<String, Object?> extra = const {}]) =>
      _session.send('editor.$method', {'handleId': _handleId, ...extra});

  @override Future<int> get pageCount async => (await _op('pageCount'))['value'] as int;
  @override Future<String> get version async => (await _op('version'))['value'] as String? ?? '1.0';
  Future<bool> get isModified async => true;
  @override Future<String> getTitle() async => (await _op('getTitle'))['value'] as String? ?? '';
  @override Future<void> setTitle(String value) => _op('setTitle', {'value': value});
  @override Future<String> getAuthor() async => (await _op('getAuthor'))['value'] as String? ?? '';
  @override Future<void> setAuthor(String value) => _op('setAuthor', {'value': value});
  @override Future<String> getSubject() async => (await _op('getSubject'))['value'] as String? ?? '';
  @override Future<void> setSubject(String value) => _op('setSubject', {'value': value});
  @override Future<String> getKeywords() async => (await _op('getKeywords'))['value'] as String? ?? '';
  @override Future<void> setKeywords(String value) => _op('setKeywords', {'value': value});
  @override Future<void> rotatePage(int page, {required int degrees}) =>
      _op('rotatePage', {'page': page, 'degrees': degrees});
  @override Future<void> rotateAllPages({required int degrees}) =>
      _op('rotateAllPages', {'degrees': degrees});
  @override Future<PdfRect> getPageMediaBox(int page) async {
    final r = await _op('getPageMediaBox', {'page': page});
    return PdfRect(
      x: (r['x'] as num).toDouble(), y: (r['y'] as num).toDouble(),
      width: (r['width'] as num).toDouble(), height: (r['height'] as num).toDouble(),
    );
  }
  @override Future<void> deletePage(int page) => _op('deletePage', {'page': page});
  @override Future<void> movePage({required int from, required int to}) =>
      _op('movePage', {'from': from, 'to': to});
  @override Future<void> extractPages(List<int> pages, PdfSink output) async {
    final r = await _op('extractPages', {'pages': pages});
    if (r['bytes'] != null) await output.write(Uint8List.view(r['bytes'] as ByteBuffer));
  }
  @override Future<void> mergeFrom(PdfSource otherPdf) async {
    final bytes = await otherPdf.readAt(0, otherPdf.length);
    await _op('mergeFrom', {'bytes': bytes.buffer});
  }
  @override Future<int> optimizeImages({int quality = 75}) async =>
      (await _op('optimizeImages', {'quality': quality}))['value'] as int? ?? 0;
  @override Future<int> unembedStandardFonts() async =>
      (await _op('unembedStandardFonts'))['value'] as int? ?? 0;
  @override Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  }) => _op('addWatermark', {
    'page': page, 'text': text,
    'fontSize': style.fontSize, 'rotation': style.rotation,
    'opacity': style.opacity, 'r': style.color.r, 'g': style.color.g, 'b': style.color.b,
  });
  @override Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect,
    String? customName, double opacity = 1.0,
  }) => _op('addStamp', {
    'page': page, 'stampType': type.index,
    'x': rect.x, 'y': rect.y, 'width': rect.width, 'height': rect.height,
    'opacity': opacity,
  });
  @override Future<void> addImageStamp(int page, Uint8List imageBytes, {
    required PdfRect rect, double opacity = 1.0,
  }) => _op('addImageStamp', {
    'page': page, 'imageBytes': imageBytes.buffer,
    'x': rect.x, 'y': rect.y, 'width': rect.width, 'height': rect.height,
    'opacity': opacity,
  });
  @override Future<void> embedFile(String name, Uint8List data) =>
      _op('embedFile', {'name': name, 'data': data.buffer});
  @override Future<void> eraseRegions(int page, List<PdfRect> regions) {
    final flat = <double>[];
    for (final r in regions) { flat.addAll([r.x, r.y, r.width, r.height]); }
    return _op('eraseRegions', {'page': page, 'regions': flat});
  }
  @override Future<void> flattenForms() => _op('flattenForms');
  @override Future<void> flattenAllAnnotations() => _op('flattenAllAnnotations');
  @override Future<void> setFormFieldValue(String fieldName, String value) =>
      _op('setFormFieldValue', {'field': fieldName, 'value': value});
  @override Future<void> cropMargins({double left = 0, double right = 0,
      double top = 0, double bottom = 0}) =>
      _op('cropMargins', {'left': left, 'right': right, 'top': top, 'bottom': bottom});
  @override Future<void> convertToPdfA({int level = 1}) =>
      _op('convertToPdfA', {'level': level});
  @override Future<void> resizeImage(int page, String imageName,
      {required double width, required double height}) =>
      _op('resizeImage', {'page': page, 'imageName': imageName, 'width': width, 'height': height});
  Future<void> addRedaction(int page, PdfRect region, {String? overlayText}) =>
      _op('addRedaction', {'page': page, 'x': region.x, 'y': region.y,
          'width': region.width, 'height': region.height, 'overlayText': overlayText});
  Future<int> get redactionCount async =>
      (await _op('redactionCount'))['value'] as int? ?? 0;
  Future<void> applyRedactions() => _op('applyAllRedactions');
  Future<void> scrubMetadata() => _op('scrubMetadata');
  @override Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
    final r = await _op('save', {
      'compress': options.compress, 'garbageCollect': options.garbageCollect,
      'linearize': options.linearize,
    });
    if (r['bytes'] != null) await output.write(Uint8List.view(r['bytes'] as ByteBuffer));
  }
  @override Future<void> dispose() async {
    await _op('dispose');
    _bridge._pool.release(_session);
  }
}

// ── WebBuilderHandle ────────────────────────────────────────────────

class _WebBuilderHandle implements BridgeBuilderHandle {
  _WebBuilderHandle(this._bridge, this._session, this._handleId);
  final WebBridge _bridge;
  final WebWorkerSession _session;
  final int _handleId;

  Future<Map<Object?, Object?>> _op(String method, [Map<String, Object?> extra = const {}]) =>
      _session.send('builder.$method', {'handleId': _handleId, ...extra});

  @override Future<void> setTitle(String value) => _op('setTitle', {'value': value});
  @override Future<void> setAuthor(String value) => _op('setAuthor', {'value': value});
  @override Future<void> setSubject(String value) => _op('setSubject', {'value': value});
  @override Future<void> setKeywords(String value) => _op('setKeywords', {'value': value});

  @override Future<BridgePageBuilderHandle> addA4Page() async {
    await _op('addA4Page');
    return _WebPageBuilderHandle(_session, _handleId);
  }
  @override Future<BridgePageBuilderHandle> addLetterPage() async {
    await _op('addLetterPage');
    return _WebPageBuilderHandle(_session, _handleId);
  }
  @override Future<BridgePageBuilderHandle> addPage({required double width, required double height}) async {
    await _op('addPage', {'width': width, 'height': height});
    return _WebPageBuilderHandle(_session, _handleId);
  }
  @override Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
    final r = await _op('build');
    if (r['bytes'] != null) await output.write(Uint8List.view(r['bytes'] as ByteBuffer));
  }
  @override Future<void> dispose() async {
    await _op('dispose');
    _bridge._pool.release(_session);
  }
}

class _WebPageBuilderHandle implements BridgePageBuilderHandle {
  _WebPageBuilderHandle(this._session, this._builderHandleId);
  final WebWorkerSession _session;
  final int _builderHandleId;

  Future<Map<Object?, Object?>> _op(String method, [Map<String, Object?> extra = const {}]) =>
      _session.send('page.$method', {'handleId': _builderHandleId, ...extra});

  @override Future<void> font(String name, double size) => _op('font', {'name': name, 'size': size});
  @override Future<void> at(double x, double y) => _op('at', {'x': x, 'y': y});
  @override Future<void> text(String text) => _op('text', {'text': text});
  @override Future<void> heading(int level, String text) => _op('heading', {'level': level, 'text': text});
  @override Future<void> paragraph(String text) => _op('paragraph', {'text': text});
  @override Future<void> space(double points) => _op('space', {'points': points});
  @override Future<void> horizontalRule() => _op('horizontalRule');
  @override Future<void> image(Uint8List imageBytes, PdfRect rect, {String altText = ''}) =>
      _op('image', {'bytes': imageBytes.buffer, 'x': rect.x, 'y': rect.y,
          'width': rect.width, 'height': rect.height, 'altText': altText});
  @override Future<void> watermark(String text) => _op('watermark', {'text': text});
  @override Future<void> textField(String name, PdfRect rect, {String? defaultValue}) =>
      _op('textField', {'name': name, 'x': rect.x, 'y': rect.y,
          'w': rect.width, 'h': rect.height, 'defaultValue': defaultValue});
  @override Future<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _op('checkbox', {'name': name, 'x': rect.x, 'y': rect.y,
          'w': rect.width, 'h': rect.height, 'checked': checked});
  @override Future<void> comboBox(String name, PdfRect rect, List<String> options, {String? selected}) =>
      _op('comboBox', {'name': name, 'x': rect.x, 'y': rect.y,
          'w': rect.width, 'h': rect.height, 'options': options, 'selected': selected});
  @override Future<void> pushButton(String name, PdfRect rect, String caption) =>
      _op('pushButton', {'name': name, 'x': rect.x, 'y': rect.y,
          'w': rect.width, 'h': rect.height, 'caption': caption});
  @override Future<void> signatureField(String name, PdfRect rect) =>
      _op('signatureField', {'name': name, 'x': rect.x, 'y': rect.y,
          'w': rect.width, 'h': rect.height});
  @override Future<void> newline() => _op('newline');
  @override Future<void> newPageSameSize() => _op('newPageSameSize');
  @override Future<void> done() => _op('done');
}
