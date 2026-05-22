import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_info.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/pdf_sink.dart';
import 'package:pdf_manipulator/src/core/pdf_source.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';
import 'package:pdf_manipulator/src/page/pdf_page_info.dart';
import 'package:pdf_manipulator/src/platform/pdf_platform.dart';
import 'package:web/web.dart' as web;

extension _JSArrayBufferSlice on JSArrayBuffer {
  @JS('slice')
  external JSArrayBuffer _slice(int begin);
}

const _opfsChunkSize = 256 * 1024;

int _opfsCounter = 0;

/// Write result bytes from the worker into a [PdfSink].
Future<void> _writeTo(PdfSink output, Map<Object?, Object?> r) async {
  final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
  await output.write(bytes);
}

PdfPlatform createPlatform() => WebPdfPlatform._();

class WebPdfPlatform implements PdfPlatform {
  WebPdfPlatform._();

  web.Worker? _worker;
  Completer<void>? _ready;
  final _pending = <int, Completer<Object?>>{};
  int _nextId = 0;
  String _workerUrl = 'pdf_manipulator/worker.js';

  @override
  void configureWorkerUrl(String url) {
    _workerUrl = url;
  }

  Future<void> _ensureWorker() async {
    if (_worker != null && _ready != null && _ready!.isCompleted) return;
    if (_ready != null && !_ready!.isCompleted) return _ready!.future;

    _ready = Completer<void>();

    String effectiveUrl = _workerUrl;

    // Cross-origin URL: fetch script, rewrite relative imports to absolute,
    // create a same-origin Blob URL. ES module workers can import cross-origin
    // modules via CORS, so the rewritten absolute imports work.
    if (_workerUrl.startsWith('http://') || _workerUrl.startsWith('https://')) {
      try {
        final baseUrl = _workerUrl.substring(0, _workerUrl.lastIndexOf('/') + 1);
        final resp = await web.window.fetch(_workerUrl.toJS).toDart;
        if (!resp.ok) {
          throw Exception('Failed to fetch worker.js: ${resp.status}');
        }
        final text = (await resp.text().toDart).toDart;

        // Rewrite all relative ES module imports to absolute URLs
        final rewritten = text.replaceAllMapped(
          RegExp(r"""from\s+['"](\./[^'"]+)['"]"""),
          (m) => "from '$baseUrl${m.group(1)!.substring(2)}'",
        );

        final blob = web.Blob(
          [rewritten.toJS].toJS,
          web.BlobPropertyBag(type: 'application/javascript'),
        );
        effectiveUrl = web.URL.createObjectURL(blob);
      } catch (e) {
        _ready!.completeError(Exception(
          'pdf_manipulator: failed to load worker from $_workerUrl: $e',
        ));
        return;
      }
    }

    _worker = web.Worker(effectiveUrl.toJS, web.WorkerOptions(type: 'module'));

    _worker!.onmessage = (web.MessageEvent e) {
      final data = (e.data as JSAny).dartify()! as Map<Object?, Object?>;
      final type = data['type'] as String;
      if (type == 'ready') {
        _ready!.complete();
      } else if (type == 'result') {
        final id = data['id']! as int;
        _pending.remove(id)?.complete(data['result']);
      } else if (type == 'error') {
        final id = data['id']! as int;
        _pending.remove(id)?.completeError(Exception('${data['error']}'));
      }
    }.toJS;

    _worker!.onerror = (web.Event e) {
      if (_ready != null && !_ready!.isCompleted) {
        _ready!.completeError(Exception(
          'pdf_manipulator: Web Worker failed. '
          'Run `dart run pdf_manipulator:setup` to install WASM assets.',
        ));
      }
    }.toJS;

    await _ready!.future;
  }

