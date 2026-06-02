import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/pdf_transport.dart';
import 'package:pdf_manipulator/src/transport/protocol/binary_codec.dart'
    as bin;
import 'package:pdf_manipulator/src/transport/protocol/codec.dart' as codec;
import 'package:pdf_manipulator/src/transport/protocol/op.dart';

/// One bridge class for both platforms. Encodes ops to binary,
/// sends via PdfTransport, decodes binary response.
///
/// O(1)-memory I/O: source bytes served on demand via DataSource,
/// output chunks flow via DataSink. The binary codec handles only
/// the ~200 bytes of op arguments — PDF data travels through the
/// transport's dedicated I/O channels.
class SharedBridge extends PdfBridge {
  SharedBridge(this._transport);
  final PdfTransport _transport;

  @override
  PdfIoMode? get ioMode => _transport.ioMode;

  @override
  Future<PdfIoMode> ensureInitialized() => _transport.ensureInitialized();

  /// Encode → send → decode. The core pattern for every operation.
  Future<Map<String, Object?>> _exec(
    EngineOp op,
    Map<String, Object?> args, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
  }) async {
    final req = bin.encodeRequest(op.wire, args);
    final res = await _transport.execute(req, sources: sources, sinks: sinks);
    final map = bin.decodeResponse(res.bytes);
    if (map.containsKey('error')) {
      throw StateError(map['error'] as String? ?? 'Engine error');
    }
    return map;
  }

  /// Execute keeping sources[0] alive for handle lifetime.
  Future<({Map<String, Object?> map, int? resourceId})> _execKeepSource(
    EngineOp op,
    Map<String, Object?> args, {
    required DataSource source,
  }) async {
    final req = bin.encodeRequest(op.wire, args);
    final res = await _transport.execute(req,
        sources: [source], keepSources: {0});
    final map = bin.decodeResponse(res.bytes);
    if (map.containsKey('error')) {
      throw StateError(map['error'] as String? ?? 'Engine error');
    }
    return (map: map, resourceId: res.resourceIds[0]);
  }

  // ══════════════════════════════════════════════════════════════════
  // Document handle — open once, query many, dispose
  // ══════════════════════════════════════════════════════════════════

  @override
  Future<BridgeDocHandle> open(DataSource source, {String? password}) async {
    final res = await _execKeepSource(EngineOp.open, {
      'sourceLength': source.length,
      'password': password,
    }, source: source);
    final handleId = res.map['handleId'] as int;
    return _SharedDocHandle(this, handleId, res.map, res.resourceId);
  }

  // ══════════════════════════════════════════════════════════════════
  // One-shot ops — source consumed in one call, no handle
  // ══════════════════════════════════════════════════════════════════

  @override
  Future<void> sign(DataSource source, DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  }) async {
    final args = <String, Object?>{
      'sourceLength': source.length,
      'reason': reason,
      'location': location,
    };
    switch (credentials) {
      case PdfPkcs12Credentials(:final data, :final password):
        args['certificate'] = data;
        args['certificatePassword'] = password;
      case PdfPemCredentials(:final certPem, :final keyPem):
        args['certPem'] = certPem;
        args['keyPem'] = keyPem;
    }
    await _exec(EngineOp.sign, args, sources: [source], sinks: [output]);
  }

  @override
  Future<void> convertTo(DataSource source, DataSink output, {
    required PdfDocumentFormat format,
    String? password,
  }) async {
    await _exec(EngineOp.convertTo, {
      'sourceLength': source.length,
      'format': format.name,
      'password': password,
    }, sources: [source], sinks: [output]);
  }

  @override
  Future<void> convertToPdf(DataSource document, DataSink output, {
    required PdfDocumentFormat format,
  }) async {
    await _exec(EngineOp.convertToPdf, {
      'sourceLength': document.length,
      'format': format.name,
    }, sources: [document], sinks: [output]);
  }

  // ══════════════════════════════════════════════════════════════════
  // Editor + Builder — handle-based sessions
  // ══════════════════════════════════════════════════════════════════

  @override
  Future<BridgeEditorHandle> openEditor(DataSource source, {String? password}) async {
    final res = await _execKeepSource(EngineOp.editorOpen, {
      'sourceLength': source.length,
      'password': password,
    }, source: source);
    final handleId = codec.decodeEditorOpen(res.map);
    return _SharedEditorHandle(this, handleId, res.resourceId);
  }

  @override
  Future<BridgeBuilderHandle> createBuilder() async {
    final map = await _exec(EngineOp.builderCreate, {});
    final handleId = map['handleId'] as int;
    return _SharedBuilderHandle(this, handleId);
  }

  @override
  Future<void> dispose() => _transport.dispose();

  // ── Helpers ──

  List<int> _resolvePages(PdfPages pages, int pageCount) => switch (pages) {
    PdfAllPages() => List.generate(pageCount, (i) => i),
    PdfSinglePage(:final index) => [index],
    PdfPageList(:final indices) => indices,
    PdfPageRange(:final start, :final end) => List.generate(end - start, (i) => start + i),
  };

  String _encodeExtractionFormat(PdfExtractionFormat format) => switch (format) {
    PdfExtractionFormat.auto => 'auto',
    PdfExtractionFormat.text => 'text',
    PdfExtractionFormat.markdown => 'markdown',
    PdfExtractionFormat.html => 'html',
    PdfExtractionFormat.plainText => 'plainText',
  };
}

