import 'dart:async';
import 'dart:ffi' as ffi;
import 'dart:isolate';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_info.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/pdf_sink.dart';
import 'package:pdf_manipulator/src/core/pdf_source.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';
import 'package:pdf_manipulator/src/ffi/bindings.dart';
import 'package:pdf_manipulator/src/ffi/streaming_sink.dart';
import 'package:pdf_manipulator/src/ffi/streaming_source.dart';
import 'package:pdf_manipulator/src/page/pdf_page_info.dart';
import 'package:pdf_manipulator/src/platform/_msg.dart';
import 'package:pdf_manipulator/src/platform/_op.dart';
import 'package:pdf_manipulator/src/platform/pdf_platform.dart';

PdfPlatform createPlatform() => NativePdfPlatform._();

// ════════════════════════════════════════════════════════════════════════════
// NativePdfPlatform — message-based isolate protocol (zero-copy)
//
// No closures cross the isolate boundary. Every request is a WorkerMsg
// (primitives + TransferableTypedData). Every response is a WorkerResult
// (primitives + TransferableTypedData). The worker entry function has a
// single switch(msg.op) that handles all operations via PdfBindings.
// ════════════════════════════════════════════════════════════════════════════

class NativePdfPlatform implements PdfPlatform {
  NativePdfPlatform._();

  SendPort? _workerPort;
  Isolate? _workerIsolate;
  final _pending = <int, Completer<Object?>>{};
  final _pendingStreams = <int, StreamController<Object?>>{};
  int _nextId = 0;
  ReceivePort? _responsePort;
  static int _nextHandleId = 0;
  final _sourceServers = <int, SourceServer>{};

  Future<void> _ensureWorker() async {
    if (_workerPort != null) return;

    final initPort = ReceivePort();
    _workerIsolate = await Isolate.spawn(
      _workerEntry,
      initPort.sendPort,
      debugName: 'PdfManipulatorWorker',
    );
    _workerPort = await initPort.first as SendPort;

    _responsePort = ReceivePort();
    _responsePort!.listen((message) {
      if (message is WorkerResult) {
        final completer = _pending.remove(message.id);
        if (completer != null) {
          if (message.error != null) {
            completer.completeError(message.error!);
          } else {
            completer.complete(message.value);
          }
        } else {
          // Streaming ops use _pendingStreams — errors from the catch
          // block arrive as WorkerResult, not WorkerStreamItem.
          final controller = _pendingStreams.remove(message.id);
          if (controller != null) {
            if (message.error != null) {
              controller.addError(message.error!);
            }
            controller.close();
          }
        }
      } else if (message is WorkerStreamItem) {
        final controller = _pendingStreams[message.id];
        if (controller == null) return;
        if (message.error != null) {
          controller.addError(message.error!);
          _pendingStreams.remove(message.id);
          controller.close();
        } else if (message.done) {
          _pendingStreams.remove(message.id);
          controller.close();
        } else {
          controller.add(message.value);
        }
      }
    });

    _workerPort!.send(_responsePort!.sendPort);
  }

  /// Send a typed WorkerMsg to the worker isolate and return the result.
  Future<Object?> _send(WorkerMsg msg) async {
    await _ensureWorker();
    final completer = Completer<Object?>();
    _pending[msg.id] = completer;
    _workerPort!.send(msg);
    return completer.future;
  }

  int _id() => _nextId++;

  /// Send an op with a PdfSource as primary input using the streaming
  /// protocol (cross-isolate reads, no full-buffer transfer).
  /// Send a streaming op with a PdfSource. Returns a Stream that yields
  /// items one at a time as the worker produces them.
  Stream<T> _sendStreamWithSource<T>(Op op, PdfSource source,
      {Map<String, Object?> args = const {}}) async* {
    await _ensureWorker();
    final id = _id();
    final server = SourceServer(source);
    final serverPort = server.start();
    final controller = StreamController<Object?>();
    _pendingStreams[id] = controller;
    _workerPort!.send(WorkerMsg(
      id: id,
      op: op,
      sourcePort: serverPort,
      args: args,
    ));
    try {
      await for (final item in controller.stream) {
        yield item as T;
      }
    } finally {
      server.stop();
    }
  }

  /// Send an op with a streaming source, and optionally a streaming sink.
  Future<Object?> _sendWithSource(Op op, PdfSource source,
      {PdfSink? sink, Map<String, Object?> args = const {}}) async {
    final srcServer = SourceServer(source);
    final srcPort = srcServer.start();
    SinkServer? snkServer;
    SendPort? snkPort;
    if (sink != null) {
      snkServer = SinkServer(sink);
      snkPort = snkServer.start();
    }
    try {
      return await _send(WorkerMsg(
        id: _id(), op: op, sourcePort: srcPort, sinkPort: snkPort, args: args,
      ));
    } finally {
      srcServer.stop();
      snkServer?.stop();
    }
  }

  // ── Inspect ──

  @override
  Future<PdfDoc> open(PdfSource source, {String? password}) async {
    final result = await _sendWithSource(Op.open, source,
      args: {'password': password},
    );
    return result as PdfDoc;
  }

  @override
  Future<PdfInfo> probe(PdfSource source) async {
    final result = await _sendWithSource(Op.probe, source);
    return result as PdfInfo;
  }

  // ── Structural ──

  @override
  Future<void> merge(List<PdfSource> inputs, PdfSink output) async {
    if (inputs.length < 2) throw ArgumentError('merge requires at least 2 PDFs');
    final srcServer = SourceServer(inputs[0]);
    final srcPort = srcServer.start();
    final snkServer = SinkServer(output);
    final snkPort = snkServer.start();
    final remaining = <Uint8List>[];
    for (var i = 1; i < inputs.length; i++) {
      remaining.add(await inputs[i].readAt(0, inputs[i].length));
    }
    try {
      await _send(WorkerMsg(
        id: _id(),
        op: Op.merge,
        sourcePort: srcPort,
        sinkPort: snkPort,
        bytesList: transferList(remaining),
      ));
    } finally {
      srcServer.stop();
      snkServer.stop();
    }
  }