  Future<Map<Object?, Object?>> _send(String op, Map<String, Object?> args) async {
    await _ensureWorker();
    final id = _nextId++;
    final completer = Completer<Object?>();
    _pending[id] = completer;

    // Build the JS message object manually (not via jsify()) so
    // ArrayBuffer references survive for the transfer list.
    //
    // ArrayBuffer.slice() creates an independent copy of the backing
    // memory. The slice is what we transfer (zero-copy across the
    // Worker boundary). The original Dart Uint8List stays usable —
    // callers can reuse the same bytes for multiple operations.
    //
    // Total cost: one memcpy (slice) + zero-copy transfer. Without
    // this, structured clone does two copies (serialize + deserialize).
    final jsArgs = JSObject();
    final transfers = <JSObject>[];
    for (final entry in args.entries) {
      final v = entry.value;
      if (v is ByteBuffer) {
        final original = v.toJS;
        final sliced = original._slice(0);
        jsArgs[entry.key] = sliced;
        transfers.add(sliced);
      } else if (v == null) {
        jsArgs[entry.key] = null;
      } else {
        jsArgs[entry.key] = v.jsify();
      }
    }

    final jsMsg = JSObject();
    jsMsg['id'] = id.toJS;
    jsMsg['op'] = op.toJS;
    jsMsg['args'] = jsArgs;

    if (transfers.isNotEmpty) {
      _worker!.postMessage(jsMsg, transfers.toJS);
    } else {
      _worker!.postMessage(jsMsg);
    }

    final result = await completer.future;
    return result as Map<Object?, Object?>;
  }

  List<Uint8List> _bytesList(Map<Object?, Object?> r) =>
      (r['chunks'] as List<Object?>).map((c) => Uint8List.view(c as ByteBuffer)).toList();

  /// Send an op with a PdfSource as primary input.
  /// Streams to OPFS in chunks, worker reads on demand via SyncAccessHandle.
  Future<Map<Object?, Object?>> _sendWithSource(
    String op,
    PdfSource source,
    Map<String, Object?> extraArgs,
  ) async {
    final filename = '_pdf_${_opfsCounter++}.tmp';
    var offset = 0;
    while (offset < source.length) {
      final count = (source.length - offset).clamp(0, _opfsChunkSize);
      final chunk = await source.readAt(offset, count);
      await _send('opfs.write', {
        'filename': filename,
        'chunk': chunk.buffer,
        'offset': offset,
      });
      offset += count;
    }
    await _send('opfs.finalize', {'filename': filename});
    return _send(op, {'opfsFile': filename, ...extraArgs});
  }

  // ── Inspect ──

