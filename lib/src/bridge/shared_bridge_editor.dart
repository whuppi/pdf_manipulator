// Editor handle — mutations on an open document.
//
// Part of shared_bridge.dart — same library so the handles reach the
// bridge's private _exec/_execKeepSource (where every PdfTask is
// born). One handle concern per file; zero logic beyond
// encode → _exec → decode.

part of 'shared_bridge.dart';

// ══════════════════════════════════════════════════════════════════
// Editor handle — wraps a handleId, routes ops through SharedBridge
// ══════════════════════════════════════════════════════════════════

class _SharedEditorHandle extends BridgeEditorHandle {
  _SharedEditorHandle(this._bridge, this._handleId, this._resourceId);
  final SharedBridge _bridge;
  final int _handleId;
  final int? _resourceId;

  PdfTask<Map<String, Object?>> _exec(
    EngineOp op,
    Map<String, Object?> args, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
  }) => _bridge._exec(
    op,
    {'handleId': _handleId, ...args},
    sources: sources,
    sinks: sinks,
  );

  PdfTask<void> _mutate(
    String editOp, [
    Map<String, Object?> extra = const {},
  ]) => _exec(EngineOp.editorMutate, {'editOp': editOp, ...extra});

  PdfTask<void> _mutateWithSource(
    String editOp,
    DataSource data, [
    Map<String, Object?> extra = const {},
  ]) => _exec(
    EngineOp.editorMutate,
    {'editOp': editOp, 'sourceLength': data.length, ...extra},
    sources: [data],
  );

  // ── Metadata queries ──

  @override
  PdfTask<int> get pageCount => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).pageCount);

  @override
  PdfTask<String> get version => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).version);

  @override
  PdfTask<bool> get isModified => _exec(
    EngineOp.editorIsModified,
    {},
  ).map((map) => map['modified'] as bool? ?? false);

  @override
  PdfTask<String> getTitle() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).title);

  @override
  PdfTask<String> getAuthor() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).author);

  @override
  PdfTask<String> getSubject() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).subject);

  @override
  PdfTask<String> getKeywords() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).keywords);

  @override
  PdfTask<String> getProducer() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).producer);

  @override
  PdfTask<String> getCreationDate() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).creationDate);

  // ── Metadata setters ──

  @override
  PdfTask<void> setTitle(String value) => _mutate('setTitle', {'title': value});
  @override
  PdfTask<void> setAuthor(String value) =>
      _mutate('setAuthor', {'author': value});
  @override
  PdfTask<void> setSubject(String value) =>
      _mutate('setSubject', {'subject': value});
  @override
  PdfTask<void> setKeywords(String value) =>
      _mutate('setKeywords', {'keywords': value});
  @override
  PdfTask<void> setProducer(String value) =>
      _mutate('setProducer', {'producer': value});
  @override
  PdfTask<void> setCreationDate(String value) =>
      _mutate('setCreationDate', {'creationDate': value});

  // ── Pages ──

  @override
  PdfTask<void> rotatePage(int page, {required int degrees}) =>
      _mutate('rotatePage', {'page': page, 'degrees': degrees});
  @override
  PdfTask<void> rotateAllPages({required int degrees}) =>
      _mutate('rotateAll', {'degrees': degrees});
  @override
  PdfTask<PdfRect> getPageMediaBox(int page) => _exec(
    EngineOp.editorPageMediaBox,
    {'page': page},
  ).map((map) => codec.decodeMediaBox(map));
  @override
  PdfTask<void> deletePage(int page) => _mutate('deletePage', {'page': page});
  @override
  PdfTask<void> movePage({required int from, required int to}) =>
      _mutate('movePage', {'from': from, 'to': to});
  @override
  PdfTask<void> selectPages(List<int> pages) =>
      _mutate('selectPages', {'pages': pages});
  @override
  PdfTask<void> mergeFrom(DataSource otherPdf) => _exec(
    EngineOp.editorMergeFrom,
    {'sourceLength': otherPdf.length},
    sources: [otherPdf],
  );

  // ── Optimization ──

  @override
  PdfTask<int> optimizeImages({int quality = 75, int minSize = 128}) => _exec(
    EngineOp.editorMutate,
    {'editOp': 'optimizeImages', 'quality': quality, 'minSize': minSize},
  ).map((map) => map['count'] as int? ?? 0);
  @override
  PdfTask<int> unembedStandardFonts() => _exec(EngineOp.editorMutate, {
    'editOp': 'unembedStandardFonts',
  }).map((map) => map['count'] as int? ?? 0);

  // ── Watermark + stamps ──

  @override
  PdfTask<void> addWatermark(
    int page,
    String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) => _mutate('watermark', {
    'page': page,
    ...codec.encodeWatermarkArgs(text, style, position, layer),
  });

  @override
  PdfTask<void> addStamp(
    int page, {
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  }) => _mutate('addStamp', {
    'page': page,
    'stampType': type.index,
    ...codec.encodeRectArgs(rect),
    'opacity': opacity,
  });

  @override
  PdfTask<void> addImageStamp(
    int page,
    DataSource imageData, {
    required PdfRect rect,
    double opacity = 1.0,
  }) => _mutateWithSource('addImageStamp', imageData, {
    'page': page,
    ...codec.encodeRectArgs(rect),
    'opacity': opacity,
  });

  // ── Content ──

  @override
  PdfTask<void> embedFile(String name, DataSource data) =>
      _mutateWithSource('embedFile', data, {'name': name});
  @override
  PdfTask<void> eraseRegions(int page, List<PdfRect> regions) => _mutate(
    'eraseRegions',
    {'page': page, 'regions': codec.encodeRegions(regions)},
  );
  @override
  PdfTask<void> flattenForms() => _mutate('flattenForms');
  @override
  PdfTask<void> flattenAllAnnotations() => _mutate('flattenAllAnnotations');
  @override
  PdfTask<void> setFormFieldValue(String fieldName, String value) =>
      _mutate('setFormFieldValue', {'fieldName': fieldName, 'value': value});
  @override
  PdfTask<void> cropMargins({
    double left = 0,
    double right = 0,
    double top = 0,
    double bottom = 0,
  }) => _mutate('cropMargins', {
    'left': left,
    'right': right,
    'top': top,
    'bottom': bottom,
  });
  @override
  PdfTask<void> convertToPdfA({int level = 1}) =>
      _mutate('convertToPdfA', {'level': level});
  @override
  PdfTask<void> resizeImage(
    int page,
    String imageName, {
    required double width,
    required double height,
  }) => _mutate('resizeImage', {
    'page': page,
    'imageName': imageName,
    'width': width,
    'height': height,
  });

  // ── Redaction ──

  @override
  PdfTask<void> addRedaction(int page, PdfRect region, {String? overlayText}) =>
      _mutate('addRedaction', {
        'page': page,
        ...codec.encodeRectArgs(region),
        'overlayText': overlayText,
      });
  @override
  PdfTask<int> redactionCount(int page) => _exec(
    EngineOp.editorRedactionCount,
    {'page': page},
  ).map((map) => map['count'] as int? ?? 0);
  @override
  PdfTask<void> applyRedactions() => _mutate('applyRedactionsDestructive');
  @override
  PdfTask<void> scrubMetadata() => _mutate('scrubMetadata');

  // ── Save ──

  @override
  PdfTask<void> save(
    DataSink output, {
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  }) => _exec(
    EngineOp.editorSave,
    codec.encodeSaveArgs(options),
    sinks: [output],
  );

  // ── Extract pages (select → save → restore, editor unchanged) ──

  @override
  PdfTask<void> extractPages(List<int> pages, DataSink output) =>
      _exec(EngineOp.editorExtractPages, {'pages': pages}, sinks: [output]);

  // ── Lifecycle ──

  @override
  Future<void> dispose() async {
    await _bridge._exec(EngineOp.editorDispose, {'handleId': _handleId});
    if (_resourceId != null) {
      await _bridge._transport.releaseSource(_resourceId);
    }
  }
}