  @override
  Future<void> split(PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int every}) async {
    if (every < 1) throw ArgumentError('every must be >= 1');
    final result = await _sendWithSource(Op.split, source,
      args: {'every': every},
    );
    final chunks = materializeList(result as List<TransferableTypedData>);
    for (var i = 0; i < chunks.length; i++) {
      final sink = sinkFactory(i);
      await sink.write(chunks[i]);
    }
  }

  @override
  Future<int> splitBySize(
      PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int maxBytes}) async {
    if (maxBytes < 1) throw ArgumentError('maxBytes must be >= 1');
    final result = await _sendWithSource(Op.splitBySize, source,
      args: {'maxBytes': maxBytes},
    );
    final chunks = materializeList(result as List<TransferableTypedData>);
    for (var i = 0; i < chunks.length; i++) {
      final sink = sinkFactory(i);
      await sink.write(chunks[i]);
    }
    return chunks.length;
  }

  @override
  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages}) async {
    await _sendWithSource(Op.extractPages, source, sink: output,
      args: {'pages': pages},
    );
  }

  @override
  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages}) async {
    await _sendWithSource(Op.deletePages, source, sink: output,
      args: {'pages': pages},
    );
  }

  @override
  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order}) async {
    await _sendWithSource(Op.reorderPages, source, sink: output,
      args: {'order': order},
    );
  }

  @override
  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to}) async {
    await _sendWithSource(Op.movePage, source, sink: output,
      args: {'from': from, 'to': to},
    );
  }

  @override
  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages}) async {
    await _sendWithSource(Op.rotatePages, source, sink: output,
      args: {'pages': pages},
    );
  }

  @override
  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees}) async {
    await _sendWithSource(Op.rotateAllPages, source, sink: output,
      args: {'degrees': degrees},
    );
  }

  // ── Content ──

  @override
  Future<void> flattenForms(PdfSource source, PdfSink output) async {
    await _sendWithSource(Op.flattenForms, source, sink: output);
  }

  @override
  Future<void> applyRedactions(PdfSource source, PdfSink output) async {
    await _sendWithSource(Op.applyRedactions, source, sink: output);
  }

  @override
  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData}) async {
    final srcServer = SourceServer(source);
    final srcPort = srcServer.start();
    final snkServer = SinkServer(output);
    final snkPort = snkServer.start();
    try {
      await _send(WorkerMsg(
        id: _id(),
        op: Op.embedFile,
        sourcePort: srcPort,
        sinkPort: snkPort,
        bytesList: [transfer(fileData)],
        args: {'name': name},
      ));
    } finally {
      srcServer.stop();
      snkServer.stop();
    }
  }

  @override
  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions}) async {
    // Flatten rects into [x0,y0,w0,h0, x1,y1,w1,h1, ...]
    final flat = <double>[];
    for (final r in regions) {
      flat.addAll([r.x, r.y, r.width, r.height]);
    }
    await _sendWithSource(Op.eraseRegions, source, sink: output,
      args: {'page': page, 'rects': flat},
    );
  }

  @override
  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75,
      bool garbageCollect = true,
      bool linearize = false}) async {
    await _sendWithSource(Op.compress, source, sink: output,
      args: {
        'imageQuality': imageQuality,
        'garbageCollect': garbageCollect,
        'linearize': linearize,
      },
    );
  }

  // ── Extraction ──

  @override
  Future<String> extractText(PdfSource source,
      {int? page, String? password}) async {
    final result = await _sendWithSource(Op.extractText, source,
      args: {'page': page, 'password': password},
    );
    return result as String;
  }

  @override
  Future<String> toMarkdown(PdfSource source,
      {int? page, String? password}) async {
    final result = await _sendWithSource(Op.toMarkdown, source,
      args: {'page': page, 'password': password},
    );
    return result as String;
  }

  @override
  Future<String> toHtml(PdfSource source,
      {required int page, String? password}) async {
    final result = await _sendWithSource(Op.toHtml, source,
      args: {'page': page, 'password': password},
    );
    return result as String;
  }

  @override
  Future<String> toPlainText(PdfSource source,
      {required int page, String? password}) async {
    final result = await _sendWithSource(Op.toPlainText, source,
      args: {'page': page, 'password': password},
    );
    return result as String;
  }

  // ── Search ──

  @override
  Future<List<SearchResult>> searchPage(PdfSource source,
      {required int page, required String query, String? password}) async {
    final result = await _sendWithSource(Op.searchPage, source,
      args: {'page': page, 'query': query, 'password': password},
    );
    return result as List<SearchResult>;
  }

  @override
  Future<List<SearchResult>> searchAll(PdfSource source,
      {required String query, String? password}) async {
    final result = await _sendWithSource(Op.searchAll, source,
      args: {'query': query, 'password': password},
    );
    return result as List<SearchResult>;
  }

  // ── Security ──

  @override
  Future<void> watermark(PdfSource source, PdfSink output,
      {required String text,
      List<int>? pages,
      double opacity = 0.3,
      double fontSize = 48,
      double rotation = 45,
      double r = 0.5,
      double g = 0.5,
      double b = 0.5}) async {
    await _sendWithSource(Op.watermark, source, sink: output,
      args: {
        'text': text, 'pages': pages,
        'opacity': opacity, 'fontSize': fontSize, 'rotation': rotation,
        'r': r, 'g': g, 'b': b,
      },
    );
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
    await _sendWithSource(Op.watermarkPositioned, source, sink: output,
      args: {
        'text': text, 'fontName': fontName, 'pages': pages,
        'x': x, 'y': y, 'width': width, 'height': height,
        'fontSize': fontSize, 'rotation': rotation, 'opacity': opacity,
        'r': r, 'g': g, 'b': b,
        'fixedPrint': fixedPrint, 'fixedPrintH': fixedPrintH, 'fixedPrintV': fixedPrintV,
      },
    );
  }

  @override
  Future<void> encrypt(PdfSource source, PdfSink output,
      {required String ownerPassword, String userPassword = ''}) async {
    await _sendWithSource(Op.encrypt, source, sink: output,
      args: {'ownerPassword': ownerPassword, 'userPassword': userPassword},
    );
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
    await _sendWithSource(Op.encryptFull, source, sink: output,
      args: {
        'ownerPassword': ownerPassword, 'userPassword': userPassword,
        'algorithm': algorithm,
        'allowPrint': allowPrint, 'allowPrintHq': allowPrintHq,
        'allowModify': allowModify, 'allowCopy': allowCopy,
        'allowAnnotate': allowAnnotate, 'allowFillForms': allowFillForms,
        'allowAccessibility': allowAccessibility, 'allowAssemble': allowAssemble,
      },
    );
  }

  @override
  Future<void> decrypt(PdfSource source, PdfSink output,
      {required String password}) async {
    await _sendWithSource(Op.decrypt, source, sink: output,
      args: {'password': password},
    );
  }

  @override
  Future<void> sign(PdfSource source, PdfSink output,
      {required Uint8List certificate,
      required String certificatePassword,
      String? reason,
      String? location}) async {
    final srcServer = SourceServer(source);
    final srcPort = srcServer.start();
    final snkServer = SinkServer(output);
    final snkPort = snkServer.start();
    try {
      await _send(WorkerMsg(
        id: _id(),
        op: Op.sign,
        sourcePort: srcPort,
        sinkPort: snkPort,
        bytesList: [transfer(certificate)],
        args: {
          'certificatePassword': certificatePassword,
          'reason': reason, 'location': location,
        },
      ));
    } finally {
      srcServer.stop();
      snkServer.stop();
    }
  }

  // ── Creation ──

  @override
  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output) async {
    if (images.isEmpty) throw ArgumentError('images must not be empty');
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _send(WorkerMsg(
        id: _id(),
        op: Op.imagesToPdf,
        sinkPort: sinkPort,
        bytesList: transferList(images),
      ));
    } finally {
      server.stop();
    }
  }

  // ── Rendering ──

  @override
  Future<RenderedPage> renderPage(PdfSource source, int pageIndex,
      {String? password}) async {
    final result = await _sendWithSource(Op.renderPage, source,
      args: {'pageIndex': pageIndex, 'password': password},
    );
    return result as RenderedPage;
  }

  @override
  Future<RenderedPage> renderPageFit(PdfSource source, int pageIndex,
      {required int width, required int height, String? password}) async {
    final result = await _sendWithSource(Op.renderPageFit, source,
      args: {
        'pageIndex': pageIndex, 'width': width, 'height': height,
        'password': password,
      },
    );
    return result as RenderedPage;
  }

  @override
  Future<RenderedPage> renderPageThumbnail(PdfSource source, int pageIndex,
      {required int size, String? password}) async {
    final result = await _sendWithSource(Op.renderPageThumbnail, source,
      args: {'pageIndex': pageIndex, 'size': size, 'password': password},
    );
    return result as RenderedPage;
  }

  @override
  Stream<RenderedPage> renderAllPages(PdfSource source,
      {required int width, required int height, String? password}) =>
    _sendStreamWithSource<RenderedPage>(Op.renderAllPages, source,
      args: {'width': width, 'height': height, 'password': password});

  // ── Image extraction ──

  @override
  Stream<PdfImage> extractImages(PdfSource source, int pageIndex,
      {String? password}) =>
    _sendStreamWithSource<PdfImage>(Op.extractImages, source,
      args: {'pageIndex': pageIndex, 'password': password});

  @override
  Stream<PdfImage> extractAllImages(PdfSource source,
      {String? password}) =>
    _sendStreamWithSource<PdfImage>(Op.extractAllImages, source,
      args: {'password': password});

  // ── Signatures ──

  @override
  Future<int> getSignatureCount(PdfSource source, {String? password}) async {
    final result = await _sendWithSource(Op.getSignatureCount, source,
      args: {'password': password},
    );
    return result as int;
  }

  @override
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
      {String? password}) async {
    final result = await _sendWithSource(Op.getSignatures, source,
      args: {'password': password},
    );
    return result as List<PdfSignatureInfo>;
  }

  @override
  Future<bool> verifySignatures(PdfSource source, {String? password}) async {
    final result = await _sendWithSource(Op.verifySignatures, source,
      args: {'password': password},
    );
    return result as bool;
  }

  // ── Validation ──

  @override
  Future<({bool compliant, int errors, int warnings})> validatePdfA(
      PdfSource source,
      {int level = 2,
      String? password}) async {
    final result = await _sendWithSource(Op.validatePdfA, source,
      args: {'level': level, 'password': password},
    );
    return result as ({bool compliant, int errors, int warnings});
  }

  @override
  Future<bool> validatePdfUa(PdfSource source,
      {int level = 1, String? password}) async {
    final result = await _sendWithSource(Op.validatePdfUa, source,
      args: {'level': level, 'password': password},
    );
    return result as bool;
  }

  // ── Encryption info ──

  @override
  Future<({bool print, bool printHq, bool modify, bool copy, bool annotate,
      bool fillForms, bool accessibility, bool assemble})>
    getPermissions(PdfSource source, {String? password}) async {
    final result = await _sendWithSource(Op.getPermissions, source,
      args: {'password': password},
    );
    return result as ({bool print, bool printHq, bool modify, bool copy,
        bool annotate, bool fillForms, bool accessibility, bool assemble});
  }

  @override
  Future<int> getEncryptionAlgorithm(PdfSource source, {String? password}) async {
    final result = await _sendWithSource(Op.getEncryptionAlgorithm, source,
      args: {'password': password},
    );
    return result as int;
  }

  // ── Editor ──

  @override
  Future<PdfEditorHandle> openEditor(PdfSource source) async {
    final handleId = _nextHandleId++;
    final server = SourceServer(source);
    final serverPort = server.start();
    _sourceServers[handleId] = server;
    await _send(WorkerMsg(
      id: _id(),
      op: Op.editorOpen,
      sourcePort: serverPort,
      args: {'handleId': handleId},
    ));
    return _NativeEditorHandle(this, handleId);
  }

  // ── Builder ──

  @override
  Future<PdfBuilderHandle> createBuilder() async {
    final handleId = _nextHandleId++;
    await _send(WorkerMsg(
      id: _id(),
      op: Op.builderCreate,
      args: {'handleId': handleId},
    ));
    return _NativeBuilderHandle(this, handleId);
  }

  @override
  void configureWorkerUrl(String url) {}

  // ── Lifecycle ──

  @override
  Future<void> dispose() async {
    if (_workerPort != null) {
      await _send(WorkerMsg(id: _id(), op: Op.dispose));
      _responsePort?.close();
      _workerIsolate?.kill();
      _workerPort = null;
      _workerIsolate = null;
      _responsePort = null;
      for (final c in _pending.values) {
        c.completeError(StateError('Worker disposed'));
      }
      _pending.clear();
    }
  }

  // ════════════════════════════════════════════════════════════════════════
  // Worker isolate entry — handles all Op codes via PdfBindings
  // ════════════════════════════════════════════════════════════════════════

  static void _workerEntry(SendPort mainPort) {
    final workerPort = ReceivePort();
    mainPort.send(workerPort.sendPort);

    SendPort? responsePort;
    final bindings = const PdfBindings();
    final handles = <int, ffi.Pointer<ffi.Void>>{};

    workerPort.listen((message) async {
      if (message is SendPort) {
        responsePort = message;
        return;
      }

      if (message is! WorkerMsg) return;
      final msg = message;

      try {
        final result = await _dispatch(bindings, handles, msg, responsePort);
        if (!identical(result, _streamSentinel)) {
          responsePort?.send(WorkerResult(id: msg.id, value: result));
        }
      } catch (e) {
        responsePort?.send(WorkerResult(id: msg.id, error: e));
      }
    });
  }

  static final _streamSentinel = Object();

  /// Save an editor handle — streams via callback writer if sinkPort is
  /// available, otherwise returns bytes as TransferableTypedData.
  static Future<Object?> _saveEditor(PdfBindings b, ffi.Pointer<ffi.Void> handle, WorkerMsg msg) async {
    final bytes = b.editorSave(handle);
    await writeBytesToSink(msg.sinkPort!, bytes);
    return null;
  }

  static ffi.Pointer<ffi.Void> _h(Map<int, ffi.Pointer<ffi.Void>> handles, int? id) {
    final h = handles[id];
    if (h == null) throw StateError('Handle $id has been disposed');
    return h;
  }

  /// Open an editor from a WorkerMsg. Reads bytes from the streaming
  /// source, uses standard editorOpen (sets source_bytes for convertToPdfA).
  static Future<(ffi.Pointer<ffi.Void>, void Function())> _openEditorFromMsg(
      PdfBindings b, WorkerMsg msg) async {
    final bytes = await _resolveBytes(msg);
    return (b.editorOpen(bytes), () {});
  }

  /// Resolve the primary PDF bytes from either a streaming source or a
  /// transferred buffer. For ops that need the full bytes (doc-based reads,
  /// non-editor operations), this reads from the SourceServer on main.
  static Future<Uint8List> _resolveBytes(WorkerMsg msg) async {
    final replyPort = ReceivePort();
    msg.sourcePort!.send(['length', replyPort.sendPort]);
    final length = await replyPort.first as int;
    final dataPort = ReceivePort();
    msg.sourcePort!.send(['read', 0, length, dataPort.sendPort]);
    final td = await dataPort.first as TransferableTypedData;
    return td.materialize().asUint8List();
  }

  static Future<Object?> _dispatch(
    PdfBindings b,
    Map<int, ffi.Pointer<ffi.Void>> handles,
    WorkerMsg msg,
    SendPort? responsePort,
  ) async {
    switch (msg.op) {
      // ── Inspect ──

      case Op.open:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final count = b.docPageCount(handle);
          final version = b.docVersion(handle);
          final isTagged = b.docHasStructureTree(handle);
          final edHandle = b.editorOpen(bytes);
          final pages = <PdfPageInfo>[];
          for (var i = 0; i < count; i++) {
            final box = b.editorGetPageMediaBox(edHandle, i);
            pages.add(PdfPageInfo(
              index: i,
              width: box.width,
              height: box.height,
              rotation: b.editorGetPageRotation(edHandle, i),
            ));
          }
          String? title, author, subject, keywords;
          try {
            final t = b.editorGetTitle(edHandle);
            final a = b.editorGetAuthor(edHandle);
            final s = b.editorGetSubject(edHandle);
            final k = b.editorGetKeywords(edHandle);
            title = t.isEmpty ? null : t;
            author = a.isEmpty ? null : a;
            subject = s.isEmpty ? null : s;
            keywords = k.isEmpty ? null : k;
          } finally {
            b.editorFree(edHandle);
          }
          return PdfDoc(
            pageCount: count,
            version: version,
            pages: pages,
            title: title,
            author: author,
            subject: subject,
            keywords: keywords,
            isTagged: isTagged,
          );
        } finally {
          b.docFree(handle);
        }

      case Op.probe:
        final bytes = await _resolveBytes(msg);
        try {
          final handle = b.docOpenFromBytes(bytes);
          try {
            final count = b.docPageCount(handle);
            final version = b.docVersion(handle);
            final isTagged = b.docHasStructureTree(handle);
            return PdfInfo(
              isValid: true,
              pageCount: count,
              version: version,
              isTagged: isTagged,
            );
          } finally {
            b.docFree(handle);
          }
        } catch (_) {
          return const PdfInfo(isValid: false);
        }

      // ── Structural ──

      case Op.merge:
        final remaining = materializeList(msg.bytesList!);
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          for (final other in remaining) {
            b.editorMerge(handle, other);
          }
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.split:
        final every = msg.args['every'] as int;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final pageCount = b.editorPageCount(handle);
          final results = <TransferableTypedData>[];
          for (var start = 0; start < pageCount; start += every) {
            final end = (start + every).clamp(0, pageCount);
            final pages = List.generate(end - start, (i) => start + i);
            results.add(transfer(b.editorExtractPages(handle, pages)));
          }
          return results;
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.splitBySize:
        final maxBytes = msg.args['maxBytes'] as int;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final pageCount = b.editorPageCount(handle);
          final results = <TransferableTypedData>[];
          var chunkPages = <int>[];
          for (var i = 0; i < pageCount; i++) {
            chunkPages.add(i);
            final trial = b.editorExtractPages(handle, chunkPages);
            if (trial.length > maxBytes && chunkPages.length > 1) {
              chunkPages.removeLast();
              results
                  .add(transfer(b.editorExtractPages(handle, chunkPages)));
              chunkPages = [i];
            }
          }
          if (chunkPages.isNotEmpty) {
            results
                .add(transfer(b.editorExtractPages(handle, chunkPages)));
          }
          return results;
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.extractPages:
        final pages = (msg.args['pages'] as List).cast<int>();
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final extracted = b.editorExtractPages(handle, pages);
          await writeBytesToSink(msg.sinkPort!, extracted);
          return null;
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.deletePages:
        final pages = (msg.args['pages'] as List).cast<int>();
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final sorted = List<int>.from(pages)
            ..sort((a, c) => c.compareTo(a));
          for (final page in sorted) {
            b.editorDeletePage(handle, page);
          }
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.reorderPages:
        final order = (msg.args['order'] as List).cast<int>();
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final reordered = b.editorExtractPages(handle, order);
          await writeBytesToSink(msg.sinkPort!, reordered);
          return null;
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.movePage:
        final from = msg.args['from'] as int;
        final to = msg.args['to'] as int;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          b.editorMovePage(handle, from, to);
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.rotatePages:
        final pages = (msg.args['pages'] as Map).cast<int, int>();
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          for (final entry in pages.entries) {
            b.editorRotatePage(handle, entry.key, entry.value);
          }
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.rotateAllPages:
        final degrees = msg.args['degrees'] as int;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          b.editorRotateAllPages(handle, degrees);
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      // ── Content ──

      case Op.flattenForms:
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          b.editorFlattenForms(handle);
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.applyRedactions:
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          b.editorApplyAllRedactions(handle);
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.embedFile:
        final fileData = materialize(msg.bytesList![0]);
        final name = msg.args['name'] as String;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          b.editorEmbedFile(handle, name, fileData);
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.eraseRegions:
        final page = msg.args['page'] as int;
        final flat = (msg.args['rects'] as List).cast<double>();
        final rects = <PdfRect>[];
        for (var i = 0; i < flat.length; i += 4) {
          rects.add(PdfRect(
              x: flat[i], y: flat[i + 1], width: flat[i + 2], height: flat[i + 3]));
        }
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          b.editorEraseRegions(handle, page, rects);
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.compress:
        final imageQuality = msg.args['imageQuality'] as int;
        final garbageCollect = msg.args['garbageCollect'] as bool;
        final linearize = msg.args['linearize'] as bool;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          b.editorOptimizeImages(handle, quality: imageQuality);
          final bytes = b.editorSaveWithOptions(handle,
              compress: true, garbageCollect: garbageCollect,
              linearize: linearize);
          await writeBytesToSink(msg.sinkPort!, bytes);
          return null;
        } finally {
          b.editorFree(handle);
          dispose();
        }

      // ── Extraction ──

      case Op.extractText:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final page = msg.args['page'] as int?;
          if (page != null) return b.docExtractText(handle, page);
          final count = b.docPageCount(handle);
          final buf = StringBuffer();
          for (var i = 0; i < count; i++) {
            if (i > 0) buf.writeln();
            buf.write(b.docExtractText(handle, i));
          }
          return buf.toString();
        } finally {
          b.docFree(handle);
        }

      case Op.toMarkdown:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final page = msg.args['page'] as int?;
          if (page != null) return b.docToMarkdown(handle, page);
          return b.docToMarkdownAll(handle);
        } finally {
          b.docFree(handle);
        }

      case Op.toHtml:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final page = msg.args['page'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docToHtml(handle, page);
        } finally {
          b.docFree(handle);
        }

      case Op.toPlainText:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final page = msg.args['page'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docToPlainText(handle, page);
        } finally {
          b.docFree(handle);
        }

      // ── Search ──

      case Op.searchPage:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final page = msg.args['page'] as int;
        final query = msg.args['query'] as String;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docSearchPage(handle, page, query);
        } finally {
          b.docFree(handle);
        }

      case Op.searchAll:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final query = msg.args['query'] as String;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docSearchAll(handle, query);
        } finally {
          b.docFree(handle);
        }

      // ── Security ──

      case Op.watermark:
        final text = msg.args['text'] as String;
        final pages = (msg.args['pages'] as List?)?.cast<int>();
        final opacity = msg.args['opacity'] as double;
        final fontSize = msg.args['fontSize'] as double;
        final rotation = msg.args['rotation'] as double;
        final r = msg.args['r'] as double;
        final g = msg.args['g'] as double;
        final bVal = msg.args['b'] as double;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final targets = pages ??
              List.generate(b.editorPageCount(handle), (i) => i);
          for (final i in targets) {
            b.editorAddWatermark(handle, i, text,
                fontSize: fontSize,
                rotation: rotation,
                opacity: opacity,
                r: r,
                g: g,
                b: bVal);
          }
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.watermarkPositioned:
        final text = msg.args['text'] as String;
        final fontName = msg.args['fontName'] as String?;
        final pages = (msg.args['pages'] as List?)?.cast<int>();
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final width = msg.args['width'] as double;
        final height = msg.args['height'] as double;
        final fontSize = msg.args['fontSize'] as double;
        final rotation = msg.args['rotation'] as double;
        final opacity = msg.args['opacity'] as double;
        final r = msg.args['r'] as double;
        final g = msg.args['g'] as double;
        final bVal = msg.args['b'] as double;
        final fixedPrint = msg.args['fixedPrint'] as bool? ?? false;
        final fixedPrintH = msg.args['fixedPrintH'] as double? ?? 0.0;
        final fixedPrintV = msg.args['fixedPrintV'] as double? ?? 0.0;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final targets = pages ??
              List.generate(b.editorPageCount(handle), (i) => i);
          for (final i in targets) {
            b.editorAddWatermarkPositioned(handle, i, text,
                x: x,
                y: y,
                width: width,
                height: height,
                fontSize: fontSize,
                fontName: fontName,
                rotation: rotation,
                opacity: opacity,
                r: r,
                g: g,
                b: bVal,
                fixedPrint: fixedPrint,
                fixedPrintH: fixedPrintH,
                fixedPrintV: fixedPrintV);
          }
          return _saveEditor(b, handle, msg);
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.encrypt:
        final ownerPassword = msg.args['ownerPassword'] as String;
        final userPassword = msg.args['userPassword'] as String;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final bytes = b.editorSaveEncrypted(handle,
              userPassword: userPassword, ownerPassword: ownerPassword);
          await writeBytesToSink(msg.sinkPort!, bytes);
          return null;
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.encryptFull:
        final ownerPassword = msg.args['ownerPassword'] as String;
        final userPassword = msg.args['userPassword'] as String;
        final algorithm = msg.args['algorithm'] as int;
        final allowPrint = msg.args['allowPrint'] as bool;
        final allowPrintHq = msg.args['allowPrintHq'] as bool;
        final allowModify = msg.args['allowModify'] as bool;
        final allowCopy = msg.args['allowCopy'] as bool;
        final allowAnnotate = msg.args['allowAnnotate'] as bool;
        final allowFillForms = msg.args['allowFillForms'] as bool;
        final allowAccessibility = msg.args['allowAccessibility'] as bool;
        final allowAssemble = msg.args['allowAssemble'] as bool;
        final (handle, dispose) = await _openEditorFromMsg(b, msg);
        try {
          final bytes = b.editorSaveEncryptedFull(handle,
              userPassword: userPassword, ownerPassword: ownerPassword,
              algorithm: algorithm,
              allowPrint: allowPrint, allowPrintHq: allowPrintHq,
              allowModify: allowModify, allowCopy: allowCopy,
              allowAnnotate: allowAnnotate, allowFillForms: allowFillForms,
              allowAccessibility: allowAccessibility, allowAssemble: allowAssemble);
          await writeBytesToSink(msg.sinkPort!, bytes);
          return null;
        } finally {
          b.editorFree(handle);
          dispose();
        }

      case Op.decrypt:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final edHandle = b.editorOpen(bytes);
          try {
            return _saveEditor(b, edHandle, msg);
          } finally {
            b.editorFree(edHandle);
          }
        } finally {
          b.docFree(handle);
        }

      case Op.sign:
        // PDF signing is inherently non-streamable — the signature blob
        // is patched into an offset computed from the full assembled output.
        // ByteRange calculation requires knowing total size upfront.
        final bytes = await _resolveBytes(msg);
        final cert = materialize(msg.bytesList![0]);
        final certificatePassword = msg.args['certificatePassword'] as String;
        final reason = msg.args['reason'] as String?;
        final location = msg.args['location'] as String?;
        final signed = b.signBytes(bytes, cert, certificatePassword,
            reason: reason, location: location);
        // Push the signed result to the sink via the streaming protocol
        await writeBytesToSink(msg.sinkPort!, signed);
        return null;

      // ── Creation ──

      case Op.imagesToPdf:
        final images = materializeList(msg.bytesList!);
        final builder = b.builderCreate();
        try {
          for (final imageBytes in images) {
            final page = b.builderAddA4Page(builder);
            try {
              b.pageBuilderImage(page, imageBytes, 0, 0, 595, 842);
            } finally {
              b.pageBuilderDone(page);
            }
          }
          final built = b.builderBuild(builder);
          await writeBytesToSink(msg.sinkPort!, built);
          return null;
        } finally {
          b.builderFree(builder);
        }

      // ── Rendering ──

      case Op.renderPage:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final pageIndex = msg.args['pageIndex'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.renderPage(handle, pageIndex);
        } finally {
          b.docFree(handle);
        }

      case Op.renderPageFit:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final pageIndex = msg.args['pageIndex'] as int;
        final width = msg.args['width'] as int;
        final height = msg.args['height'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.renderPageFit(handle, pageIndex,
              fitWidth: width, fitHeight: height);
        } finally {
          b.docFree(handle);
        }

      case Op.renderPageThumbnail:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final pageIndex = msg.args['pageIndex'] as int;
        final size = msg.args['size'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.renderPageThumbnail(handle, pageIndex,
              thumbnailSize: size);
        } finally {
          b.docFree(handle);
        }

      case Op.renderAllPages:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final width = msg.args['width'] as int;
        final height = msg.args['height'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final count = b.docPageCount(handle);
          for (var i = 0; i < count; i++) {
            final page = b.renderPageFit(handle, i,
                fitWidth: width, fitHeight: height);
            responsePort?.send(WorkerStreamItem(id: msg.id, value: page));
          }
          responsePort?.send(WorkerStreamItem(id: msg.id, done: true));
          return _streamSentinel;
        } finally {
          b.docFree(handle);
        }

      // ── Image extraction ──

      case Op.extractImages:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final pageIndex = msg.args['pageIndex'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final images = b.docGetEmbeddedImages(handle, pageIndex);
          for (final img in images) {
            responsePort?.send(WorkerStreamItem(id: msg.id, value: img));
          }
          responsePort?.send(WorkerStreamItem(id: msg.id, done: true));
          return _streamSentinel;
        } finally {
          b.docFree(handle);
        }

      case Op.extractAllImages:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final count = b.docPageCount(handle);
          for (var i = 0; i < count; i++) {
            final images = b.docGetEmbeddedImages(handle, i);
            for (final img in images) {
              responsePort?.send(WorkerStreamItem(id: msg.id, value: img));
            }
          }
          responsePort?.send(WorkerStreamItem(id: msg.id, done: true));
          return _streamSentinel;
        } finally {
          b.docFree(handle);
        }

      // ── Signatures ──

      case Op.getSignatureCount:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docGetSignatureCount(handle);
        } finally {
          b.docFree(handle);
        }

      case Op.getSignatures:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          final count = b.docGetSignatureCount(handle);
          final sigs = <PdfSignatureInfo>[];
          for (var i = 0; i < count; i++) {
            sigs.add(b.docGetSignature(handle, i));
          }
          return sigs;
        } finally {
          b.docFree(handle);
        }

      case Op.verifySignatures:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docVerifyAllSignatures(handle);
        } finally {
          b.docFree(handle);
        }

      // ── Validation ──

      case Op.validatePdfA:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final level = msg.args['level'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docValidatePdfA(handle, level: level);
        } finally {
          b.docFree(handle);
        }

      case Op.validatePdfUa:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final level = msg.args['level'] as int;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docValidatePdfUa(handle, level: level);
        } finally {
          b.docFree(handle);
        }

      // ── Editor handle ops ──

      case Op.editorOpen:
        final handleId = msg.args['handleId'] as int;
        final bytes = await _resolveBytes(msg);
        handles[handleId] = b.editorOpen(bytes);
        return null;

      case Op.editorDispose:
        final handleId = msg.args['handleId'] as int?;
        final h = handles.remove(handleId);
        if (h != null) b.editorFree(h);
        return null;

      case Op.editorPageCount:
        final handleId = msg.args['handleId'] as int;
        return b.editorPageCount(_h(handles, handleId));

      case Op.editorVersion:
        final handleId = msg.args['handleId'] as int;
        return b.editorVersion(_h(handles, handleId));

      case Op.editorIsModified:
        final handleId = msg.args['handleId'] as int;
        return b.editorIsModified(_h(handles, handleId));

      case Op.editorGetTitle:
        final handleId = msg.args['handleId'] as int;
        return b.editorGetTitle(_h(handles, handleId));

      case Op.editorSetTitle:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.editorSetTitle(_h(handles, handleId), value);
        return null;

      case Op.editorGetAuthor:
        final handleId = msg.args['handleId'] as int;
        return b.editorGetAuthor(_h(handles, handleId));

      case Op.editorSetAuthor:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.editorSetAuthor(_h(handles, handleId), value);
        return null;

      case Op.editorGetSubject:
        final handleId = msg.args['handleId'] as int;
        return b.editorGetSubject(_h(handles, handleId));

      case Op.editorSetSubject:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.editorSetSubject(_h(handles, handleId), value);
        return null;

      case Op.editorGetKeywords:
        final handleId = msg.args['handleId'] as int;
        return b.editorGetKeywords(_h(handles, handleId));

      case Op.editorSetKeywords:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.editorSetKeywords(_h(handles, handleId), value);
        return null;

      case Op.editorRotatePage:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        final degrees = msg.args['degrees'] as int;
        b.editorRotatePage(_h(handles, handleId), page, degrees);
        return null;

      case Op.editorRotateAllPages:
        final handleId = msg.args['handleId'] as int;
        final degrees = msg.args['degrees'] as int;
        b.editorRotateAllPages(_h(handles, handleId), degrees);
        return null;

      case Op.editorGetPageMediaBox:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        return b.editorGetPageMediaBox(_h(handles, handleId), page);

      case Op.editorDeletePage:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        b.editorDeletePage(_h(handles, handleId), page);
        return null;

      case Op.editorMovePage:
        final handleId = msg.args['handleId'] as int;
        final from = msg.args['from'] as int;
        final to = msg.args['to'] as int;
        b.editorMovePage(_h(handles, handleId), from, to);
        return null;

      case Op.editorExtractPages:
        final handleId = msg.args['handleId'] as int;
        final pages = (msg.args['pages'] as List).cast<int>();
        final extracted = b.editorExtractPages(_h(handles, handleId), pages);
        await writeBytesToSink(msg.sinkPort!, extracted);
        return null;

      case Op.editorMergeFrom:
        final handleId = msg.args['handleId'] as int;
        final replyPort = ReceivePort();
        msg.sourcePort!.send(['length', replyPort.sendPort]);
        final length = await replyPort.first as int;
        final dataPort = ReceivePort();
        msg.sourcePort!.send(['read', 0, length, dataPort.sendPort]);
        final td = await dataPort.first as TransferableTypedData;
        final otherPdf = td.materialize().asUint8List();
        b.editorMerge(_h(handles, handleId), otherPdf);
        return null;

      case Op.editorOptimizeImages:
        final handleId = msg.args['handleId'] as int;
        final quality = msg.args['quality'] as int;
        return b.editorOptimizeImages(_h(handles, handleId),
            quality: quality);

      case Op.editorUnembedStandardFonts:
        final handleId = msg.args['handleId'] as int;
        return b.editorUnembedStandardFonts(_h(handles, handleId));

      case Op.editorAddWatermark:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        final text = msg.args['text'] as String;
        final opacity = msg.args['opacity'] as double;
        final fontSize = msg.args['fontSize'] as double;
        final rotation = msg.args['rotation'] as double;
        final r = msg.args['r'] as double;
        final g = msg.args['g'] as double;
        final bVal = msg.args['b'] as double;
        b.editorAddWatermark(
          _h(handles, handleId),
          page,
          text,
          fontSize: fontSize,
          rotation: rotation,
          opacity: opacity,
          r: r,
          g: g,
          b: bVal,
        );
        return null;

      case Op.editorEmbedFile:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final fileData = materialize(msg.bytes!);
        b.editorEmbedFile(_h(handles, handleId), name, fileData);
        return null;

      case Op.editorEraseRegions:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        final flat = (msg.args['rects'] as List).cast<double>();
        final rects = <PdfRect>[];
        for (var i = 0; i < flat.length; i += 4) {
          rects.add(PdfRect(
              x: flat[i],
              y: flat[i + 1],
              width: flat[i + 2],
              height: flat[i + 3]));
        }
        b.editorEraseRegions(_h(handles, handleId), page, rects);
        return null;

      case Op.editorFlattenForms:
        final handleId = msg.args['handleId'] as int;
        b.editorFlattenForms(_h(handles, handleId));
        return null;

      case Op.editorFlattenAllAnnotations:
        final handleId = msg.args['handleId'] as int;
        b.editorFlattenAllAnnotations(_h(handles, handleId));
        return null;

      case Op.editorApplyAllRedactions:
        final handleId = msg.args['handleId'] as int;
        b.editorApplyAllRedactions(_h(handles, handleId));
        return null;

      case Op.editorSetFormFieldValue:
        final handleId = msg.args['handleId'] as int;
        final field = msg.args['field'] as String;
        final value = msg.args['value'] as String;
        b.editorSetFormFieldValue(
            _h(handles, handleId), field, value);
        return null;

      case Op.editorCropMargins:
        final handleId = msg.args['handleId'] as int;
        final left = msg.args['left'] as double;
        final right = msg.args['right'] as double;
        final top = msg.args['top'] as double;
        final bottom = msg.args['bottom'] as double;
        b.editorCropMargins(_h(handles, handleId),
            left: left,
            right: right,
            top: top,
            bottom: bottom);
        return null;

      case Op.editorConvertToPdfA:
        final handleId = msg.args['handleId'] as int;
        final level = msg.args['level'] as int;
        b.editorConvertToPdfA(_h(handles, handleId), level);
        return null;

      case Op.editorSave:
        final handleId = msg.args['handleId'] as int;
        final saveBytes = b.editorSave(_h(handles, handleId));
        await writeBytesToSink(msg.sinkPort!, saveBytes);
        return null;

      case Op.editorSaveWithOptions:
        final handleId = msg.args['handleId'] as int;
        final compress = msg.args['compress'] as bool;
        final garbageCollect = msg.args['garbageCollect'] as bool;
        final linearize = msg.args['linearize'] as bool;
        final optsBytes = b.editorSaveWithOptions(_h(handles, handleId),
            compress: compress, garbageCollect: garbageCollect,
            linearize: linearize);
        await writeBytesToSink(msg.sinkPort!, optsBytes);
        return null;

      case Op.editorSaveEncrypted:
        final handleId = msg.args['handleId'] as int;
        final ownerPassword = msg.args['ownerPassword'] as String;
        final userPassword = msg.args['userPassword'] as String;
        final encBytes = b.editorSaveEncrypted(_h(handles, handleId),
            userPassword: userPassword, ownerPassword: ownerPassword);
        await writeBytesToSink(msg.sinkPort!, encBytes);
        return null;

      case Op.editorSaveEncryptedFull:
        final handleId = msg.args['handleId'] as int;
        final ownerPassword = msg.args['ownerPassword'] as String;
        final userPassword = msg.args['userPassword'] as String;
        final algorithm = msg.args['algorithm'] as int;
        final allowPrint = msg.args['allowPrint'] as bool;
        final allowPrintHq = msg.args['allowPrintHq'] as bool;
        final allowModify = msg.args['allowModify'] as bool;
        final allowCopy = msg.args['allowCopy'] as bool;
        final allowAnnotate = msg.args['allowAnnotate'] as bool;
        final allowFillForms = msg.args['allowFillForms'] as bool;
        final allowAccessibility = msg.args['allowAccessibility'] as bool;
        final allowAssemble = msg.args['allowAssemble'] as bool;
        final encFullBytes = b.editorSaveEncryptedFull(_h(handles, handleId),
            userPassword: userPassword, ownerPassword: ownerPassword,
            algorithm: algorithm,
            allowPrint: allowPrint, allowPrintHq: allowPrintHq,
            allowModify: allowModify, allowCopy: allowCopy,
            allowAnnotate: allowAnnotate, allowFillForms: allowFillForms,
            allowAccessibility: allowAccessibility, allowAssemble: allowAssemble);
        await writeBytesToSink(msg.sinkPort!, encFullBytes);
        return null;

      case Op.editorAddWatermarkPositioned:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        final text = msg.args['text'] as String;
        final fontName = msg.args['fontName'] as String?;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final width = msg.args['width'] as double;
        final height = msg.args['height'] as double;
        final fontSize = msg.args['fontSize'] as double;
        final rotation = msg.args['rotation'] as double;
        final opacity = msg.args['opacity'] as double;
        final r = msg.args['r'] as double;
        final g = msg.args['g'] as double;
        final bVal = msg.args['b'] as double;
        b.editorAddWatermarkPositioned(
          _h(handles, handleId),
          page,
          text,
          x: x,
          y: y,
          width: width,
          height: height,
          fontSize: fontSize,
          fontName: fontName,
          rotation: rotation,
          opacity: opacity,
          r: r,
          g: g,
          b: bVal,
        );
        return null;

      case Op.editorAddStamp:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        final stampType = msg.args['stampType'] as int;
        final customName = msg.args['customName'] as String?;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final width = msg.args['width'] as double;
        final height = msg.args['height'] as double;
        final opacity = msg.args['opacity'] as double;
        b.editorAddStamp(
          _h(handles, handleId),
          page,
          stampType: stampType,
          customName: customName,
          x: x,
          y: y,
          width: width,
          height: height,
          opacity: opacity,
        );
        return null;

      case Op.editorAddImageStamp:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        final imageBytes = msg.args['imageBytes'] as Uint8List;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final width = msg.args['width'] as double;
        final height = msg.args['height'] as double;
        final opacity = msg.args['opacity'] as double;
        b.editorAddImageStamp(
          _h(handles, handleId),
          page,
          imageBytes,
          x: x, y: y, width: width, height: height,
          opacity: opacity,
        );
        return null;

      case Op.editorResizeImage:
        final handleId = msg.args['handleId'] as int;
        final page = msg.args['page'] as int;
        final imageName = msg.args['imageName'] as String;
        final width = msg.args['width'] as double;
        final height = msg.args['height'] as double;
        b.editorResizeImage(
          _h(handles, handleId),
          page,
          imageName,
          width,
          height,
        );
        return null;

      // ── Builder handle ops ──

      case Op.builderCreate:
        final handleId = msg.args['handleId'] as int;
        handles[handleId] = b.builderCreate();
        return null;

      case Op.builderDispose:
        final handleId = msg.args['handleId'] as int?;
        final h = handles.remove(handleId);
        if (h != null) b.builderFree(h);
        return null;

      case Op.builderSetTitle:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.builderSetTitle(_h(handles, handleId), value);
        return null;

      case Op.builderSetAuthor:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.builderSetAuthor(_h(handles, handleId), value);
        return null;

      case Op.builderSetSubject:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.builderSetSubject(_h(handles, handleId), value);
        return null;

      case Op.builderSetKeywords:
        final handleId = msg.args['handleId'] as int;
        final value = msg.args['value'] as String;
        b.builderSetKeywords(_h(handles, handleId), value);
        return null;

      case Op.builderAddA4Page:
        final handleId = msg.args['handleId'] as int;
        final pageHandleId = msg.args['pageHandleId'] as int;
        final parentH = _h(handles, handleId);
        final pageH = b.builderAddA4Page(parentH);
        handles[pageHandleId] = pageH;
        return null;

      case Op.builderAddLetterPage:
        final handleId = msg.args['handleId'] as int;
        final pageHandleId = msg.args['pageHandleId'] as int;
        final parentH = _h(handles, handleId);
        final pageH = b.builderAddLetterPage(parentH);
        handles[pageHandleId] = pageH;
        return null;

      case Op.builderAddPage:
        final handleId = msg.args['handleId'] as int;
        final pageHandleId = msg.args['pageHandleId'] as int;
        final width = msg.args['width'] as double;
        final height = msg.args['height'] as double;
        final parentH = _h(handles, handleId);
        final pageH =
            b.builderAddPage(parentH, width, height);
        handles[pageHandleId] = pageH;
        return null;

      case Op.builderBuild:
        final handleId = msg.args['handleId'] as int;
        final built = b.builderBuild(_h(handles, handleId));
        await writeBytesToSink(msg.sinkPort!, built);
        return null;

      case Op.builderBuildEncrypted:
        final handleId = msg.args['handleId'] as int;
        final ownerPassword = msg.args['ownerPassword'] as String;
        final userPassword = msg.args['userPassword'] as String;
        final builtEnc = b.builderBuildEncrypted(_h(handles, handleId),
            userPassword: userPassword, ownerPassword: ownerPassword);
        await writeBytesToSink(msg.sinkPort!, builtEnc);
        return null;

      // ── Page builder ops ──

      case Op.pageFont:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final size = msg.args['size'] as double;
        b.pageBuilderFont(_h(handles, handleId), name, size);
        return null;

      case Op.pageAt:
        final handleId = msg.args['handleId'] as int;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        b.pageBuilderAt(_h(handles, handleId), x, y);
        return null;

      case Op.pageText:
        final handleId = msg.args['handleId'] as int;
        final text = msg.args['text'] as String;
        b.pageBuilderText(_h(handles, handleId), text);
        return null;

      case Op.pageHeading:
        final handleId = msg.args['handleId'] as int;
        final level = msg.args['level'] as int;
        final text = msg.args['text'] as String;
        b.pageBuilderHeading(_h(handles, handleId), level, text);
        return null;

      case Op.pageParagraph:
        final handleId = msg.args['handleId'] as int;
        final text = msg.args['text'] as String;
        b.pageBuilderParagraph(_h(handles, handleId), text);
        return null;

      case Op.pageSpace:
        final handleId = msg.args['handleId'] as int;
        final points = msg.args['points'] as double;
        b.pageBuilderSpace(_h(handles, handleId), points);
        return null;

      case Op.pageHorizontalRule:
        final handleId = msg.args['handleId'] as int;
        b.pageBuilderHorizontalRule(_h(handles, handleId));
        return null;

      case Op.pageImage:
        final handleId = msg.args['handleId'] as int;
        final imageBytes = materialize(msg.bytes!);
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final width = msg.args['width'] as double;
        final height = msg.args['height'] as double;
        final altText = msg.args['altText'] as String? ?? '';
        b.pageBuilderImage(_h(handles, handleId), imageBytes, x,
            y, width, height,
            altText: altText);
        return null;

      case Op.pageWatermark:
        final handleId = msg.args['handleId'] as int;
        final text = msg.args['text'] as String;
        b.pageBuilderWatermark(_h(handles, handleId), text);
        return null;

      case Op.pageDone:
        final handleId = msg.args['handleId'] as int;
        final h = handles.remove(handleId);
        if (h != null) b.pageBuilderDone(h);
        return null;

      // ── Page builder form field ops ──

      case Op.pageTextField:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final defaultValue = msg.args['defaultValue'] as String?;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final w = msg.args['w'] as double;
        final h = msg.args['h'] as double;
        b.pageBuilderTextField(_h(handles, handleId), name,
            x, y, w, h,
            defaultValue: defaultValue);
        return null;

      case Op.pageCheckbox:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final checked = msg.args['checked'] as bool? ?? false;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final w = msg.args['w'] as double;
        final h = msg.args['h'] as double;
        b.pageBuilderCheckbox(_h(handles, handleId), name,
            x, y, w, h,
            checked: checked);
        return null;

      case Op.pageComboBox:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final selected = msg.args['selected'] as String?;
        final options = (msg.args['options'] as List?)?.cast<String>() ?? const <String>[];
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final w = msg.args['w'] as double;
        final h = msg.args['h'] as double;
        b.pageBuilderComboBox(_h(handles, handleId), name,
            x, y, w, h,
            options,
            selected: selected);
        return null;

      case Op.pagePushButton:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final caption = msg.args['caption'] as String;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final w = msg.args['w'] as double;
        final h = msg.args['h'] as double;
        b.pageBuilderPushButton(_h(handles, handleId), name,
            x, y, w, h,
            caption);
        return null;

      case Op.pageSignatureField:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final x = msg.args['x'] as double;
        final y = msg.args['y'] as double;
        final w = msg.args['w'] as double;
        final h = msg.args['h'] as double;
        b.pageBuilderSignatureField(_h(handles, handleId), name,
            x, y, w, h);
        return null;

      case Op.pageRadioGroup:
        final handleId = msg.args['handleId'] as int;
        final name = msg.args['name'] as String;
        final selected = msg.args['selected'] as String?;
        final values = (msg.args['values'] as List).cast<String>();
        // doubleListArg packed as [x0,y0,w0,h0, x1,y1,w1,h1, ...]
        final flat = (msg.args['rects'] as List).cast<double>();
        final count = flat.length ~/ 4;
        final rxs = <double>[], rys = <double>[], rws = <double>[], rhs = <double>[];
        for (var i = 0; i < count; i++) {
          rxs.add(flat[i * 4]);
          rys.add(flat[i * 4 + 1]);
          rws.add(flat[i * 4 + 2]);
          rhs.add(flat[i * 4 + 3]);
        }
        b.pageBuilderRadioGroup(_h(handles, handleId), name,
            values, rxs, rys, rws, rhs, selected: selected);
        return null;

      case Op.pageFieldKeystroke:
        final handleId = msg.args['handleId'] as int;
        final script = msg.args['script'] as String;
        b.pageBuilderFieldKeystroke(_h(handles, handleId), script);
        return null;

      case Op.pageFieldFormat:
        final handleId = msg.args['handleId'] as int;
        final script = msg.args['script'] as String;
        b.pageBuilderFieldFormat(_h(handles, handleId), script);
        return null;

      case Op.pageFieldValidate:
        final handleId = msg.args['handleId'] as int;
        final script = msg.args['script'] as String;
        b.pageBuilderFieldValidate(_h(handles, handleId), script);
        return null;

      case Op.pageFieldCalculate:
        final handleId = msg.args['handleId'] as int;
        final script = msg.args['script'] as String;
        b.pageBuilderFieldCalculate(_h(handles, handleId), script);
        return null;

      case Op.pageLinkUrl:
        final handleId = msg.args['handleId'] as int;
        final url = msg.args['url'] as String;
        b.pageBuilderLinkUrl(_h(handles, handleId), url);
        return null;

      case Op.pageLinkPage:
        final handleId = msg.args['handleId'] as int;
        final targetPage = msg.args['targetPage'] as int;
        b.pageBuilderLinkPage(_h(handles, handleId), targetPage);
        return null;

      case Op.pageFootnote:
        final handleId = msg.args['handleId'] as int;
        final refMark = msg.args['refMark'] as String;
        final noteText = msg.args['noteText'] as String;
        b.pageBuilderFootnote(_h(handles, handleId), refMark, noteText);
        return null;

      case Op.pageColumns:
        final handleId = msg.args['handleId'] as int;
        final columnCount = msg.args['columnCount'] as int;
        final gapPt = msg.args['gapPt'] as double;
        final text = msg.args['text'] as String;
        b.pageBuilderColumns(_h(handles, handleId), columnCount, gapPt, text);
        return null;

      case Op.pageNewline:
        final handleId = msg.args['handleId'] as int;
        b.pageBuilderNewline(_h(handles, handleId));
        return null;

      case Op.pageNewPageSameSize:
        final handleId = msg.args['handleId'] as int;
        b.pageBuilderNewPageSameSize(_h(handles, handleId));
        return null;

      // ── Encryption info ──

      case Op.getPermissions:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docGetPermissions(handle);
        } finally {
          b.docFree(handle);
        }

      case Op.getEncryptionAlgorithm:
        final bytes = await _resolveBytes(msg);
        final password = msg.args['password'] as String?;
        final handle = b.docOpenFromBytes(bytes, password: password);
        try {
          return b.docGetEncryptionAlgorithm(handle);
        } finally {
          b.docFree(handle);
        }

      // ── Lifecycle ──

      case Op.dispose:
        for (final h in handles.values) {
          b.editorFree(h);
        }
        handles.clear();
        return null;
    }
  }
}