  @override
  Future<PdfDoc> open(PdfSource source, {String? password}) async {
    final r = await _sendWithSource('open', source, {'password': password});
    final pagesRaw = r['pages'] as List;
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
    );
  }

  @override
  Future<PdfInfo> probe(PdfSource source) async {
    final r = await _sendWithSource('probe', source, {});
    return PdfInfo(
      isValid: r['isValid'] as bool,
      pageCount: r['pageCount'] as int?,
      isEncrypted: r['isEncrypted'] as bool? ?? false,
    );
  }

  // ── Structural ──

  @override
  Future<void> merge(List<PdfSource> inputs, PdfSink output) async {
    if (inputs.length < 2) throw ArgumentError('merge requires at least 2 PDFs');
    final inputBytes = <ByteBuffer>[];
    for (final src in inputs) {
      inputBytes.add((await src.readAt(0, src.length)).buffer);
    }
    final r = await _send('merge', {
      'inputs': inputBytes,
    });
    await _writeTo(output, r);
  }

  @override
  Future<void> split(PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int every}) async {
    final r = await _sendWithSource('split', source, {'every': every});
    final chunks = _bytesList(r);
    for (var i = 0; i < chunks.length; i++) {
      final sink = sinkFactory(i);
      await sink.write(chunks[i]);
    }
  }

  @override
  Future<int> splitBySize(PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int maxBytes}) async {
    final r = await _sendWithSource('splitBySize', source, {'maxBytes': maxBytes});
    final chunks = _bytesList(r);
    for (var i = 0; i < chunks.length; i++) {
      final sink = sinkFactory(i);
      await sink.write(chunks[i]);
    }
    return chunks.length;
  }

  @override
  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages}) async {
    final r = await _sendWithSource('extractPages', source, {'pages': pages});
    await _writeTo(output, r);
  }

  @override
  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages}) async {
    final r = await _sendWithSource('deletePages', source, {'pages': pages});
    await _writeTo(output, r);
  }

  @override
  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order}) async {
    final r = await _sendWithSource('reorderPages', source, {'order': order});
    await _writeTo(output, r);
  }

  @override
  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to}) async {
    final r = await _sendWithSource('movePage', source, {'from': from, 'to': to});
    await _writeTo(output, r);
  }

  @override
  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages}) async {
    final r = await _sendWithSource('rotatePages', source, {'pages': pages});
    await _writeTo(output, r);
  }

  @override
  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees}) async {
    final r = await _sendWithSource('rotateAllPages', source, {'degrees': degrees});
    await _writeTo(output, r);
  }

  // ── Content ──

  @override
  Future<void> flattenForms(PdfSource source, PdfSink output) async {
    final r = await _sendWithSource('flattenForms', source, {});
    await _writeTo(output, r);
  }

  @override
  Future<void> applyRedactions(PdfSource source, PdfSink output) async {
    final r = await _sendWithSource('applyRedactions', source, {});
    await _writeTo(output, r);
  }

  @override
  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData}) async {
    final r = await _sendWithSource('embedFile', source, {'name': name, 'fileData': fileData.buffer});
    await _writeTo(output, r);
  }

  @override
  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions}) async {
    final rects = <double>[];
    for (final reg in regions) {
      rects.addAll([reg.x, reg.y, reg.width, reg.height]);
    }
    final r = await _sendWithSource('eraseRegions', source, {
      'page': page,
      'rects': Float32List.fromList(rects).buffer,
    });
    await _writeTo(output, r);
  }

  @override
  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75, bool garbageCollect = true, bool linearize = false}) async {
    final r = await _sendWithSource('compress', source, {
      'imageQuality': imageQuality,
      'garbageCollect': garbageCollect, 'linearize': linearize,
    });
    await _writeTo(output, r);
  }

  // ── Extraction ──

  @override
  Future<String> extractText(PdfSource source, {int? page, String? password}) async {
    final r = await _sendWithSource('extractText', source, {
      'page': page, 'password': password,
    });
    return r['text'] as String;
  }

  @override
  Future<String> toMarkdown(PdfSource source, {int? page, String? password}) async {
    final r = await _sendWithSource('toMarkdown', source, {
      'page': page, 'password': password,
    });
    return r['text'] as String;
  }

  @override
  Future<String> toHtml(PdfSource source, {required int page, String? password}) async {
    final r = await _sendWithSource('toHtml', source, {
      'page': page, 'password': password,
    });
    return r['text'] as String;
  }

  @override
  Future<String> toPlainText(PdfSource source, {required int page, String? password}) async {
    final r = await _sendWithSource('toPlainText', source, {
      'page': page, 'password': password,
    });
    return r['text'] as String;
  }

  // ── Search ──

  @override
  Future<List<SearchResult>> searchPage(PdfSource source,
      {required int page, required String query, String? password}) async {
    final r = await _sendWithSource('searchPage', source, {
      'page': page, 'query': query, 'password': password,
    });
    return _parseSearchResults(r['results'] as List<Object?>);
  }

  @override
  Future<List<SearchResult>> searchAll(PdfSource source,
      {required String query, String? password}) async {
    final r = await _sendWithSource('searchAll', source, {
      'query': query, 'password': password,
    });
    return _parseSearchResults(r['results'] as List<Object?>);
  }

  List<SearchResult> _parseSearchResults(List<Object?> raw) => raw.map((item) {
        final m = item as Map<Object?, Object?>;
        return SearchResult(
          text: m['text'] as String? ?? '',
          page: m['page'] as int? ?? 0,
          rect: PdfRect(
            x: (m['x'] as num?)?.toDouble() ?? 0,
            y: (m['y'] as num?)?.toDouble() ?? 0,
            width: (m['width'] as num?)?.toDouble() ?? 0,
            height: (m['height'] as num?)?.toDouble() ?? 0,
          ),
        );
      }).toList();

  // ── Security ──

  @override
  Future<void> watermark(PdfSource source, PdfSink output,
      {required String text, List<int>? pages, double opacity = 0.3,
       double fontSize = 48, double rotation = 45,
       double r = 0.5, double g = 0.5, double b = 0.5}) async {
    final res = await _sendWithSource('watermark', source, {
      'text': text, 'pages': pages,
      'opacity': opacity, 'fontSize': fontSize, 'rotation': rotation,
      'r': r, 'g': g, 'b': b,
    });
    await _writeTo(output, res);
  }

  @override
  Future<void> watermarkPositioned(PdfSource source, PdfSink output, {
    required String text,
    required double x, required double y,
    required double width, required double height,
    List<int>? pages,
    double fontSize = 48, String? fontName,
    double rotation = 45, double opacity = 0.3,
    double r = 0.5, double g = 0.5, double b = 0.5,
    bool fixedPrint = false,
    double fixedPrintH = 0.0,
    double fixedPrintV = 0.0,
  }) async {
    final res = await _sendWithSource('watermarkPositioned', source, {
      'text': text, 'pages': pages,
      'x': x, 'y': y, 'width': width, 'height': height,
      'fontSize': fontSize, 'fontName': fontName,
      'rotation': rotation, 'opacity': opacity,
      'r': r, 'g': g, 'b': b,
      'fixedPrint': fixedPrint, 'fixedPrintH': fixedPrintH, 'fixedPrintV': fixedPrintV,
    });
    await _writeTo(output, res);
  }

  @override
  Future<void> encrypt(PdfSource source, PdfSink output,
      {required String ownerPassword, String userPassword = ''}) async {
    final r = await _sendWithSource('encrypt', source, {
      'ownerPassword': ownerPassword, 'userPassword': userPassword,
    });
    await _writeTo(output, r);
  }

  @override
  Future<void> encryptFull(PdfSource source, PdfSink output, {
    required String ownerPassword,
    String userPassword = '',
    int algorithm = 3,
    bool allowPrint = true,
    bool allowPrintHq = true,
    bool allowModify = true,
    bool allowCopy = true,
    bool allowAnnotate = true,
    bool allowFillForms = true,
    bool allowAccessibility = true,
    bool allowAssemble = true,
  }) async {
    final r = await _sendWithSource('encryptFull', source, {
      'ownerPassword': ownerPassword,
      'userPassword': userPassword, 'algorithm': algorithm,
      'allowPrint': allowPrint, 'allowPrintHq': allowPrintHq,
      'allowModify': allowModify, 'allowCopy': allowCopy,
      'allowAnnotate': allowAnnotate, 'allowFillForms': allowFillForms,
      'allowAccessibility': allowAccessibility, 'allowAssemble': allowAssemble,
    });
    await _writeTo(output, r);
  }

  @override
  Future<void> decrypt(PdfSource source, PdfSink output,
      {required String password}) async {
    final r = await _sendWithSource('decrypt', source, {'password': password});
    await _writeTo(output, r);
  }

  @override
  Future<void> sign(PdfSource source, PdfSink output,
      {required Uint8List certificate, required String certificatePassword,
       String? reason, String? location}) async {
    final r = await _sendWithSource('sign', source, {
      'certificate': certificate.buffer,
      'certificatePassword': certificatePassword,
      'reason': reason, 'location': location,
    });
    await _writeTo(output, r);
  }

  // ── Creation ──

  @override
  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output) async {
    if (images.isEmpty) throw ArgumentError('images must not be empty');
    final r = await _send('imagesToPdf', {
      'images': images.map((i) => i.buffer).toList(),
    });
    await _writeTo(output, r);
  }

  // ── Rendering ──

  @override
  Future<RenderedPage> renderPage(PdfSource source, int pageIndex,
      {String? password}) async {
    final r = await _sendWithSource('renderPage', source, {
      'pageIndex': pageIndex, 'password': password,
    });
    return _parseRenderedPage(r);
  }

  @override
  Future<RenderedPage> renderPageFit(PdfSource source, int pageIndex,
      {required int width, required int height, String? password}) async {
    final r = await _sendWithSource('renderPageFit', source, {
      'pageIndex': pageIndex,
      'width': width, 'height': height, 'password': password,
    });
    return _parseRenderedPage(r);
  }

  @override
  Future<RenderedPage> renderPageThumbnail(PdfSource source, int pageIndex,
      {required int size, String? password}) async {
    final r = await _sendWithSource('renderPageThumbnail', source, {
      'pageIndex': pageIndex,
      'size': size, 'password': password,
    });
    return _parseRenderedPage(r);
  }

  @override
  Stream<RenderedPage> renderAllPages(PdfSource source,
      {required int width, required int height, String? password}) async* {
    final r = await _sendWithSource('renderAllPages', source, {
      'width': width, 'height': height, 'password': password,
    });
    for (final page in (r['pages'] as List<Object?>).map(_parseRenderedPage)) {
      yield page;
    }
  }

  RenderedPage _parseRenderedPage(Object? raw) {
    final m = raw as Map<Object?, Object?>;
    return RenderedPage(
      width: m['width'] as int,
      height: m['height'] as int,
      data: Uint8List.view(m['data'] as ByteBuffer),
    );
  }

  // ── Image extraction ──

  @override
  Stream<PdfImage> extractImages(PdfSource source, int pageIndex,
      {String? password}) async* {
    final r = await _sendWithSource('extractImages', source, {
      'pageIndex': pageIndex, 'password': password,
    });
    for (final img in _parseImages(r['images'] as List<Object?>)) {
      yield img;
    }
  }

  @override
  Stream<PdfImage> extractAllImages(PdfSource source,
      {String? password}) async* {
    final r = await _sendWithSource('extractAllImages', source, {
      'password': password,
    });
    for (final img in _parseImages(r['images'] as List<Object?>)) {
      yield img;
    }
  }

  List<PdfImage> _parseImages(List<Object?> raw) => raw.map((item) {
        final m = item as Map<Object?, Object?>;
        return PdfImage(
          width: m['width'] as int,
          height: m['height'] as int,
          format: m['format'] as String? ?? '',
          colorSpace: m['colorSpace'] as String? ?? '',
          bitsPerComponent: m['bitsPerComponent'] as int? ?? 8,
          data: Uint8List.view(m['data'] as ByteBuffer),
        );
      }).toList();

  // ── Signatures ──

  @override
  Future<int> getSignatureCount(PdfSource source, {String? password}) async {
    final r = await _sendWithSource('getSignatureCount', source, {
      'password': password,
    });
    return r['count'] as int;
  }

  @override
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
      {String? password}) async {
    final r = await _sendWithSource('getSignatures', source, {
      'password': password,
    });
    return (r['signatures'] as List<Object?>).map((s) {
      final m = s as Map<Object?, Object?>;
      return PdfSignatureInfo(
        signerName: m['signerName'] as String?,
        isValid: m['isValid'] as bool? ?? false,
      );
    }).toList();
  }

  @override
  Future<bool> verifySignatures(PdfSource source, {String? password}) async {
    final r = await _sendWithSource('verifySignatures', source, {
      'password': password,
    });
    return r['valid'] as bool? ?? true;
  }

  // ── Validation ──

  @override
  Future<({bool compliant, int errors, int warnings})> validatePdfA(
      PdfSource source, {int level = 2, String? password}) async {
    final levels = ['1b', '1a', '2b', '2a', '2u', '3b', '3a', '3u'];
    final r = await _sendWithSource('validatePdfA', source, {
      'level': level < levels.length ? levels[level] : '2b',
      'password': password,
    });
    return (
      compliant: r['compliant'] as bool? ?? false,
      errors: r['errors'] as int? ?? 0,
      warnings: r['warnings'] as int? ?? 0,
    );
  }

  @override
  Future<bool> validatePdfUa(PdfSource source, {int level = 1, String? password}) async {
    final r = await _sendWithSource('validatePdfUa', source, {
      'password': password,
    });
    return r['accessible'] as bool? ?? false;
  }

  // ── Encryption info ──

  @override
  Future<({bool print, bool printHq, bool modify, bool copy, bool annotate,
      bool fillForms, bool accessibility, bool assemble})>
    getPermissions(PdfSource source, {String? password}) async {
    final r = await _sendWithSource('getPermissions', source, {
      'password': password,
    });
    return (
      print: r['print'] as bool? ?? true,
      printHq: r['printHq'] as bool? ?? true,
      modify: r['modify'] as bool? ?? true,
      copy: r['copy'] as bool? ?? true,
      annotate: r['annotate'] as bool? ?? true,
      fillForms: r['fillForms'] as bool? ?? true,
      accessibility: r['accessibility'] as bool? ?? true,
      assemble: r['assemble'] as bool? ?? true,
    );
  }

  @override
  Future<int> getEncryptionAlgorithm(PdfSource source, {String? password}) async {
    final r = await _sendWithSource('getEncryptionAlgorithm', source, {
      'password': password,
    });
    return r['algorithm'] as int? ?? -1;
  }

  // ── Editor ──

  @override
  Future<PdfEditorHandle> openEditor(PdfSource source) async {
    final r = await _sendWithSource('editorOpen', source, {});
    final handleId = r['handleId'] as int;
    return _WebEditorHandle(this, handleId);
  }

  // ── Builder ──

  @override
  Future<PdfBuilderHandle> createBuilder() async {
    final r = await _send('builderCreate', {});
    final handleId = r['handleId'] as int;
    return _WebBuilderHandle(this, handleId);
  }

  // ── Lifecycle ──

  @override
  Future<void> dispose() async {
    _worker?.terminate();
    _worker = null;
    _ready = null;
    for (final c in _pending.values) {
      c.completeError(Exception('Worker terminated'));
    }
    _pending.clear();
  }
}

