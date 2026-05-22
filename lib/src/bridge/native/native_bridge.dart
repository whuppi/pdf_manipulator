// NativeBridge — implements PdfBridge for native platforms.
//
// Main-isolate side only. Spawns a worker isolate that owns the Rust
// thread pool. Each operation: main creates SourceServer/SinkServer,
// sends a command to the worker, worker does FFI, result comes back.
//
// INTERNAL — created by bridge_factory.dart.

import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:pdf_manipulator/src/api/pdf_sink.dart';
import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/api/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/api/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/api/types/pdf_params.dart';
import 'package:pdf_manipulator/src/bridge/bridge.dart';
import 'package:pdf_manipulator/src/bridge/native/sink_server.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/bridge/native/source_server.dart';
import 'package:pdf_manipulator/src/bridge/ffi/bridge_bindings.dart' as bridge_ffi;
import 'package:pdf_manipulator/src/core/errors.dart';
import 'package:pdf_manipulator/src/bridge/native/worker_entry.dart';
import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';
import 'package:pdf_manipulator/src/page/pdf_page_info.dart';

class NativeBridge implements PdfBridge {
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
      workerEntryPoint,
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
        // One-shot response: [id, isError: bool, value]
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
        // Stream event: [id, 'item'/'done'/'error', data]
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
    return completer.future;
  }

  // ── Result decoding ────────────────────────────────────────────────

  static PdfDoc _decodePdfDoc(Uint8List bytes) {
    final data = ByteData.sublistView(bytes);
    var offset = 0;

    final status = bytes[offset]; offset += 1;
    if (status == 0) {
      final errorCode = data.getInt32(offset, Endian.little); offset += 4;
      final msgLen = data.getUint16(offset, Endian.little); offset += 2;
      final message = String.fromCharCodes(bytes, offset, offset + msgLen);
      throw StateError('PDF open failed (code $errorCode): $message');
    }

    final pageCount = data.getInt32(offset, Endian.little); offset += 4;
    final major = bytes[offset]; offset += 1;
    final minor = bytes[offset]; offset += 1;
    final isEncrypted = bytes[offset] != 0; offset += 1;
    final isTagged = bytes[offset] != 0; offset += 1;

    final pages = <PdfPageInfo>[];
    for (var i = 0; i < pageCount; i++) {
      final width = data.getFloat64(offset, Endian.little); offset += 8;
      final height = data.getFloat64(offset, Endian.little); offset += 8;
      final rotation = data.getInt32(offset, Endian.little); offset += 4;
      pages.add(PdfPageInfo(
        index: i,
        width: width,
        height: height,
        rotation: rotation,
      ));
    }

    String readStr() {
      final len = data.getUint16(offset, Endian.little); offset += 2;
      if (len == 0) return '';
      final s = String.fromCharCodes(bytes, offset, offset + len);
      offset += len;
      return s;
    }

    final title = readStr();
    final author = readStr();
    final subject = readStr();
    final keywords = readStr();

    return PdfDoc(
      pageCount: pageCount,
      version: '$major.$minor',
      pages: pages,
      title: title.isEmpty ? null : title,
      author: author.isEmpty ? null : author,
      subject: subject.isEmpty ? null : subject,
      keywords: keywords.isEmpty ? null : keywords,
      isTagged: isTagged,
      isEncrypted: isEncrypted,
    );
  }

  /// Encode a list of ints as [count: i32, values: [i32; N]] (little-endian).
  static Uint8List _encodeIntList(List<int> values) {
    final buf = Uint8List(4 + values.length * 4);
    final bd = ByteData.sublistView(buf);
    bd.setInt32(0, values.length, Endian.little);
    for (var i = 0; i < values.length; i++) {
      bd.setInt32(4 + i * 4, values[i], Endian.little);
    }
    return buf;
  }

  /// Helper: submit an edit operation (source → edit → sink).
  Future<void> _submitEdit(
    String op,
    PdfSource source,
    PdfSink sink, {
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
        throw StateError('Operation $op failed');
      }
    } finally {
      srcServer.stop();
      snkServer.stop();
    }
  }

  /// Helper: submit a read-only operation (source → read → result bytes).
  Future<Uint8List> _submitRead(
    int opCode,
    PdfSource source, {
    String? password,
    Uint8List? params,
  }) async {
    _checkDisposed();
    final server = SourceServer(source);
    final serverPort = server.start();
    try {
      final result = await _send('read', {
        'sourcePort': serverPort,
        'sourceLength': source.length,
        'password': password,
        'opCode': opCode,
        'params': params,
      });
      return result as Uint8List;
    } finally {
      server.stop();
    }
  }

  /// Decode a text result: [status: u8, text_len: u32, text: utf8 bytes]
  String _decodeTextResult(Uint8List bytes) {
    if (bytes.isEmpty || bytes[0] == 0) {
      throw StateError('Read operation failed');
    }
    final bd = ByteData.sublistView(bytes);
    final textLen = bd.getUint32(1, Endian.little);
    return utf8.decode(bytes.sublist(5, 5 + textLen));
  }

  /// Decode a validation result: [status: u8, compliant: u8, errors: i32, warnings: i32]
  PdfValidationResult _decodeValidationResult(Uint8List bytes) {
    if (bytes.isEmpty || bytes[0] == 0) {
      throw StateError('Validation failed');
    }
    final bd = ByteData.sublistView(bytes);
    return PdfValidationResult(
      compliant: bytes[1] == 1,
      errors: bd.getInt32(2, Endian.little),
      warnings: bd.getInt32(6, Endian.little),
    );
  }

  // ── PdfBridge implementation ───────────────────────────────────────

  @override
  Future<PdfDoc> open(PdfSource source, {String? password}) async {
    _checkDisposed();
    final server = SourceServer(source);
    final serverPort = server.start();
    try {
      final result = await _send('open', {
        'sourcePort': serverPort,
        'sourceLength': source.length,
        'password': password,
      });
      return _decodePdfDoc(result as Uint8List);
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> merge(List<PdfSource> inputs, PdfSink output) async {
    if (inputs.length < 2) throw ArgumentError('merge requires at least 2 PDFs');
    final secondaries = <Uint8List>[];
    for (var i = 1; i < inputs.length; i++) {
      secondaries.add(await inputs[i].readAt(0, inputs[i].length));
    }
    await _submitEdit('merge', inputs[0], output, secondaries: secondaries);
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
      // Try extracting current chunk — if it exceeds maxBytes, split before this page
      final trialSink = ByteCountSink();
      await extractPages(source, trialSink, pages: chunkPages);
      if (trialSink.length > maxBytes && chunkPages.length > 1) {
        // Too big with this page — emit the chunk WITHOUT this page
        chunkPages.removeLast();
        final sink = sinkFactory(chunkIndex);
        await extractPages(source, sink, pages: chunkPages);
        chunkIndex++;
        chunkPages = [i]; // start new chunk with the page that didn't fit
      }
    }
    // Emit remaining pages
    if (chunkPages.isNotEmpty) {
      final sink = sinkFactory(chunkIndex);
      await extractPages(source, sink, pages: chunkPages);
      chunkIndex++;
    }
    return chunkIndex;
  }

  @override
  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages}) async {
    final params = _encodeIntList(pages);
    await _submitEdit('extractPages', source, output, params: params);
  }

  @override
  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages}) async {
    final params = _encodeIntList(pages);
    await _submitEdit('deletePages', source, output, params: params);
  }

  @override
  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order}) async {
    final params = _encodeIntList(order);
    await _submitEdit('extractPages', source, output, params: params);
  }

  @override
  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to}) async {
    final params = Uint8List(8);
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, from, Endian.little);
    bd.setInt32(4, to, Endian.little);
    await _submitEdit('movePage', source, output, params: params);
  }

  @override
  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages}) async {
    final params = Uint8List(4 + pages.length * 8);
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, pages.length, Endian.little);
    var i = 0;
    for (final entry in pages.entries) {
      bd.setInt32(4 + i * 8, entry.key, Endian.little);
      bd.setInt32(8 + i * 8, entry.value, Endian.little);
      i++;
    }
    await _submitEdit('rotatePages', source, output, params: params);
  }

  @override
  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees}) async {
    final params = Uint8List(4);
    ByteData.sublistView(params).setInt32(0, degrees, Endian.little);
    await _submitEdit('rotateAllPages', source, output, params: params);
  }

  @override
  Future<void> flattenForms(PdfSource source, PdfSink output) async {
    await _submitEdit('flattenForms', source, output);
  }

  @override
  Future<void> applyRedactions(PdfSource source, PdfSink output) async {
    await _submitEdit('applyRedactions', source, output);
  }

  @override
  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData}) async {
    // embedFile: op code 11, name as params, file as secondary
    final nameBytes = utf8.encode(name);
    final params = Uint8List(4 + nameBytes.length);
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, nameBytes.length, Endian.little);
    params.setAll(4, nameBytes);
    await _submitEdit('embedFile', source, output,
        params: params, secondaries: [fileData]);
  }

  @override
  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions}) async {
    // eraseRegions: op code 12, params: [page: i32, count: i32, rects: [x,y,w,h as f64; N]]
    final params = Uint8List(8 + regions.length * 32);
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, page, Endian.little);
    bd.setInt32(4, regions.length, Endian.little);
    for (var i = 0; i < regions.length; i++) {
      final r = regions[i];
      bd.setFloat64(8 + i * 32, r.x, Endian.little);
      bd.setFloat64(16 + i * 32, r.y, Endian.little);
      bd.setFloat64(24 + i * 32, r.width, Endian.little);
      bd.setFloat64(32 + i * 32, r.height, Endian.little);
    }
    await _submitEdit('eraseRegions', source, output, params: params);
  }

  @override
  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75, bool garbageCollect = true,
       bool linearize = false}) async {
    await _submitEdit('compress', source, output,
        params: Uint8List.fromList([imageQuality.clamp(1, 100)]));
  }

  @override
  Future<String> extract(PdfSource source, {
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
    final result = await _submitRead(opCode, source, password: password, params: params);
    return _decodeTextResult(result);
  }

  @override
  Future<List<SearchResult>> search(PdfSource source, {
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
    final result = await _submitRead(3, source, params: params, password: password);
    // Decode search results
    final rbd = ByteData.sublistView(result);
    final count = rbd.getInt32(1, Endian.little);
    var off = 5;
    final hits = <SearchResult>[];
    for (var i = 0; i < count; i++) {
      final page = rbd.getInt32(off, Endian.little); off += 4;
      final x = rbd.getFloat32(off, Endian.little); off += 4;
      final y = rbd.getFloat32(off, Endian.little); off += 4;
      final w = rbd.getFloat32(off, Endian.little); off += 4;
      final h = rbd.getFloat32(off, Endian.little); off += 4;
      final textLen = rbd.getUint16(off, Endian.little); off += 2;
      final text = utf8.decode(result.sublist(off, off + textLen));
      off += textLen;
      hits.add(SearchResult(page: page, text: text, rect: PdfRect(x: x.toDouble(), y: y.toDouble(), width: w.toDouble(), height: h.toDouble())));
    }
    return hits;
  }

  @override
  Future<void> watermark(PdfSource source, PdfSink output, {
    required String text, PdfPages pages = const PdfPages.all(),
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  }) async {
    _checkDisposed();
    // Resolve page index: -1 for all, single for PdfPages.single
    final pageIndex = switch (pages) {
      PdfAllPages() => -1,
      PdfSinglePage(:final index) => index,
      PdfPageList(:final indices) => indices.first, // watermark first for now
      PdfPageRange(:final start) => start,
    };
    final textBytes = utf8.encode(text);
    final params = Uint8List(4 + 4 + textBytes.length + 8 * 6);
    final bd = ByteData.sublistView(params);
    var off = 0;
    bd.setInt32(off, pageIndex, Endian.little); off += 4;
    bd.setInt32(off, textBytes.length, Endian.little); off += 4;
    params.setAll(off, textBytes); off += textBytes.length;
    bd.setFloat64(off, style.fontSize, Endian.little); off += 8;
    bd.setFloat64(off, style.rotation, Endian.little); off += 8;
    bd.setFloat64(off, style.opacity, Endian.little); off += 8;
    bd.setFloat64(off, style.color.r, Endian.little); off += 8;
    bd.setFloat64(off, style.color.g, Endian.little); off += 8;
    bd.setFloat64(off, style.color.b, Endian.little);
    await _submitEdit('watermark', source, output, params: params);
  }

  @override
  Future<void> encrypt(PdfSource source, PdfSink output, {
    required PdfEncryptionConfig encryption,
  }) async {
    // encrypt: op code 13, params encode encryption config
    final ownerBytes = utf8.encode(encryption.ownerPassword);
    final userBytes = utf8.encode(encryption.userPassword);
    final params = Uint8List(4 + 4 + ownerBytes.length + 4 + userBytes.length + 1);
    final bd = ByteData.sublistView(params);
    var off = 0;
    bd.setInt32(off, encryption.algorithm.index, Endian.little); off += 4;
    bd.setInt32(off, ownerBytes.length, Endian.little); off += 4;
    params.setAll(off, ownerBytes); off += ownerBytes.length;
    bd.setInt32(off, userBytes.length, Endian.little); off += 4;
    params.setAll(off, userBytes); off += userBytes.length;
    // permissions as a bitmask
    var permBits = 0;
    if (encryption.permissions.print) permBits |= 1;
    if (encryption.permissions.printHq) permBits |= 2;
    if (encryption.permissions.modify) permBits |= 4;
    if (encryption.permissions.copy) permBits |= 8;
    if (encryption.permissions.annotate) permBits |= 16;
    if (encryption.permissions.fillForms) permBits |= 32;
    if (encryption.permissions.accessibility) permBits |= 64;
    if (encryption.permissions.assemble) permBits |= 128;
    params[off] = permBits;
    await _submitEdit('encrypt', source, output, params: params);
  }

  @override
  Future<void> decrypt(PdfSource source, PdfSink output, {
    required String password,
  }) async {
    // decrypt: open with password, save without encryption
    // This is just an open+save — the password authenticates, save is unencrypted
    final passwordBytes = utf8.encode(password);
    final params = Uint8List(4 + passwordBytes.length);
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, passwordBytes.length, Endian.little);
    params.setAll(4, passwordBytes);
    await _submitEdit('decrypt', source, output, params: params);
  }

  @override
  Future<void> sign(PdfSource source, PdfSink output, {
    required Uint8List certificate, required String certificatePassword,
    String? reason, String? location,
  }) async {
    _checkDisposed();
    final pwBytes = utf8.encode(certificatePassword);
    final reasonBytes = reason != null ? utf8.encode(reason) : Uint8List(0);
    final locBytes = location != null ? utf8.encode(location) : Uint8List(0);
    final params = Uint8List(4 + pwBytes.length + 4 + reasonBytes.length + 4 + locBytes.length);
    final bd = ByteData.sublistView(params);
    var off = 0;
    bd.setInt32(off, pwBytes.length, Endian.little); off += 4;
    params.setAll(off, pwBytes); off += pwBytes.length;
    bd.setInt32(off, reasonBytes.length, Endian.little); off += 4;
    params.setAll(off, reasonBytes); off += reasonBytes.length;
    bd.setInt32(off, locBytes.length, Endian.little); off += 4;
    params.setAll(off, locBytes);
    await _submitEdit('sign', source, output, params: params, secondaries: [certificate]);
  }

  @override
  Future<void> addStamp(PdfSource source, PdfSink output, {
    required int page, required PdfStampType type,
    required PdfRect rect, String? customName, double opacity = 1.0,
  }) async {
    _checkDisposed();
    final params = Uint8List(4 + 4 + 8 * 5); // page, stampType, x, y, w, h, opacity
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, page, Endian.little);
    bd.setInt32(4, type.index, Endian.little);
    bd.setFloat64(8, rect.x, Endian.little);
    bd.setFloat64(16, rect.y, Endian.little);
    bd.setFloat64(24, rect.width, Endian.little);
    bd.setFloat64(32, rect.height, Endian.little);
    bd.setFloat64(40, opacity, Endian.little);
    await _submitEdit('addStamp', source, output, params: params);
  }

  @override
  Future<void> addImageStamp(PdfSource source, PdfSink output, {
    required int page, required Uint8List imageBytes,
    required PdfRect rect, double opacity = 1.0,
  }) async {
    _checkDisposed();
    final params = Uint8List(4 + 8 * 5); // page, x, y, w, h, opacity
    final bd = ByteData.sublistView(params);
    bd.setInt32(0, page, Endian.little);
    bd.setFloat64(4, rect.x, Endian.little);
    bd.setFloat64(12, rect.y, Endian.little);
    bd.setFloat64(20, rect.width, Endian.little);
    bd.setFloat64(28, rect.height, Endian.little);
    bd.setFloat64(36, opacity, Endian.little);
    await _submitEdit('addImageStamp', source, output, params: params, secondaries: [imageBytes]);
  }

  @override
  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output) async {
    _checkDisposed();
    if (images.isEmpty) throw ArgumentError('images must not be empty');
    final server = SinkServer(output);
    final sinkPort = server.start();
    try {
      final result = await _send('imagesToPdf', {
        'images': images,
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

  /// Submit a streaming operation. Returns a Stream that yields raw item
  /// bytes one at a time. The caller decodes each item.
  Stream<Uint8List> _submitStream(
    int opCode,
    PdfSource source, {
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

    _workerPort!.send([id, 'stream', {
      'sourcePort': serverPort,
      'sourceLength': source.length,
      'password': password,
      'opCode': opCode,
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

  /// Encode PdfPages into binary params for the Rust dispatch.
  Uint8List _encodePages(PdfPages pages) {
    switch (pages) {
      case PdfAllPages():
        return Uint8List.fromList([0]); // type=0: all pages
      case PdfSinglePage(:final index):
        final buf = Uint8List(5);
        buf[0] = 1; // type=1: single
        ByteData.sublistView(buf).setInt32(1, index, Endian.little);
        return buf;
      case PdfPageList(:final indices):
        final buf = Uint8List(5 + indices.length * 4);
        buf[0] = 2; // type=2: list
        final bd = ByteData.sublistView(buf);
        bd.setInt32(1, indices.length, Endian.little);
        for (var i = 0; i < indices.length; i++) {
          bd.setInt32(5 + i * 4, indices[i], Endian.little);
        }
        return buf;
      case PdfPageRange(:final start, :final end):
        final buf = Uint8List(9);
        buf[0] = 3; // type=3: range
        final bd = ByteData.sublistView(buf);
        bd.setInt32(1, start, Endian.little);
        bd.setInt32(5, end, Endian.little);
        return buf;
    }
  }

  @override
  Stream<RenderedPage> render(PdfSource source, {
    required PdfPages pages, PdfRenderSize? size, String? password,
  }) async* {
    _checkDisposed();
    final pagesBytes = _encodePages(pages);
    final params = Uint8List(pagesBytes.length + 8);
    params.setAll(0, pagesBytes);
    final bd = ByteData.sublistView(params);
    bd.setInt32(pagesBytes.length, size?.maxWidth ?? 0, Endian.little);
    bd.setInt32(pagesBytes.length + 4, size?.maxHeight ?? 0, Endian.little);

    await for (final itemBytes in _submitStream(1, source, password: password, params: params)) {
      // Decode render item: [type=1, width: i32, height: i32, pixels...]
      final ibd = ByteData.sublistView(itemBytes);
      final w = ibd.getInt32(1, Endian.little);
      final h = ibd.getInt32(5, Endian.little);
      final pixels = Uint8List.sublistView(itemBytes, 9);
      yield RenderedPage(width: w, height: h, data: pixels);
    }
  }

  @override
  Stream<PdfImage> extractImages(PdfSource source, {
    required PdfPages pages, String? password,
  }) async* {
    _checkDisposed();
    final params = _encodePages(pages);

    await for (final itemBytes in _submitStream(2, source, password: password, params: params)) {
      // Decode image item: [type=1, w: i32, h: i32, fmt_len: u8, fmt, cs_len: u8, cs, bpc: i32, data_len: i32, data]
      final ibd = ByteData.sublistView(itemBytes);
      var offset = 1;
      final w = ibd.getInt32(offset, Endian.little); offset += 4;
      final h = ibd.getInt32(offset, Endian.little); offset += 4;

      final fmtLen = itemBytes[offset]; offset += 1;
      final format = String.fromCharCodes(itemBytes, offset, offset + fmtLen); offset += fmtLen;

      final csLen = itemBytes[offset]; offset += 1;
      final colorSpace = String.fromCharCodes(itemBytes, offset, offset + csLen); offset += csLen;

      final bpc = ibd.getInt32(offset, Endian.little); offset += 4;
      final dataLen = ibd.getInt32(offset, Endian.little); offset += 4;
      final data = Uint8List.sublistView(itemBytes, offset, offset + dataLen);

      yield PdfImage(
        width: w,
        height: h,
        format: format,
        colorSpace: colorSpace,
        bitsPerComponent: bpc,
        data: data,
      );
    }
  }

  @override
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source, {
    String? password,
  }) async {
    _checkDisposed();
    final result = await _submitRead(4, source, password: password);
    final rbd = ByteData.sublistView(result);
    final count = rbd.getInt32(1, Endian.little);
    var off = 5;
    final sigs = <PdfSignatureInfo>[];
    for (var i = 0; i < count; i++) {
      final nameLen = rbd.getUint16(off, Endian.little); off += 2;
      final name = utf8.decode(result.sublist(off, off + nameLen)); off += nameLen;
      final reasonLen = rbd.getUint16(off, Endian.little); off += 2;
      final reason = utf8.decode(result.sublist(off, off + reasonLen)); off += reasonLen;
      final locLen = rbd.getUint16(off, Endian.little); off += 2;
      final loc = utf8.decode(result.sublist(off, off + locLen)); off += locLen;
      sigs.add(PdfSignatureInfo(signerName: name.isEmpty ? null : name, reason: reason.isEmpty ? null : reason, location: loc.isEmpty ? null : loc, isValid: false));
    }
    return sigs;
  }

  @override
  Future<bool> verifySignatures(PdfSource source, {String? password}) async {
    _checkDisposed();
    final result = await _submitRead(5, source, password: password);
    return result[1] == 1;
  }

  @override
  Future<PdfValidationResult> validatePdfA(PdfSource source, {
    int level = 2, String? password,
  }) async {
    // validatePdfA: read op code 6, params: [level: i32]
    final params = Uint8List(4);
    ByteData.sublistView(params).setInt32(0, level, Endian.little);
    final result = await _submitRead(6, source, password: password, params: params);
    return _decodeValidationResult(result);
  }

  @override
  Future<bool> validatePdfUa(PdfSource source, {
    int level = 1, String? password,
  }) async {
    // validatePdfUa: read op code 7, params: [level: i32]
    final params = Uint8List(4);
    ByteData.sublistView(params).setInt32(0, level, Endian.little);
    final result = await _submitRead(7, source, password: password, params: params);
    return result.isNotEmpty && result[0] == 1 && result.length > 1 && result[1] == 1;
  }

  @override
  Future<BridgeEditorHandle> openEditor(PdfSource source, {String? password}) async {
    _checkDisposed();
    final server = SourceServer(source);
    final serverPort = server.start();
    try {
      final result = await _send('editorOpen', {
        'sourcePort': serverPort,
        'sourceLength': source.length,
        'password': password,
      });
      final bytes = result as Uint8List;
      if (bytes.isEmpty || bytes[0] != 1) {
        throw _decodeError(bytes);
      }
      final bd = ByteData.sublistView(bytes);
      final handleId = bd.getUint64(1, Endian.little);
      return _NativeEditorHandle(this, handleId);
    } finally {
      server.stop();
    }
  }

  @override
  Future<BridgeBuilderHandle> createBuilder() async {
    _checkDisposed();
    await _ensureWorker();
    final result = await _send('builderCreate', {});
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

  PdfError _decodeError(Uint8List bytes) {
    if (bytes.isEmpty) return const PdfEngineError('Unknown error');
    final bd = ByteData.sublistView(bytes);
    final code = bytes.length >= 5 ? bd.getInt32(1, Endian.little) : 0;
    String msg = 'Error code $code';
    if (bytes.length >= 7) {
      final msgLen = bd.getUint16(5, Endian.little);
      if (bytes.length >= 7 + msgLen) {
        msg = utf8.decode(bytes.sublist(7, 7 + msgLen));
      }
    }
    return switch (code) {
      1 => PdfInvalidArgument(msg),
      2 => PdfIoError(msg),
      3 => PdfCorrupted(msg),
      _ => PdfEngineError(msg),
    };
  }
}

/// Native editor handle — persistent Rust editor across multiple FFI calls.
class _NativeEditorHandle implements BridgeEditorHandle {
  _NativeEditorHandle(this._bridge, this._handleId);

  final NativeBridge _bridge;
  final int _handleId;

  Future<Uint8List> _mutate(int opCode, {Uint8List? params, List<Uint8List>? secondaries}) async {
    final result = await _bridge._send('editorMutate', {
      'handleId': _handleId,
      'opCode': opCode,
      'params': params,
      'secondaries': secondaries,
    });
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1) throw _bridge._decodeError(bytes);
    return bytes;
  }

  @override Future<int> get pageCount async => bridge_ffi.bridgeEditorPageCount(_handleId);

  Future<_Metadata> _getMetadata() async {
    final result = await _bridge._send('editorGetMetadata', {'handleId': _handleId});
    final bytes = result as Uint8List;
    if (bytes.isEmpty || bytes[0] != 1) throw _bridge._decodeError(bytes);
    final bd = ByteData.sublistView(bytes);
    final vMajor = bd.getUint8(1);
    final vMinor = bd.getUint8(2);
    var off = 7; // skip status(1) + version(2) + pageCount(4)
    String readStr() {
      final len = bd.getUint16(off, Endian.little);
      off += 2;
      final s = utf8.decode(bytes.sublist(off, off + len));
      off += len;
      return s;
    }
    return _Metadata(
      version: '$vMajor.$vMinor',
      title: readStr(),
      author: readStr(),
      subject: readStr(),
      keywords: readStr(),
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
    final out = calloc<ffi.Double>(4);
    try {
      final rc = bridge_ffi.bridgeEditorGetPageMediaBox(_handleId, page, out);
      if (rc != 0) return const PdfRect(x: 0, y: 0, width: 595, height: 842);
      return PdfRect(x: out[0], y: out[1], width: out[2], height: out[3]);
    } finally {
      calloc.free(out);
    }
  }
  @override Future<void> deletePage(int page) =>
      _mutate(3, params: _encodeDeletePages([page]));
  @override Future<void> movePage({required int from, required int to}) =>
      _mutate(10, params: _encodeFromTo(from, to));
  @override Future<void> extractPages(List<int> pages, PdfSink output) async {
    // Save the current editor state to a temp buffer, then use the
    // one-shot extractPages on those bytes.
    final tempBytes = BytesBuilder(copy: false);
    final tempSink = _CollectSink(tempBytes);
    await save(tempSink);
    final saved = tempBytes.takeBytes();
    final tempSource = _MemorySource(saved);
    await _bridge.extractPages(tempSource, output, pages: pages);
  }

  @override Future<void> mergeFrom(PdfSource otherPdf) async {
    // Read all bytes from the other source, pass as secondary to merge op
    final bytes = await otherPdf.readAt(0, otherPdf.length);
    await _mutate(1, secondaries: [bytes]);
  }

  @override Future<int> optimizeImages({int quality = 75}) async {
    await _mutate(9, params: Uint8List.fromList([quality]));
    return 0;
  }
  @override Future<int> unembedStandardFonts() async {
    await _mutate(23);
    return 0;
  }

  @override Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  }) => _mutate(15, params: _encodeWatermark(page, text, style));

  @override Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect,
    String? customName, double opacity = 1.0,
  }) => _mutate(17, params: _encodeStamp(page, type.index, rect, opacity));

  @override Future<void> addImageStamp(int page, Uint8List imageBytes, {
    required PdfRect rect, double opacity = 1.0,
  }) => _mutate(18, params: _encodeImageStamp(page, rect, opacity), secondaries: [imageBytes]);

  @override Future<void> embedFile(String name, Uint8List data) =>
      _mutate(11, params: _encodeString(name), secondaries: [data]);
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
  Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
    final server = SinkServer(output);
    final serverPort = server.start();
    try {
      final result = await _bridge._send('editorSave', {
        'handleId': _handleId,
        'sinkPort': serverPort,
        'compress': options.compress,
        'garbageCollect': options.garbageCollect,
        'linearize': options.linearize,
        'encryptAlgo': options.encryption != null
            ? options.encryption!.algorithm.index + 1 : 0,
        'encryptUserPw': options.encryption?.userPassword ?? '',
        'encryptOwnerPw': options.encryption?.ownerPassword ?? '',
        'encryptPermissions': options.encryption?.permissions.toBits() ?? -1,
      });
      final bytes = result as Uint8List;
      if (bytes.isEmpty || bytes[0] != 1) throw _bridge._decodeError(bytes);
    } finally {
      server.stop();
    }
  }

  @override
  Future<void> dispose() async {
    await _bridge._send('editorDispose', {'handleId': _handleId});
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

  Uint8List _encodeWatermark(int page, String text, PdfWatermarkStyle style) {
    final textBytes = utf8.encode(text);
    final bd = ByteData(8 + textBytes.length + 48);
    bd.setInt32(0, page, Endian.little);
    bd.setInt32(4, textBytes.length, Endian.little);
    bd.buffer.asUint8List().setRange(8, 8 + textBytes.length, textBytes);
    var off = 8 + textBytes.length;
    bd.setFloat64(off, style.fontSize, Endian.little); off += 8;
    bd.setFloat64(off, style.rotation, Endian.little); off += 8;
    bd.setFloat64(off, style.opacity, Endian.little); off += 8;
    bd.setFloat64(off, style.color.r, Endian.little); off += 8;
    bd.setFloat64(off, style.color.g, Endian.little); off += 8;
    bd.setFloat64(off, style.color.b, Endian.little);
    return bd.buffer.asUint8List();
  }

  Uint8List _encodeStamp(int page, int stampType, PdfRect rect, double opacity) {
    final bd = ByteData(44);
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
    bd.setInt32(0, 1, Endian.little); // count
    bd.setInt32(4, page, Endian.little);
    bd.setInt32(8, degrees, Endian.little);
    return bd.buffer.asUint8List();
  }
}

class _CollectSink implements PdfSink {
  _CollectSink(this._builder);
  final BytesBuilder _builder;
  @override void write(Uint8List chunk) => _builder.add(chunk);
}

class _MemorySource implements PdfSource {
  _MemorySource(this._data);
  final Uint8List _data;
  @override int get length => _data.length;
  @override Uint8List readAt(int offset, int count) {
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.sublistView(_data, offset, end);
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
      _send('builderSetMetadata', {'handleId': _handleId, 'op': 1, 'value': value});

  @override
  Future<void> setAuthor(String value) =>
      _send('builderSetMetadata', {'handleId': _handleId, 'op': 2, 'value': value});

  @override
  Future<void> setSubject(String value) =>
      _send('builderSetMetadata', {'handleId': _handleId, 'op': 3, 'value': value});

  @override
  Future<void> setKeywords(String value) =>
      _send('builderSetMetadata', {'handleId': _handleId, 'op': 4, 'value': value});

  @override
  Future<BridgePageBuilderHandle> addA4Page() async {
    await _send('builderAddPage', {'handleId': _handleId, 'pageType': 0});
    return _NativePageBuilderHandle(_bridge, _handleId);
  }

  @override
  Future<BridgePageBuilderHandle> addLetterPage() async {
    await _send('builderAddPage', {'handleId': _handleId, 'pageType': 1});
    return _NativePageBuilderHandle(_bridge, _handleId);
  }

  @override
  Future<BridgePageBuilderHandle> addPage({
    required double width, required double height,
  }) async {
    await _send('builderAddPage', {
      'handleId': _handleId, 'pageType': 2, 'width': width, 'height': height,
    });
    return _NativePageBuilderHandle(_bridge, _handleId);
  }

  @override
  Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()}) async {
    final snkServer = SinkServer(output);
    final snkPort = snkServer.start();
    try {
      final result = await _send('builderSave', {
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
    await _send('builderDispose', {'handleId': _handleId});
  }
}

class _NativePageBuilderHandle implements BridgePageBuilderHandle {
  _NativePageBuilderHandle(this._bridge, this._builderHandleId);
  final NativeBridge _bridge;
  final int _builderHandleId;

  Future<void> _pageOp(int opCode, {Uint8List? params, Uint8List? secondary}) =>
      _bridge._send('builderPageOp', {
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

  @override
  Future<void> at(double x, double y) => _pageOp(2, params: _encodeF32x2(x, y));

  @override
  Future<void> text(String text) => _pageOp(3, params: Uint8List.fromList(utf8.encode(text)));

  @override
  Future<void> heading(int level, String text) {
    final textBytes = utf8.encode(text);
    final params = Uint8List(1 + textBytes.length);
    params[0] = level;
    params.setAll(1, textBytes);
    return _pageOp(4, params: params);
  }

  @override
  Future<void> paragraph(String text) =>
      _pageOp(5, params: Uint8List.fromList(utf8.encode(text)));

  @override
  Future<void> space(double points) => _pageOp(6, params: _encodeF32(points));

  @override
  Future<void> horizontalRule() => _pageOp(7);

  @override
  Future<void> image(Uint8List imageBytes, PdfRect rect, {String altText = ''}) {
    final rectBytes = _encodeF32x4(rect.x, rect.y, rect.width, rect.height);
    final altBytes = utf8.encode(altText);
    final params = Uint8List(16 + altBytes.length);
    params.setAll(0, rectBytes);
    params.setAll(16, altBytes);
    return _pageOp(8, params: params, secondary: imageBytes);
  }

  @override
  Future<void> watermark(String text) =>
      _pageOp(9, params: Uint8List.fromList(utf8.encode(text)));

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
  Future<void> comboBox(String name, PdfRect rect, List<String> options,
      {String? selected}) {
    // Not wired — comboBox op code 12 not implemented in Rust dispatch
    return _pageOp(12);
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

  @override
  Future<void> newline() => _pageOp(15);

  @override
  Future<void> newPageSameSize() => _pageOp(16);

  @override
  Future<void> done() => _pageOp(17);
}