// ══════════════════════════════════════════════════════════════════
// Document handle — wraps a handleId, routes read ops to the engine
// ══════════════════════════════════════════════════════════════════

class _SharedDocHandle extends BridgeDocHandle {
  _SharedDocHandle(this._bridge, this._handleId, this._openResult, this._resourceId);
  final SharedBridge _bridge;
  final int _handleId;
  final Map<String, Object?> _openResult;
  final int? _resourceId;

  int get _pageCount => _openResult['pageCount'] as int? ?? 0;

  List<int> _pages(PdfPages pages) => _bridge._resolvePages(pages, _pageCount);

  Future<Map<String, Object?>> _exec(EngineOp op, Map<String, Object?> args, {
    List<DataSink> sinks = const [],
  }) => _bridge._exec(op, {'handleId': _handleId, ...args}, sinks: sinks);

  @override
  Map<String, Object?> get openResult => _openResult;

  @override
  Future<String> extract({
    required PdfPages pages,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) async {
    final fmt = _bridge._encodeExtractionFormat(format);
    return switch (pages) {
      PdfAllPages() => codec.decodeExtractResult(
          await _exec(EngineOp.extract, {'format': fmt})),
      PdfSinglePage(:final index) => codec.decodeExtractResult(
          await _exec(EngineOp.extract, {'format': fmt, 'page': index})),
      PdfPageList(:final indices) => () async {
          final buf = StringBuffer();
          for (final i in indices) {
            buf.write(codec.decodeExtractResult(
                await _exec(EngineOp.extract, {'format': fmt, 'page': i})));
          }
          return buf.toString();
        }(),
      PdfPageRange(:final start, :final end) => () async {
          final buf = StringBuffer();
          for (var i = start; i < end; i++) {
            buf.write(codec.decodeExtractResult(
                await _exec(EngineOp.extract, {'format': fmt, 'page': i})));
          }
          return buf.toString();
        }(),
    };
  }

  @override
  Future<List<SearchResult>> search({
    required String query,
    required PdfPages pages,
  }) async {
    return switch (pages) {
      PdfAllPages() => codec.decodeSearchResults(
          await _exec(EngineOp.search, {'query': query})),
      PdfSinglePage(:final index) => codec.decodeSearchResults(
          await _exec(EngineOp.search, {'query': query, 'page': index})),
      PdfPageList(:final indices) => () async {
          final all = <SearchResult>[];
          for (final i in indices) {
            all.addAll(codec.decodeSearchResults(
                await _exec(EngineOp.search, {'query': query, 'page': i})));
          }
          return all;
        }(),
      PdfPageRange(:final start, :final end) => () async {
          final all = <SearchResult>[];
          for (var i = start; i < end; i++) {
            all.addAll(codec.decodeSearchResults(
                await _exec(EngineOp.search, {'query': query, 'page': i})));
          }
          return all;
        }(),
    };
  }

  @override
  Stream<RenderedPage> render({
    required PdfPages pages,
    PdfRenderSize? size,
  }) async* {
    final sink = _FrameCollectorSink();
    final req = bin.encodeRequest(EngineOp.render.wire, {
      'handleId': _handleId,
      'pageIndices': _pages(pages),
      if (size?.maxWidth != null) 'maxWidth': size!.maxWidth,
      if (size?.maxHeight != null) 'maxHeight': size!.maxHeight,
      'hasSink': true,
    });
    await _bridge._transport.execute(req, sinks: [sink]);
    for (final frame in sink.frames) {
      final map = bin.decodeResponse(frame);
      if (map.containsKey('error')) throw StateError(map['error'] as String);
      yield codec.decodeRenderedPage(map);
    }
  }

  @override
  Stream<PdfImage> extractImages({required PdfPages pages}) async* {
    final pageIndices = _pages(pages);
    for (final page in pageIndices) {
      final sink = _FrameCollectorSink();
      final req = bin.encodeRequest(EngineOp.extractImages.wire, {
        'handleId': _handleId,
        'page': page,
        'hasSink': true,
      });
      await _bridge._transport.execute(req, sinks: [sink]);
      for (final frame in sink.frames) {
        final map = bin.decodeResponse(frame);
        if (map.containsKey('error')) throw StateError(map['error'] as String);
        yield codec.decodePdfImage(map);
      }
    }
  }

  @override
  Future<List<PdfSignatureInfo>> getSignatures() async {
    final map = await _exec(EngineOp.getSignatures, {});
    return codec.decodeSignatures(map);
  }

  @override
  Future<bool> verifySignatures() async {
    final map = await _exec(EngineOp.verifySignatures, {});
    return codec.decodeVerifySignatures(map);
  }

  @override
  Future<PdfValidationResult> validatePdfA({int level = 2}) async {
    final map = await _exec(EngineOp.validatePdfA, {'level': level});
    return codec.decodeValidationResult(map);
  }

  @override
  Future<bool> validatePdfUa({int level = 1}) async {
    final map = await _exec(EngineOp.validatePdfUa, {'level': level});
    return codec.decodeValidatePdfUa(map);
  }

  @override
  Future<List<PdfBookmarkSplit>> planSplitByBookmarks() async {
    final map = await _exec(EngineOp.planSplitByBookmarks, {});
    return codec.decodeBookmarkSplits(map);
  }

  @override
  Future<PdfPageClassification> classifyPage(int page) async {
    final map = await _exec(EngineOp.classifyPage, {'page': page});
    return codec.decodeClassifyPage(map);
  }

  @override
  Future<PdfDocumentClassification> classifyDocument() async {
    final map = await _exec(EngineOp.classifyDocument, {});
    return codec.decodeClassifyDocument(map);
  }

  @override
  Future<void> dispose() async {
    await _bridge._exec(EngineOp.docDispose, {'handleId': _handleId});
    if (_resourceId != null) {
      await _bridge._transport.releaseSource(_resourceId);
    }
  }
}

// ══════════════════════════════════════════════════════════════════
// Editor handle — wraps a handleId, routes ops through SharedBridge
// ══════════════════════════════════════════════════════════════════

class _SharedEditorHandle extends BridgeEditorHandle {
  _SharedEditorHandle(this._bridge, this._handleId, this._resourceId);
  final SharedBridge _bridge;
  final int _handleId;
  final int? _resourceId;