class _WebEditorHandle implements PdfEditorHandle {
  final WebPdfPlatform _p;
  final int _id;
  _WebEditorHandle(this._p, this._id);

  Future<Map<Object?, Object?>> _op(String method, [Map<String, Object?>? args]) =>
      _p._send('editor.$method', {'handleId': _id, ...?args});

  @override Future<int> get pageCount async => (await _op('pageCount'))['value'] as int;
  @override Future<String> get version async => (await _op('version'))['value'] as String;
  @override Future<bool> get isModified async => (await _op('isModified'))['value'] as bool;

  @override Future<String> getTitle() async => (await _op('getTitle'))['value'] as String? ?? '';
  @override Future<void> setTitle(String v) => _op('setTitle', {'value': v});
  @override Future<String> getAuthor() async => (await _op('getAuthor'))['value'] as String? ?? '';
  @override Future<void> setAuthor(String v) => _op('setAuthor', {'value': v});
  @override Future<String> getSubject() async => (await _op('getSubject'))['value'] as String? ?? '';
  @override Future<void> setSubject(String v) => _op('setSubject', {'value': v});
  @override Future<String> getKeywords() async => (await _op('getKeywords'))['value'] as String? ?? '';
  @override Future<void> setKeywords(String v) => _op('setKeywords', {'value': v});

