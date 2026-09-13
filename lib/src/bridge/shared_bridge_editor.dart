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
  PdfTask<String> get title => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).title);

  @override
  PdfTask<String> get author => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).author);

  @override
  PdfTask<String> get subject => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).subject);

  @override
  PdfTask<String> get keywords => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).keywords);

  @override
  PdfTask<String> get producer => _exec(
    EngineOp.editorGetMetadata,
    {},
  ).map((map) => codec.decodeEditorMetadata(map).producer);

  @override
  PdfTask<String> get creationDate => _exec(
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
  PdfTask<PdfRect> pageMediaBox(int page) => _exec(
    EngineOp.editorPageMediaBox,
    {'page': page},
  ).map((map) => codec.decodeMediaBox(map));
  @override
  PdfTask<PdfRect?> pageCropBox(int page) => _exec(EngineOp.editorPageCropBox, {
    'page': page,
  }).map((map) => codec.decodeCropBox(map));
  @override
  PdfTask<void> setPageMediaBox(int page, PdfRect box) =>
      _mutate('setPageMediaBox', {'page': page, ...codec.encodeRectArgs(box)});
  @override
  PdfTask<void> setPageCropBox(int page, PdfRect box) =>
      _mutate('setPageCropBox', {'page': page, ...codec.encodeRectArgs(box)});
  @override
  PdfTask<void> setPageRotation(int page, {required int degrees}) =>
      _mutate('setPageRotation', {'page': page, 'degrees': degrees});
  @override
  PdfTask<void> deletePage(int page) => _mutate('deletePage', {'page': page});
  @override
  PdfTask<void> movePage({required int from, required int to}) =>
      _mutate('movePage', {'from': from, 'to': to});
  @override
  PdfTask<void> selectPages(List<int> pages) =>
      _mutate('selectPages', {'pages': pages});
  @override
  PdfTask<void> mergeFrom(DataSource otherPdf, {List<int>? pages}) => _exec(
    EngineOp.editorMergeFrom,
    {'sourceLength': otherPdf.length, if (pages != null) 'pages': pages},
    sources: [otherPdf],
  );

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
    if (relationship != null) 'relationship': relationship.name,
  });
  @override
  PdfTask<void> eraseRegions(int page, List<PdfRect> regions) => _mutate(
    'eraseRegions',
    {'page': page, 'regions': codec.encodeRegions(regions)},
  );
  @override
  PdfTask<void> flattenForms({int? page}) =>
      _mutate('flattenForms', {if (page != null) 'page': page});
  @override
  PdfTask<void> flattenAnnotations({int? page}) =>
      _mutate('flattenAnnotations', {if (page != null) 'page': page});
  @override
  PdfTask<void> clearEraseRegions(int page) =>
      _mutate('clearEraseRegions', {'page': page});
  @override
  PdfTask<void> setFormFieldValue(String fieldName, String value) =>
      _mutate('setFormFieldValue', {'fieldName': fieldName, 'value': value});
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
  PdfTask<void> convertToPdfA({int level = 1}) =>
      _mutate('convertToPdfA', {'level': level});
  @override
  PdfTask<List<PdfPageImage>> pageImages(int page) => _exec(
    EngineOp.editorPageImages,
    {'page': page},
  ).map(codec.decodePageImages);

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

  @override
  PdfTask<void> repositionImage(
    int page,
    String imageName, {
    required double x,
    required double y,
  }) => _mutate('repositionImage', {
    'page': page,
    'imageName': imageName,
    'x': x,
    'y': y,
  });

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
  PdfTask<void> setFormFieldTooltip(String name, String tooltip) =>
      _mutate('setFormFieldTooltip', {'name': name, 'tooltip': tooltip});

  @override
  PdfTask<void> setFormFieldBounds(String name, PdfRect bounds) => _mutate(
    'setFormFieldBounds',
    {'name': name, ...codec.encodeRectArgs(bounds)},
  );

  @override
  PdfTask<void> setFormFieldMaxLength(String name, int maxLength) =>
      _mutate('setFormFieldMaxLength', {'name': name, 'maxLength': maxLength});

  @override
  PdfTask<void> setFormFieldAlignment(
    String name,
    PdfTextAlignment alignment,
  ) => _mutate('setFormFieldAlignment', {
    'name': name,
    'alignment': alignment.index,
  });

  @override
  PdfTask<void> setFormFieldBackgroundColor(String name, PdfColor color) =>
      _mutate('setFormFieldBackgroundColor', {
        'name': name,
        ...codec.encodeColorArgs(color),
      });

  @override
  PdfTask<void> setFormFieldBorderColor(String name, PdfColor color) => _mutate(
    'setFormFieldBorderColor',
    {'name': name, ...codec.encodeColorArgs(color)},
  );

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