  Future<Map<String, Object?>> _exec(EngineOp op, Map<String, Object?> args, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
  }) => _bridge._exec(op, {'handleId': _handleId, ...args},
      sources: sources, sinks: sinks);

  Future<void> _mutate(String editOp, [Map<String, Object?> extra = const {}]) =>
      _exec(EngineOp.editorMutate, {'editOp': editOp, ...extra});

  Future<void> _mutateWithSource(String editOp, DataSource data,
      [Map<String, Object?> extra = const {}]) =>
      _exec(EngineOp.editorMutate, {
        'editOp': editOp,
        'sourceLength': data.length,
        ...extra,
      }, sources: [data]);

  // ── Metadata queries ──

  @override
  Future<int> get pageCount async =>
      codec.decodeEditorMetadata(await _exec(EngineOp.editorGetMetadata, {})).pageCount;

  @override
  Future<String> get version async =>
      codec.decodeEditorMetadata(await _exec(EngineOp.editorGetMetadata, {})).version;

  @override
  Future<bool> get isModified async =>
      (await _exec(EngineOp.editorIsModified, {}))['modified'] as bool? ?? false;

  @override
  Future<String> getTitle() async =>
      codec.decodeEditorMetadata(await _exec(EngineOp.editorGetMetadata, {})).title;