  @override Future<void> rotatePage(int i, {required int degrees}) =>
      _op('rotatePage', {'page': i, 'degrees': degrees});
  @override Future<void> rotateAllPages({required int degrees}) =>
      _op('rotateAllPages', {'degrees': degrees});
  @override Future<PdfRect> getPageMediaBox(int i) async {
    final r = await _op('getPageMediaBox', {'page': i});
    return PdfRect(
      x: (r['x'] as num).toDouble(), y: (r['y'] as num).toDouble(),
      width: (r['width'] as num).toDouble(), height: (r['height'] as num).toDouble(),
    );
  }
  @override Future<void> deletePage(int i) => _op('deletePage', {'page': i});
  @override Future<void> movePage({required int from, required int to}) =>
      _op('movePage', {'from': from, 'to': to});
  @override Future<void> extractPages(List<int> pages, PdfSink output) async {
    final r = await _op('extractPages', {'pages': pages});
    final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
    await output.write(bytes);
  }
  @override Future<void> mergeFrom(PdfSource otherPdf) async {
    final bytes = await otherPdf.readAt(0, otherPdf.length);
    await _op('mergeFrom', {'bytes': bytes.buffer});
  }

  @override Future<int> optimizeImages({int quality = 75}) async =>
      (await _op('optimizeImages', {'quality': quality}))['value'] as int;
  @override Future<int> unembedStandardFonts() async =>
      (await _op('unembedStandardFonts'))['value'] as int;
  @override Future<void> addWatermark(int i, String text,
      {double fontSize = 48, double rotation = 45, double opacity = 0.3,
       double r = 0.5, double g = 0.5, double b = 0.5}) =>
      _op('addWatermark', {'page': i, 'text': text, 'fontSize': fontSize,
          'rotation': rotation, 'opacity': opacity, 'r': r, 'g': g, 'b': b});