// ════════════════════════════════════════════════════════════════════════════
// _NativeEditorHandle — thin wrapper that sends WorkerMsg for each op
// ════════════════════════════════════════════════════════════════════════════

class _NativeEditorHandle implements PdfEditorHandle {
  final NativePdfPlatform _platform;
  final int _id;
  _NativeEditorHandle(this._platform, this._id);

  Future<Object?> _msg(WorkerMsg m) => _platform._send(m);
  int _newId() => _platform._id();

  @override
  Future<int> get pageCount async =>
      await _msg(WorkerMsg(id: _newId(), op: Op.editorPageCount, args: {'handleId': _id}))
          as int;

  @override
  Future<String> get version async =>
      await _msg(WorkerMsg(id: _newId(), op: Op.editorVersion, args: {'handleId': _id}))
          as String;

  @override
  Future<bool> get isModified async =>
      await _msg(
              WorkerMsg(id: _newId(), op: Op.editorIsModified, args: {'handleId': _id}))
          as bool;

  @override
  Future<String> getTitle() async =>
      await _msg(WorkerMsg(id: _newId(), op: Op.editorGetTitle, args: {'handleId': _id}))
          as String;

  @override
  Future<void> setTitle(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.editorSetTitle, args: {'handleId': _id, 'value': v}));

  @override
  Future<String> getAuthor() async =>
      await _msg(
              WorkerMsg(id: _newId(), op: Op.editorGetAuthor, args: {'handleId': _id}))
          as String;

  @override
  Future<void> setAuthor(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.editorSetAuthor, args: {'handleId': _id, 'value': v}));

  @override
  Future<String> getSubject() async =>
      await _msg(
              WorkerMsg(id: _newId(), op: Op.editorGetSubject, args: {'handleId': _id}))
          as String;

  @override
  Future<void> setSubject(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.editorSetSubject, args: {'handleId': _id, 'value': v}));

  @override
  Future<String> getKeywords() async =>
      await _msg(
              WorkerMsg(id: _newId(), op: Op.editorGetKeywords, args: {'handleId': _id}))
          as String;

  @override
  Future<void> setKeywords(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.editorSetKeywords, args: {'handleId': _id, 'value': v}));

  @override
  Future<void> rotatePage(int i, {required int degrees}) => _msg(WorkerMsg(
      id: _newId(),
      op: Op.editorRotatePage,
      args: {'handleId': _id, 'page': i, 'degrees': degrees}));

  @override
  Future<void> rotateAllPages({required int degrees}) => _msg(WorkerMsg(
      id: _newId(),
      op: Op.editorRotateAllPages,
      args: {'handleId': _id, 'degrees': degrees}));

  @override
  Future<PdfRect> getPageMediaBox(int i) async =>
      await _msg(WorkerMsg(
              id: _newId(),
              op: Op.editorGetPageMediaBox,
              args: {'handleId': _id, 'page': i}))
          as PdfRect;

  @override
  Future<void> deletePage(int i) => _msg(WorkerMsg(
      id: _newId(), op: Op.editorDeletePage, args: {'handleId': _id, 'page': i}));

  @override
  Future<void> movePage({required int from, required int to}) => _msg(
      WorkerMsg(
          id: _newId(),
          op: Op.editorMovePage,
          args: {'handleId': _id, 'from': from, 'to': to}));

  @override
  Future<void> extractPages(List<int> pages, PdfSink output) async {
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _msg(WorkerMsg(
          id: _newId(),
          op: Op.editorExtractPages,
          sinkPort: sinkPort,
          args: {'handleId': _id, 'pages': pages}));
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> mergeFrom(PdfSource otherPdf) async {
    final server = SourceServer(otherPdf);
    final serverPort = server.start();
    try {
      await _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorMergeFrom,
        args: {'handleId': _id},
        sourcePort: serverPort));
    } finally {
      server.stop();
    }
  }

  @override
  Future<int> optimizeImages({int quality = 75}) async =>
      await _msg(WorkerMsg(
              id: _newId(),
              op: Op.editorOptimizeImages,
              args: {'handleId': _id, 'quality': quality}))
          as int;

  @override
  Future<int> unembedStandardFonts() async =>
      await _msg(WorkerMsg(
              id: _newId(),
              op: Op.editorUnembedStandardFonts,
              args: {'handleId': _id}))
          as int;

  @override
  Future<void> addWatermark(int i, String text,
          {double fontSize = 48,
          double rotation = 45,
          double opacity = 0.3,
          double r = 0.5,
          double g = 0.5,
          double b = 0.5}) =>
      _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorAddWatermark,
        args: {
          'handleId': _id, 'page': i, 'text': text,
          'opacity': opacity, 'fontSize': fontSize, 'rotation': rotation,
          'r': r, 'g': g, 'b': b,
        },
      ));

  @override
  Future<void> embedFile(String name, Uint8List data) => _msg(WorkerMsg(
      id: _newId(),
      op: Op.editorEmbedFile,
      args: {'handleId': _id, 'name': name},
      bytes: transfer(data)));

  @override
  Future<void> eraseRegions(int i, List<PdfRect> regions) {
    final flat = <double>[];
    for (final r in regions) {
      flat.addAll([r.x, r.y, r.width, r.height]);
    }
    return _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorEraseRegions,
        args: {'handleId': _id, 'page': i, 'rects': flat}));
  }

  @override
  Future<void> flattenForms() => _msg(
      WorkerMsg(id: _newId(), op: Op.editorFlattenForms, args: {'handleId': _id}));

  @override
  Future<void> flattenAllAnnotations() => _msg(WorkerMsg(
      id: _newId(), op: Op.editorFlattenAllAnnotations, args: {'handleId': _id}));

  @override
  Future<void> applyAllRedactions() => _msg(WorkerMsg(
      id: _newId(), op: Op.editorApplyAllRedactions, args: {'handleId': _id}));

  @override
  Future<void> setFormFieldValue(String field, String value) => _msg(
      WorkerMsg(
          id: _newId(),
          op: Op.editorSetFormFieldValue,
          args: {'handleId': _id, 'field': field, 'value': value}));

  @override
  Future<void> cropMargins(
          {double left = 0,
          double right = 0,
          double top = 0,
          double bottom = 0}) =>
      _msg(WorkerMsg(
          id: _newId(),
          op: Op.editorCropMargins,
          args: {
            'handleId': _id,
            'left': left, 'right': right, 'top': top, 'bottom': bottom,
          }));

  @override
  Future<void> convertToPdfA({int level = 1}) => _msg(WorkerMsg(
      id: _newId(),
      op: Op.editorConvertToPdfA,
      args: {'handleId': _id, 'level': level}));

  @override
  Future<void> save(PdfSink output) async {
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _msg(WorkerMsg(
          id: _newId(), op: Op.editorSave,
          sinkPort: sinkPort, args: {'handleId': _id}));
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> saveWithOptions(PdfSink output,
      {bool compress = true,
      bool garbageCollect = true,
      bool linearize = false}) async {
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _msg(WorkerMsg(
          id: _newId(), op: Op.editorSaveWithOptions,
          sinkPort: sinkPort,
          args: {
            'handleId': _id,
            'compress': compress, 'garbageCollect': garbageCollect, 'linearize': linearize,
          }));
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> saveEncrypted(PdfSink output,
      {required String ownerPassword, String userPassword = ''}) async {
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _msg(WorkerMsg(
          id: _newId(),
          op: Op.editorSaveEncrypted,
          sinkPort: sinkPort,
          args: {'handleId': _id, 'ownerPassword': ownerPassword, 'userPassword': userPassword}));
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> saveEncryptedFull(PdfSink output, {
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
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorSaveEncryptedFull,
        sinkPort: sinkPort,
        args: {
          'handleId': _id,
          'ownerPassword': ownerPassword, 'userPassword': userPassword,
          'algorithm': algorithm,
          'allowPrint': allowPrint, 'allowPrintHq': allowPrintHq,
          'allowModify': allowModify, 'allowCopy': allowCopy,
          'allowAnnotate': allowAnnotate, 'allowFillForms': allowFillForms,
          'allowAccessibility': allowAccessibility, 'allowAssemble': allowAssemble,
        },
      ));
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> addWatermarkPositioned(int i, String text, {
    required double x, required double y,
    required double width, required double height,
    double fontSize = 48, String? fontName,
    double rotation = 45, double opacity = 0.3,
    double r = 0.5, double g = 0.5, double b = 0.5,
  }) =>
      _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorAddWatermarkPositioned,
        args: {
          'handleId': _id, 'page': i, 'text': text, 'fontName': fontName,
          'x': x, 'y': y, 'width': width, 'height': height,
          'fontSize': fontSize, 'rotation': rotation, 'opacity': opacity,
          'r': r, 'g': g, 'b': b,
        },
      ));

  @override
  Future<void> addStamp(int i, {
    required int stampType,
    String? customName,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) =>
      _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorAddStamp,
        args: {
          'handleId': _id, 'page': i, 'stampType': stampType, 'customName': customName,
          'x': x, 'y': y, 'width': width, 'height': height, 'opacity': opacity,
        },
      ));

  @override
  Future<void> addImageStamp(int i, Uint8List imageBytes, {
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) =>
      _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorAddImageStamp,
        args: {
          'handleId': _id, 'page': i, 'imageBytes': imageBytes,
          'x': x, 'y': y, 'width': width, 'height': height, 'opacity': opacity,
        },
      ));

  @override
  Future<void> resizeImage(int i, String imageName,
      {required double width, required double height}) =>
      _msg(WorkerMsg(
        id: _newId(),
        op: Op.editorResizeImage,
        args: {
          'handleId': _id, 'page': i, 'imageName': imageName,
          'width': width, 'height': height,
        },
      ));

  @override
  Future<void> dispose() async {
    await _msg(
        WorkerMsg(id: _newId(), op: Op.editorDispose, args: {'handleId': _id}));
    _platform._sourceServers.remove(_id)?.stop();
  }
}