  @override
  Future<String> getAuthor() async =>
      codec.decodeEditorMetadata(await _exec(EngineOp.editorGetMetadata, {})).author;

  @override
  Future<String> getSubject() async =>
      codec.decodeEditorMetadata(await _exec(EngineOp.editorGetMetadata, {})).subject;

  @override
  Future<String> getKeywords() async =>
      codec.decodeEditorMetadata(await _exec(EngineOp.editorGetMetadata, {})).keywords;

  // ── Metadata setters ──

  @override Future<void> setTitle(String value) => _mutate('setTitle', {'title': value});
  @override Future<void> setAuthor(String value) => _mutate('setAuthor', {'author': value});
  @override Future<void> setSubject(String value) => _mutate('setSubject', {'subject': value});
  @override Future<void> setKeywords(String value) => _mutate('setKeywords', {'keywords': value});

  // ── Pages ──

  @override Future<void> rotatePage(int page, {required int degrees}) =>
      _mutate('rotatePage', {'page': page, 'degrees': degrees});
  @override Future<void> rotateAllPages({required int degrees}) =>
      _mutate('rotateAll', {'degrees': degrees});
  @override Future<PdfRect> getPageMediaBox(int page) async =>
      codec.decodeMediaBox(await _exec(EngineOp.editorPageMediaBox, {'page': page}));
  @override Future<void> deletePage(int page) =>
      _mutate('deletePage', {'page': page});
  @override Future<void> movePage({required int from, required int to}) =>
      _mutate('movePage', {'from': from, 'to': to});
  @override Future<void> selectPages(List<int> pages) =>
      _mutate('selectPages', {'pages': pages});
  @override Future<void> mergeFrom(DataSource otherPdf) =>
      _exec(EngineOp.editorMergeFrom, {
        'sourceLength': otherPdf.length,
      }, sources: [otherPdf]);

  // ── Optimization ──

  @override Future<int> optimizeImages({int quality = 75, int minSize = 128}) async =>
      (await _exec(EngineOp.editorMutate, {'editOp': 'optimizeImages', 'quality': quality, 'minSize': minSize}))['count'] as int? ?? 0;
  @override Future<int> unembedStandardFonts() async =>
      (await _exec(EngineOp.editorMutate, {'editOp': 'unembedStandardFonts'}))['count'] as int? ?? 0;

  // ── Watermark + stamps ──