  @override Future<void> embedFile(String name, Uint8List data) =>
      _op('embedFile', {'name': name, 'data': data.buffer});
  @override Future<void> eraseRegions(int i, List<PdfRect> regions) {
    final rects = <double>[];
    for (final reg in regions) {
      rects.addAll([reg.x, reg.y, reg.width, reg.height]);
    }
    return _op('eraseRegions', {'page': i, 'rects': Float32List.fromList(rects).buffer});
  }
  @override Future<void> flattenForms() => _op('flattenForms');
  @override Future<void> flattenAllAnnotations() => _op('flattenAllAnnotations');
  @override Future<void> applyAllRedactions() => _op('applyAllRedactions');
  @override Future<void> setFormFieldValue(String field, String value) =>
      _op('setFormFieldValue', {'field': field, 'value': value});
  @override Future<void> cropMargins({double left = 0, double right = 0,
      double top = 0, double bottom = 0}) =>
      _op('cropMargins', {'left': left, 'right': right, 'top': top, 'bottom': bottom});
  @override Future<void> convertToPdfA({int level = 1}) =>
      _op('convertToPdfA', {'level': level});

  @override Future<void> save(PdfSink output) async {
    final r = await _op('save');
    final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
    await output.write(bytes);
  }
  @override Future<void> saveWithOptions(PdfSink output, {bool compress = true,
      bool garbageCollect = true, bool linearize = false}) async {
    final r = await _op('saveWithOptions', {
      'compress': compress, 'garbageCollect': garbageCollect, 'linearize': linearize});
    final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
    await output.write(bytes);
  }
  @override Future<void> saveEncrypted(PdfSink output, {required String ownerPassword,
      String userPassword = ''}) async {
    final r = await _op('saveEncrypted', {
      'ownerPassword': ownerPassword, 'userPassword': userPassword});
    final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
    await output.write(bytes);
  }