// ════════════════════════════════════════════════════════════════════════════
// _NativeBuilderHandle — thin wrapper that sends WorkerMsg for each op
// ════════════════════════════════════════════════════════════════════════════

class _NativeBuilderHandle implements PdfBuilderHandle {
  final NativePdfPlatform _platform;
  final int _id;
  _NativeBuilderHandle(this._platform, this._id);

  Future<Object?> _msg(WorkerMsg m) => _platform._send(m);
  int _newId() => _platform._id();

  @override
  Future<void> setTitle(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.builderSetTitle, args: {'handleId': _id, 'value': v}));

  @override
  Future<void> setAuthor(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.builderSetAuthor, args: {'handleId': _id, 'value': v}));

  @override
  Future<void> setSubject(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.builderSetSubject, args: {'handleId': _id, 'value': v}));

  @override
  Future<void> setKeywords(String v) => _msg(WorkerMsg(
      id: _newId(), op: Op.builderSetKeywords, args: {'handleId': _id, 'value': v}));

  @override
  Future<PdfPageBuilderHandle> addA4Page() async {
    final pageHandleId = NativePdfPlatform._nextHandleId++;
    await _msg(WorkerMsg(
        id: _newId(),
        op: Op.builderAddA4Page,
        args: {'handleId': _id, 'pageHandleId': pageHandleId}));
    return _NativePageBuilderHandle(_platform, pageHandleId);
  }

  @override
  Future<PdfPageBuilderHandle> addLetterPage() async {
    final pageHandleId = NativePdfPlatform._nextHandleId++;
    await _msg(WorkerMsg(
        id: _newId(),
        op: Op.builderAddLetterPage,
        args: {'handleId': _id, 'pageHandleId': pageHandleId}));
    return _NativePageBuilderHandle(_platform, pageHandleId);
  }

  @override
  Future<PdfPageBuilderHandle> addPage(
      {required double width, required double height}) async {
    final pageHandleId = NativePdfPlatform._nextHandleId++;
    await _msg(WorkerMsg(
        id: _newId(),
        op: Op.builderAddPage,
        args: {
          'handleId': _id, 'pageHandleId': pageHandleId,
          'width': width, 'height': height,
        }));
    return _NativePageBuilderHandle(_platform, pageHandleId);
  }

  @override
  Future<void> build(PdfSink output) async {
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _msg(WorkerMsg(
          id: _newId(), op: Op.builderBuild,
          sinkPort: sinkPort, args: {'handleId': _id}));
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> buildEncrypted(PdfSink output,
      {required String ownerPassword, String userPassword = ''}) async {
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      await _msg(WorkerMsg(
          id: _newId(), op: Op.builderBuildEncrypted,
          sinkPort: sinkPort,
          args: {'handleId': _id, 'ownerPassword': ownerPassword, 'userPassword': userPassword}));
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> dispose() => _msg(
      WorkerMsg(id: _newId(), op: Op.builderDispose, args: {'handleId': _id}));
}

// ════════════════════════════════════════════════════════════════════════════
// _NativePageBuilderHandle — thin wrapper that sends WorkerMsg for each op
// ════════════════════════════════════════════════════════════════════════════

class _NativePageBuilderHandle implements PdfPageBuilderHandle {
  final NativePdfPlatform _platform;
  final int _id;
  _NativePageBuilderHandle(this._platform, this._id);

  Future<Object?> _msg(WorkerMsg m) => _platform._send(m);
  int _newId() => _platform._id();

  @override
  Future<void> font(String name, double size) => _msg(WorkerMsg(
      id: _newId(),
      op: Op.pageFont,
      args: {'handleId': _id, 'name': name, 'size': size}));

  @override
  Future<void> at(double x, double y) => _msg(WorkerMsg(
      id: _newId(),
      op: Op.pageAt,
      args: {'handleId': _id, 'x': x, 'y': y}));

  @override
  Future<void> text(String text) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageText, args: {'handleId': _id, 'text': text}));

  @override
  Future<void> heading(int level, String text) => _msg(WorkerMsg(
      id: _newId(),
      op: Op.pageHeading,
      args: {'handleId': _id, 'level': level, 'text': text}));

  @override
  Future<void> paragraph(String text) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageParagraph, args: {'handleId': _id, 'text': text}));

  @override
  Future<void> space(double points) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageSpace, args: {'handleId': _id, 'points': points}));

  @override
  Future<void> horizontalRule() => _msg(
      WorkerMsg(id: _newId(), op: Op.pageHorizontalRule, args: {'handleId': _id}));

  @override
  Future<void> image(Uint8List imageBytes, double x, double y, double width,
          double height, {String altText = ''}) =>
      _msg(WorkerMsg(
        id: _newId(),
        op: Op.pageImage,
        args: {
          'handleId': _id,
          'x': x, 'y': y, 'width': width, 'height': height,
          'altText': altText,
        },
        bytes: transfer(imageBytes),
      ));

  @override
  Future<void> watermark(String text) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageWatermark, args: {'handleId': _id, 'text': text}));

  // ── Form fields ──

  @override
  Future<void> textField(String name, double x, double y, double w, double h,
      {String? defaultValue}) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageTextField,
      args: {
        'handleId': _id, 'name': name, 'defaultValue': defaultValue,
        'x': x, 'y': y, 'w': w, 'h': h,
      }));

  @override
  Future<void> checkbox(String name, double x, double y, double w, double h,
      {bool checked = false}) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageCheckbox,
      args: {
        'handleId': _id, 'name': name, 'checked': checked,
        'x': x, 'y': y, 'w': w, 'h': h,
      }));

  @override
  Future<void> comboBox(String name, double x, double y, double w, double h,
      List<String> options, {String? selected}) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageComboBox,
      args: {
        'handleId': _id, 'name': name, 'selected': selected, 'options': options,
        'x': x, 'y': y, 'w': w, 'h': h,
      }));

  @override
  Future<void> pushButton(String name, double x, double y, double w, double h,
      String caption) => _msg(WorkerMsg(
      id: _newId(), op: Op.pagePushButton,
      args: {
        'handleId': _id, 'name': name, 'caption': caption,
        'x': x, 'y': y, 'w': w, 'h': h,
      }));

  @override
  Future<void> signatureField(String name, double x, double y, double w,
      double h) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageSignatureField,
      args: {
        'handleId': _id, 'name': name,
        'x': x, 'y': y, 'w': w, 'h': h,
      }));

  @override
  Future<void> radioGroup(String name, List<String> values,
      List<double> xs, List<double> ys, List<double> ws, List<double> hs,
      {String? selected}) {
    // Pack parallel arrays into flat [x0,y0,w0,h0, x1,y1,w1,h1, ...]
    final flat = <double>[];
    for (var i = 0; i < xs.length; i++) {
      flat.addAll([xs[i], ys[i], ws[i], hs[i]]);
    }
    return _msg(WorkerMsg(
        id: _newId(), op: Op.pageRadioGroup,
        args: {
          'handleId': _id, 'name': name, 'selected': selected,
          'values': values, 'rects': flat,
        }));
  }

  @override
  Future<void> fieldKeystroke(String script) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageFieldKeystroke, args: {'handleId': _id, 'script': script}));

  @override
  Future<void> fieldFormat(String script) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageFieldFormat, args: {'handleId': _id, 'script': script}));

  @override
  Future<void> fieldValidate(String script) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageFieldValidate, args: {'handleId': _id, 'script': script}));

  @override
  Future<void> fieldCalculate(String script) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageFieldCalculate, args: {'handleId': _id, 'script': script}));

  @override
  Future<void> linkUrl(String url) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageLinkUrl, args: {'handleId': _id, 'url': url}));

  @override
  Future<void> linkPage(int targetPage) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageLinkPage, args: {'handleId': _id, 'targetPage': targetPage}));

  @override
  Future<void> footnote(String refMark, String noteText) => _msg(WorkerMsg(
      id: _newId(), op: Op.pageFootnote,
      args: {'handleId': _id, 'refMark': refMark, 'noteText': noteText}));

  @override
  Future<void> columns(int columnCount, double gapPt, String text) =>
      _msg(WorkerMsg(
          id: _newId(), op: Op.pageColumns,
          args: {'handleId': _id, 'columnCount': columnCount, 'gapPt': gapPt, 'text': text}));

  @override
  Future<void> newline() => _msg(
      WorkerMsg(id: _newId(), op: Op.pageNewline, args: {'handleId': _id}));

  @override
  Future<void> newPageSameSize() => _msg(
      WorkerMsg(id: _newId(), op: Op.pageNewPageSameSize, args: {'handleId': _id}));

  @override
  Future<void> done() =>
      _msg(WorkerMsg(id: _newId(), op: Op.pageDone, args: {'handleId': _id}));
}