  @override Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) => _mutate('watermark', {'page': page, ...codec.encodeWatermarkArgs(text, style, position, layer)});

  @override Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect, double opacity = 1.0,
  }) => _mutate('addStamp', {'page': page, 'stampType': type.index, ...codec.encodeRectArgs(rect), 'opacity': opacity});

  @override Future<void> addImageStamp(int page, DataSource imageData, {
    required PdfRect rect, double opacity = 1.0,
  }) => _mutateWithSource('addImageStamp', imageData, {
        'page': page, ...codec.encodeRectArgs(rect), 'opacity': opacity,
      });

  // ── Content ──

  @override Future<void> embedFile(String name, DataSource data) =>
      _mutateWithSource('embedFile', data, {'name': name});
  @override Future<void> eraseRegions(int page, List<PdfRect> regions) =>
      _mutate('eraseRegions', {'page': page, 'regions': codec.encodeRegions(regions)});
  @override Future<void> flattenForms() => _mutate('flattenForms');
  @override Future<void> flattenAllAnnotations() => _mutate('flattenAllAnnotations');
  @override Future<void> setFormFieldValue(String fieldName, String value) =>
      _mutate('setFormFieldValue', {'fieldName': fieldName, 'value': value});
  @override Future<void> cropMargins({
    double left = 0, double right = 0, double top = 0, double bottom = 0,
  }) => _mutate('cropMargins', {'left': left, 'right': right, 'top': top, 'bottom': bottom});
  @override Future<void> convertToPdfA({int level = 1}) =>
      _mutate('convertToPdfA', {'level': level});
  @override Future<void> resizeImage(int page, String imageName, {
    required double width, required double height,
  }) => _mutate('resizeImage', {'page': page, 'imageName': imageName, 'width': width, 'height': height});

  // ── Redaction ──

  @override Future<void> addRedaction(int page, PdfRect region, {String? overlayText}) =>
      _mutate('addRedaction', {'page': page, ...codec.encodeRectArgs(region), 'overlayText': overlayText});
  @override Future<int> redactionCount(int page) async =>
      (await _exec(EngineOp.editorRedactionCount, {'page': page}))['count'] as int? ?? 0;
  @override Future<void> applyRedactions() => _mutate('applyRedactionsDestructive');
  @override Future<void> scrubMetadata() => _mutate('scrubMetadata');

  // ── Save ──

  @override Future<void> save(DataSink output, {PdfSaveOptions options = const PdfSaveOptions.fullRewrite()}) async {
    await _exec(EngineOp.editorSave, codec.encodeSaveArgs(options), sinks: [output]);
  }

  // ── Extract pages (select → save → restore, editor unchanged) ──

  @override Future<void> extractPages(List<int> pages, DataSink output) async {
    await _exec(EngineOp.editorExtractPages, {'pages': pages}, sinks: [output]);
  }

  // ── Lifecycle ──

  @override Future<void> dispose() async {
    await _bridge._exec(EngineOp.editorDispose, {'handleId': _handleId});
    if (_resourceId != null) {
      await _bridge._transport.releaseSource(_resourceId);
    }
  }
}

// ══════════════════════════════════════════════════════════════════
// Builder handle
// ══════════════════════════════════════════════════════════════════

class _SharedBuilderHandle extends BridgeBuilderHandle {
  _SharedBuilderHandle(this._bridge, this._handleId);
  final SharedBridge _bridge;
  final int _handleId;

  Future<Map<String, Object?>> _exec(EngineOp op, Map<String, Object?> args) =>
      _bridge._exec(op, {'handleId': _handleId, ...args});

  @override Future<void> setTitle(String value) => _exec(EngineOp.builderSetMetadata, {'title': value});
  @override Future<void> setAuthor(String value) => _exec(EngineOp.builderSetMetadata, {'author': value});
  @override Future<void> setSubject(String value) => _exec(EngineOp.builderSetMetadata, {'subject': value});
  @override Future<void> setKeywords(String value) => _exec(EngineOp.builderSetMetadata, {'keywords': value});

  @override Future<BridgePageBuilderHandle> addA4Page() async {
    await _exec(EngineOp.builderAddPage, {'pageType': 'a4'});
    return _SharedPageBuilderHandle(_bridge, _handleId);
  }
  @override Future<BridgePageBuilderHandle> addLetterPage() async {
    await _exec(EngineOp.builderAddPage, {'pageType': 'letter'});
    return _SharedPageBuilderHandle(_bridge, _handleId);
  }
  @override Future<BridgePageBuilderHandle> addPage({required double width, required double height}) async {
    await _exec(EngineOp.builderAddPage, {'width': width, 'height': height});
    return _SharedPageBuilderHandle(_bridge, _handleId);
  }

  @override Future<void> save(DataSink output) =>
      _bridge._exec(EngineOp.builderSave, {'handleId': _handleId}, sinks: [output]);
  @override Future<void> dispose() =>
      _exec(EngineOp.builderDispose, {});
}

// ══════════════════════════════════════════════════════════════════
// Page builder handle
// ══════════════════════════════════════════════════════════════════

class _SharedPageBuilderHandle extends BridgePageBuilderHandle {
  _SharedPageBuilderHandle(this._bridge, this._handleId);
  final SharedBridge _bridge;
  final int _handleId;

  Future<void> _op(String op, [Map<String, Object?> extra = const {}]) =>
      _bridge._exec(EngineOp.builderPageOp, {'handleId': _handleId, 'pageOp': op, ...extra});

