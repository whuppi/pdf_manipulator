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
    Set<int> keepSources = const {},
  }) => _bridge._exec(
    op,
    {'handleId': _handleId, ...args},
    sources: sources,
    sinks: sinks,
    keepSources: keepSources,
    scope: scope,
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

  // ── Metadata ──

  @override
  PdfTask<int> pageCount() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).pageCount);

  @override
  PdfTask<String> version() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).version);

  @override
  PdfTask<bool> isModified() => _exec(
    EngineOp.editorIsModified,
    {},
  ).map((map) => map['modified'] as bool? ?? false);

  @override
  PdfTask<PdfMetadata> metadata() => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).metadata);

  @override
  PdfTask<void> setMetadata(PdfMetadata metadata) =>
      _mutate('setMetadata', codec.encodeMetadataArgs(metadata));

  // ── Pages ──

  @override
  PdfTask<void> rotatePages(PdfPages pages, {required PdfRotation by}) =>
      _mutate('rotatePages', {
        ...codec.encodePages(pages),
        'rotation': codec.putEnum(by),
      });
  @override
  PdfTask<void> setRotation(PdfPages pages, PdfRotation rotation) => _mutate(
    'setRotation',
    {...codec.encodePages(pages), 'rotation': codec.putEnum(rotation)},
  );
  @override
  PdfTask<List<PdfPageInfo>> pageInfo({PdfPages? pages}) => _exec(
    EngineOp.editorPageInfo,
    codec.encodePages(pages ?? const PdfPages.all()),
  ).map(codec.decodePageInfos);
  @override
  PdfTask<void> setMediaBox(PdfPages pages, PdfRect box) => _mutate(
    'setMediaBox',
    {...codec.encodePages(pages), ...codec.encodeRectArgs(box)},
  );
  @override
  PdfTask<void> setCropBox(PdfPages pages, PdfRect? box) =>
      _mutate('setCropBox', {
        ...codec.encodePages(pages),
        if (box != null) ...codec.encodeRectArgs(box),
      });
  @override
  PdfTask<void> deletePages(PdfPages pages) =>
      _mutate('deletePages', codec.encodePages(pages));
  @override
  PdfTask<void> movePage({required int from, required int to}) =>
      _mutate('movePage', {'from': from, 'to': to});
  @override
  PdfTask<void> keepPages(List<int> order) =>
      _mutate('keepPages', {'pages': order});
  @override
  PdfTask<void> mergeFrom(
    DataSource source, {
    PdfPages? pages,
    String? password,
  }) => _exec(
    EngineOp.editorMergeFrom,
    {
      'sourceLength': source.length,
      if (pages != null) ..._mergePages(pages),
      if (password != null) 'password': password,
    },
    sources: [source],
    // The editor keeps the merged document open to save from it; the
    // channel lives until the editor is disposed.
    keepSources: {0},
  );

  /// The wire pages of a merge: the engine reads every page of the source
  /// when the key is absent.
  static Map<String, Object?> _mergePages(PdfPages pages) =>
      pages is PdfPagesAll ? const {} : codec.encodePages(pages);

  // ── Optimization ──

  @override
  PdfTask<PdfImageReport> reduceImages(PdfImagePolicy policy) => _exec(
    EngineOp.editorMutate,
    {'editOp': 'reduceImages', ...codec.encodeImagePolicy(policy)},
  ).map(codec.decodeImageReport);
  @override
  PdfTask<int> unembedStandardFonts() => _exec(EngineOp.editorMutate, {
    'editOp': 'unembedStandardFonts',
  }).map((map) => map['count'] as int? ?? 0);

  // ── Watermark + stamps ──

  @override
  PdfTask<void> addWatermark(
    PdfPages pages,
    String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) => _mutate('watermark', {
    ...codec.encodePages(pages),
    ...codec.encodeWatermarkArgs(text, style, position, layer),
  });

  @override
  PdfTask<void> addStamp(
    PdfPages pages, {
    required PdfStampType type,
    required PdfRect rect,
    required double opacity,
  }) => _mutate('addStamp', {
    ...codec.encodePages(pages),
    'stampType': codec.putEnum(type),
    ...codec.encodeRectArgs(rect),
    'opacity': opacity,
  });

  @override
  PdfTask<void> addImageStamp(
    PdfPages pages,
    DataSource imageData, {
    required PdfRect rect,
    required double opacity,
  }) => _mutateWithSource('addImageStamp', imageData, {
    ...codec.encodePages(pages),
    ...codec.encodeRectArgs(rect),
    'opacity': opacity,
  });

  // ── Content ──

  @override
  PdfTask<void> embedFile(
    String name,
    DataSource data, {
    String? description,
    String? mimeType,
    PdfAttachmentRelationship? relationship,
  }) => _mutateWithSource('embedFile', data, {
    'name': name,
    if (description != null) 'description': description,
    if (mimeType != null) 'mimeType': mimeType,
    if (relationship != null) 'relationship': codec.putEnum(relationship),
  });
  @override
  PdfTask<void> flattenForms(PdfPages pages) =>
      _mutate('flattenForms', codec.encodePages(pages));
  @override
  PdfTask<void> flattenAnnotations(PdfPages pages) =>
      _mutate('flattenAnnotations', codec.encodePages(pages));
  @override
  PdfTask<void> setFormFieldValue(String fieldName, String value) =>
      _mutate('setFormFieldValue', {'fieldName': fieldName, 'value': value});
  @override
  PdfTask<void> setFormFieldChoice(String fieldName, String value) =>
      _mutate('setFormFieldChoice', {'fieldName': fieldName, 'value': value});
  @override
  PdfTask<void> setFormFieldChoices(String fieldName, List<String> values) =>
      _mutate('setFormFieldChoices', {
        'fieldName': fieldName,
        'values': values,
      });
  @override
  PdfTask<void> clearFormFieldValue(String fieldName) =>
      _mutate('clearFormFieldValue', {'fieldName': fieldName});
  @override
  PdfTask<void> setCheckboxFieldValue(String fieldName, bool checked) =>
      _mutate('setCheckboxFieldValue', {
        'fieldName': fieldName,
        'checked': checked,
      });
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
  PdfTask<void> convertToPdfA({PdfALevel level = PdfALevel.a2}) =>
      _mutate('convertToPdfA', {'level': codec.putEnum(level)});
  @override
  PdfTask<List<PdfPageImage>> pageImages(int page) => _exec(
    EngineOp.editorPageImages,
    {'page': page},
  ).map(codec.decodePageImages);

  @override
  PdfTask<void> setImageBounds(int page, String imageName, PdfRect bounds) =>
      _mutate('setImageBounds', {
        'page': page,
        'imageName': imageName,
        'x': bounds.x,
        'y': bounds.y,
        'width': bounds.width,
        'height': bounds.height,
      });

  // ── Form field properties ──

  @override
  PdfTask<void> removeFormField(String name) =>
      _mutate('removeFormField', {'name': name});

  @override
  PdfTask<void> setFormFieldReadOnly(String name, bool readOnly) =>
      _mutate('setFormFieldReadOnly', {'name': name, 'readOnly': readOnly});

  @override
  PdfTask<void> setFormFieldRequired(String name, bool required) =>
      _mutate('setFormFieldRequired', {'name': name, 'required': required});

  @override
  PdfTask<void> setFormFieldTooltip(String name, String? tooltip) => _mutate(
    'setFormFieldTooltip',
    {'name': name, if (tooltip != null) 'tooltip': tooltip},
  );

  @override
  PdfTask<void> setFormFieldBounds(String name, PdfRect bounds) => _mutate(
    'setFormFieldBounds',
    {'name': name, ...codec.encodeRectArgs(bounds)},
  );

  @override
  PdfTask<void> setFormFieldMaxLength(String name, int? maxLength) => _mutate(
    'setFormFieldMaxLength',
    {'name': name, if (maxLength != null) 'maxLength': maxLength},
  );

  @override
  PdfTask<void> setFormFieldAlignment(String name, PdfAlignment alignment) =>
      _mutate('setFormFieldAlignment', {
        'name': name,
        'alignment': codec.putEnum(alignment),
      });

  @override
  PdfTask<void> setFormFieldBackgroundColor(String name, PdfColor? color) =>
      _mutate('setFormFieldBackgroundColor', {
        'name': name,
        if (color != null) ...codec.encodeColorArgs(color),
      });

  @override
  PdfTask<void> setFormFieldBorderColor(String name, PdfColor? color) =>
      _mutate('setFormFieldBorderColor', {
        'name': name,
        if (color != null) ...codec.encodeColorArgs(color),
      });

  @override
  PdfTask<void> setFormFieldBorderWidth(String name, double width) =>
      _mutate('setFormFieldBorderWidth', {'name': name, 'width': width});

  @override
  PdfTask<void> setFormFieldAppearance(
    String name, {
    required String font,
    required double fontSize,
    PdfColor color = PdfColor.black,
  }) => _mutate('setFormFieldAppearance', {
    'name': name,
    'font': font,
    'fontSize': fontSize,
    ...codec.encodeColorArgs(color),
  });

  @override
  PdfTask<void> setFormFieldFlags(String name, Set<PdfFormFieldFlag> flags) =>
      _mutate('setFormFieldFlags', {
        'name': name,
        'flags': codec.encodeFormFieldFlags(flags),
      });

  // ── Redaction ──

  @override
  PdfTask<void> markRedaction(
    int page,
    PdfRect region,
    PdfRedactionOverlay overlay,
  ) => _mutate('addRedaction', {
    'page': page,
    ...codec.encodeRectArgs(region),
    ...codec.encodeRedactionOverlay(overlay),
  });
  @override
  PdfTask<void> clearRedactions() => _mutate('clearRedactions');
  @override
  PdfTask<PdfRedactionReport> redact(
    int page,
    List<PdfRect> regions,
    PdfRedactionOverlay overlay,
  ) => _exec(EngineOp.editorMutate, {
    'editOp': 'redact',
    'page': page,
    'regions': codec.encodeRegions(regions),
    ...codec.encodeRedactionOverlay(overlay),
  }).map(codec.decodeRedactionReport);
  @override
  PdfTask<int> redactionCount(int page) => _exec(
    EngineOp.editorRedactionCount,
    {'page': page},
  ).map((map) => map['count'] as int? ?? 0);
  @override
  PdfTask<PdfRedactionReport> applyRedactions() => _exec(
    EngineOp.editorMutate,
    {'editOp': 'applyRedactions'},
  ).map(codec.decodeRedactionReport);
  @override
  PdfTask<void> scrubMetadata() => _mutate('scrubMetadata');
  @override
  PdfTask<void> sanitize(PdfSanitizeOptions options) => _mutate('sanitize', {
    'metadata': options.metadata,
    'javascript': options.javascript,
    'embeddedFiles': options.embeddedFiles,
  });

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