  @override Future<void> saveEncryptedFull(PdfSink output, {
    required String ownerPassword,
    String userPassword = '',
    int algorithm = 3,
    bool allowPrint = true,
    bool allowPrintHq = true,
    bool allowModify = true,
    bool allowCopy = true,
    bool allowAnnotate = true,
    bool allowFillForms = true,
    bool allowAccessibility = true,
    bool allowAssemble = true,
  }) async {
    final r = await _op('saveEncryptedFull', {
      'ownerPassword': ownerPassword, 'userPassword': userPassword,
      'algorithm': algorithm,
      'allowPrint': allowPrint, 'allowPrintHq': allowPrintHq,
      'allowModify': allowModify, 'allowCopy': allowCopy,
      'allowAnnotate': allowAnnotate, 'allowFillForms': allowFillForms,
      'allowAccessibility': allowAccessibility, 'allowAssemble': allowAssemble,
    });
    final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
    await output.write(bytes);
  }

  @override Future<void> addWatermarkPositioned(int i, String text, {
    required double x, required double y,
    required double width, required double height,
    double fontSize = 48, String? fontName,
    double rotation = 45, double opacity = 0.3,
    double r = 0.5, double g = 0.5, double b = 0.5,
  }) => _op('addWatermarkPositioned', {
    'page': i, 'text': text, 'x': x, 'y': y,
    'width': width, 'height': height,
    'fontSize': fontSize, 'fontName': fontName,
    'rotation': rotation, 'opacity': opacity,
    'r': r, 'g': g, 'b': b,
  });