  @override Future<void> font(String name, double size) => _op('font', {'name': name, 'size': size});
  @override Future<void> at(double x, double y) => _op('at', {'x': x, 'y': y});
  @override Future<void> text(String text) => _op('text', {'text': text});
  @override Future<void> heading(int level, String text) => _op('heading', {'level': level, 'text': text});
  @override Future<void> paragraph(String text) => _op('paragraph', {'text': text});
  @override Future<void> space(double points) => _op('space', {'points': points});
  @override Future<void> horizontalRule() => _op('horizontalRule');
  @override Future<void> image(DataSource imageData, PdfRect rect, {String altText = ''}) =>
      _bridge._exec(EngineOp.builderPageOp, {
        'handleId': _handleId,
        'pageOp': 'image',
        'sourceLength': imageData.length,
        ...codec.encodeRectArgs(rect),
        'altText': altText,
      }, sources: [imageData]);
  @override Future<void> watermark(String text) => _op('watermark', {'text': text});
  @override Future<void> textField(String name, PdfRect rect, {String? defaultValue}) =>
      _op('textField', {'name': name, ...codec.encodeRectArgs(rect), 'defaultValue': defaultValue});
  @override Future<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _op('checkbox', {'name': name, ...codec.encodeRectArgs(rect), 'checked': checked});
  @override Future<void> comboBox(String name, PdfRect rect, List<String> options, {String? selected}) =>
      _op('comboBox', {'name': name, ...codec.encodeRectArgs(rect), 'options': options, 'selected': selected});
  @override Future<void> pushButton(String name, PdfRect rect, String caption) =>
      _op('pushButton', {'name': name, ...codec.encodeRectArgs(rect), 'caption': caption});
  @override Future<void> signatureField(String name, PdfRect rect) =>
      _op('signatureField', {'name': name, ...codec.encodeRectArgs(rect)});
  @override Future<void> radioGroup(String name, List<({String value, PdfRect rect})> options, {String? selected}) =>
      _op('radioGroup', {'name': name, 'options': options.map((o) => <String, Object?>{'value': o.value, ...codec.encodeRectArgs(o.rect)}).toList(), 'selected': selected});
  @override Future<void> fieldKeystroke(String script) => _op('fieldKeystroke', {'script': script});
  @override Future<void> fieldFormat(String script) => _op('fieldFormat', {'script': script});
  @override Future<void> fieldValidate(String script) => _op('fieldValidate', {'script': script});
  @override Future<void> fieldCalculate(String script) => _op('fieldCalculate', {'script': script});
  @override Future<void> linkUrl(String url) => _op('linkUrl', {'url': url});
  @override Future<void> linkPage(int targetPage) => _op('linkPage', {'targetPage': targetPage});
  @override Future<void> footnote(String refMark, String noteText) => _op('footnote', {'refMark': refMark, 'noteText': noteText});
  @override Future<void> columns(int columnCount, double gapPt, String text) => _op('columns', {'columnCount': columnCount, 'gapPt': gapPt, 'text': text});
  @override Future<void> newline() => _op('newline');
  @override Future<void> newPageSameSize() => _op('newPageSameSize');
  @override Future<void> done() => _bridge._exec(EngineOp.builderPageDone, {'handleId': _handleId});
}

// ── O(1)-memory framed sink for streaming ops ──────────────────────

class _FrameCollectorSink implements DataSink {
  final _buffer = BytesBuilder(copy: false);
  final _frames = <Uint8List>[];

  List<Uint8List> get frames => _frames;

  @override
  void write(Uint8List chunk) {
    _buffer.add(chunk);
    _parseFrames();
  }

  void _parseFrames() {
    while (true) {
      final bytes = _buffer.toBytes();
      if (bytes.length < 4) break;
      final len = ByteData.sublistView(bytes).getUint32(0, Endian.little);
      if (len == 0) {
        _buffer.clear();
        if (bytes.length > 4) {
          _buffer.add(Uint8List.sublistView(bytes, 4));
        }
        break;
      }
      if (bytes.length < 4 + len) break;
      _frames.add(Uint8List.sublistView(bytes, 4, 4 + len));
      _buffer.clear();
      if (bytes.length > 4 + len) {
        _buffer.add(Uint8List.sublistView(bytes, 4 + len));
      }
    }
  }
}