  @override Future<void> addStamp(int i, {
    required int stampType,
    String? customName,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) => _op('addStamp', {
    'page': i, 'stampType': stampType, 'customName': customName,
    'x': x, 'y': y, 'width': width, 'height': height,
    'opacity': opacity,
  });

  @override Future<void> addImageStamp(int i, Uint8List imageBytes, {
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) => _op('addImageStamp', {
    'page': i, 'imageBytes': imageBytes.buffer,
    'x': x, 'y': y, 'width': width, 'height': height, 'opacity': opacity,
  });

  @override Future<void> resizeImage(int i, String imageName,
      {required double width, required double height}) =>
      _op('resizeImage', {'page': i, 'imageName': imageName,
          'width': width, 'height': height});

  @override Future<void> dispose() => _op('dispose');
}

class _WebBuilderHandle implements PdfBuilderHandle {
  final WebPdfPlatform _p;
  final int _id;
  _WebBuilderHandle(this._p, this._id);

  Future<Map<Object?, Object?>> _op(String method, [Map<String, Object?>? args]) =>
      _p._send('builder.$method', {'handleId': _id, ...?args});

  @override Future<void> setTitle(String v) => _op('setTitle', {'value': v});
  @override Future<void> setAuthor(String v) => _op('setAuthor', {'value': v});
  @override Future<void> setSubject(String v) => _op('setSubject', {'value': v});
  @override Future<void> setKeywords(String v) => _op('setKeywords', {'value': v});

  @override Future<PdfPageBuilderHandle> addA4Page() async {
    final r = await _op('addA4Page');
    return _WebPageBuilderHandle(_p, r['handleId'] as int);
  }
  @override Future<PdfPageBuilderHandle> addLetterPage() async {
    final r = await _op('addLetterPage');
    return _WebPageBuilderHandle(_p, r['handleId'] as int);
  }
  @override Future<PdfPageBuilderHandle> addPage({required double width, required double height}) async {
    final r = await _op('addPage', {'width': width, 'height': height});
    return _WebPageBuilderHandle(_p, r['handleId'] as int);
  }

  @override Future<void> build(PdfSink output) async {
    final r = await _op('build');
    final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
    await output.write(bytes);
  }
  @override Future<void> buildEncrypted(PdfSink output, {required String ownerPassword,
      String userPassword = ''}) async {
    final r = await _op('buildEncrypted', {
      'ownerPassword': ownerPassword, 'userPassword': userPassword});
    final bytes = Uint8List.view(r['bytes'] as ByteBuffer);
    await output.write(bytes);
  }

  @override Future<void> dispose() => _op('dispose');
}

class _WebPageBuilderHandle implements PdfPageBuilderHandle {
  final WebPdfPlatform _p;
  final int _id;
  _WebPageBuilderHandle(this._p, this._id);

  Future<Map<Object?, Object?>> _op(String method, [Map<String, Object?>? args]) =>
      _p._send('page.$method', {'handleId': _id, ...?args});

  @override Future<void> font(String name, double size) =>
      _op('font', {'name': name, 'size': size});
  @override Future<void> at(double x, double y) => _op('at', {'x': x, 'y': y});
  @override Future<void> text(String text) => _op('text', {'text': text});
  @override Future<void> heading(int level, String text) =>
      _op('heading', {'level': level, 'text': text});
  @override Future<void> paragraph(String text) => _op('paragraph', {'text': text});
  @override Future<void> space(double points) => _op('space', {'points': points});
  @override Future<void> horizontalRule() => _op('horizontalRule');
  @override Future<void> image(Uint8List imageBytes, double x, double y,
      double width, double height, {String altText = ''}) =>
      _op('image', {'bytes': imageBytes.buffer, 'x': x, 'y': y,
          'width': width, 'height': height, 'altText': altText});
  @override Future<void> watermark(String text) => _op('watermark', {'text': text});

  // ── Form fields ──

  @override Future<void> textField(String name, double x, double y, double w,
      double h, {String? defaultValue}) =>
      _op('textField', {'name': name, 'x': x, 'y': y, 'w': w, 'h': h,
          'defaultValue': defaultValue});
  @override Future<void> checkbox(String name, double x, double y, double w,
      double h, {bool checked = false}) =>
      _op('checkbox', {'name': name, 'x': x, 'y': y, 'w': w, 'h': h,
          'checked': checked});
  @override Future<void> comboBox(String name, double x, double y, double w,
      double h, List<String> options, {String? selected}) =>
      _op('comboBox', {'name': name, 'x': x, 'y': y, 'w': w, 'h': h,
          'options': options, 'selected': selected});
  @override Future<void> pushButton(String name, double x, double y, double w,
      double h, String caption) =>
      _op('pushButton', {'name': name, 'x': x, 'y': y, 'w': w, 'h': h,
          'caption': caption});
  @override Future<void> signatureField(String name, double x, double y,
      double w, double h) =>
      _op('signatureField', {'name': name, 'x': x, 'y': y, 'w': w, 'h': h});

  @override Future<void> radioGroup(String name, List<String> values,
      List<double> xs, List<double> ys, List<double> ws, List<double> hs,
      {String? selected}) {
    final flat = <double>[];
    for (var i = 0; i < xs.length; i++) {
      flat.addAll([xs[i], ys[i], ws[i], hs[i]]);
    }
    return _op('radioGroup', {'name': name, 'values': values,
        'rects': flat, 'selected': selected});
  }

  @override Future<void> fieldKeystroke(String script) =>
      _op('fieldKeystroke', {'script': script});
  @override Future<void> fieldFormat(String script) =>
      _op('fieldFormat', {'script': script});
  @override Future<void> fieldValidate(String script) =>
      _op('fieldValidate', {'script': script});
  @override Future<void> fieldCalculate(String script) =>
      _op('fieldCalculate', {'script': script});

  @override Future<void> linkUrl(String url) =>
      _op('linkUrl', {'url': url});
  @override Future<void> linkPage(int targetPage) =>
      _op('linkPage', {'targetPage': targetPage});

  @override Future<void> footnote(String refMark, String noteText) =>
      _op('footnote', {'refMark': refMark, 'noteText': noteText});
  @override Future<void> columns(int columnCount, double gapPt, String text) =>
      _op('columns', {'columnCount': columnCount, 'gapPt': gapPt, 'text': text});
  @override Future<void> newline() => _op('newline');
  @override Future<void> newPageSameSize() => _op('newPageSameSize');

  @override Future<void> done() => _op('done');
}
