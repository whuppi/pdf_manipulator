// Builder + page-builder handles — create from scratch.
//
// Part of shared_bridge.dart — same library so the handles reach the
// bridge's private _exec/_execKeepSource (where every PdfTask is
// born). One handle concern per file; zero logic beyond
// encode → _exec → decode.

part of 'shared_bridge.dart';

// ══════════════════════════════════════════════════════════════════
// Builder handle
// ══════════════════════════════════════════════════════════════════

class _SharedBuilderHandle extends BridgeBuilderHandle {
  _SharedBuilderHandle(this._bridge, this._handleId);
  final SharedBridge _bridge;
  final int _handleId;

  PdfTask<Map<String, Object?>> _exec(EngineOp op, Map<String, Object?> args) =>
      _bridge._exec(op, {'handleId': _handleId, ...args});

  @override
  PdfTask<void> setTitle(String value) =>
      _exec(EngineOp.builderSetMetadata, {'title': value});
  @override
  PdfTask<void> setAuthor(String value) =>
      _exec(EngineOp.builderSetMetadata, {'author': value});
  @override
  PdfTask<void> setSubject(String value) =>
      _exec(EngineOp.builderSetMetadata, {'subject': value});
  @override
  PdfTask<void> setKeywords(String value) =>
      _exec(EngineOp.builderSetMetadata, {'keywords': value});

  @override
  PdfTask<BridgePageBuilderHandle> addA4Page() => _exec(
    EngineOp.builderAddPage,
    {'pageType': 'a4'},
  ).map((_) => _SharedPageBuilderHandle(_bridge, _handleId));
  @override
  PdfTask<BridgePageBuilderHandle> addLetterPage() => _exec(
    EngineOp.builderAddPage,
    {'pageType': 'letter'},
  ).map((_) => _SharedPageBuilderHandle(_bridge, _handleId));
  @override
  PdfTask<BridgePageBuilderHandle> addPage({
    required double width,
    required double height,
  }) => _exec(EngineOp.builderAddPage, {
    'width': width,
    'height': height,
  }).map((_) => _SharedPageBuilderHandle(_bridge, _handleId));

  @override
  PdfTask<void> save(DataSink output, {bool compress = true}) => _bridge._exec(
    EngineOp.builderSave,
    {'handleId': _handleId, 'compress': compress},
    sinks: [output],
  );
  @override
  Future<void> dispose() => _exec(EngineOp.builderDispose, {});
}

// ══════════════════════════════════════════════════════════════════
// Page builder handle
// ══════════════════════════════════════════════════════════════════

/// The stamp-type names the engine's WASM-party `parse_wasm_stamp_type`
/// accepts, one per `PdfStampType` value.
String stampWireName(PdfStampType type) => switch (type) {
  PdfStampType.approved => 'Approved',
  PdfStampType.experimental => 'Experimental',
  PdfStampType.notApproved => 'NotApproved',
  PdfStampType.asIs => 'AsIs',
  PdfStampType.expired => 'Expired',
  PdfStampType.notForPublicRelease => 'NotForPublicRelease',
  PdfStampType.confidential => 'Confidential',
  PdfStampType.final_ => 'Final',
  PdfStampType.sold => 'Sold',
  PdfStampType.departmental => 'Departmental',
  PdfStampType.forComment => 'ForComment',
  PdfStampType.topSecret => 'TopSecret',
  PdfStampType.draft => 'Draft',
  PdfStampType.forPublicRelease => 'ForPublicRelease',
};

class _SharedPageBuilderHandle extends BridgePageBuilderHandle {
  _SharedPageBuilderHandle(this._bridge, this._handleId);
  final SharedBridge _bridge;
  final int _handleId;

  PdfTask<void> _op(String op, [Map<String, Object?> extra = const {}]) =>
      _bridge._exec(EngineOp.builderPageOp, {
        'handleId': _handleId,
        'pageOp': op,
        ...extra,
      });

  @override
  PdfTask<void> font(String name, double size) =>
      _op('font', {'name': name, 'size': size});
  @override
  PdfTask<void> at(double x, double y) => _op('at', {'x': x, 'y': y});
  @override
  PdfTask<void> text(String text) => _op('text', {'text': text});
  @override
  PdfTask<void> heading(int level, String text) =>
      _op('heading', {'level': level, 'text': text});
  @override
  PdfTask<void> paragraph(String text) => _op('paragraph', {'text': text});
  @override
  PdfTask<void> space(double points) => _op('space', {'points': points});
  @override
  PdfTask<void> horizontalRule() => _op('horizontalRule');
  @override
  PdfTask<void> image(
    DataSource imageData,
    PdfRect rect, {
    String altText = '',
  }) => _bridge._exec(
    EngineOp.builderPageOp,
    {
      'handleId': _handleId,
      'pageOp': 'image',
      'sourceLength': imageData.length,
      ...codec.encodeRectArgs(rect),
      'altText': altText,
    },
    sources: [imageData],
  );
  @override
  PdfTask<void> watermark(String text) => _op('watermark', {'text': text});
  @override
  PdfTask<void> textField(String name, PdfRect rect, {String? defaultValue}) =>
      _op('textField', {
        'name': name,
        ...codec.encodeRectArgs(rect),
        'defaultValue': defaultValue,
      });
  @override
  PdfTask<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _op('checkbox', {
        'name': name,
        ...codec.encodeRectArgs(rect),
        'checked': checked,
      });
  @override
  PdfTask<void> comboBox(
    String name,
    PdfRect rect,
    List<String> options, {
    String? selected,
  }) => _op('comboBox', {
    'name': name,
    ...codec.encodeRectArgs(rect),
    'options': options,
    'selected': selected,
  });
  @override
  PdfTask<void> pushButton(String name, PdfRect rect, String caption) => _op(
    'pushButton',
    {'name': name, ...codec.encodeRectArgs(rect), 'caption': caption},
  );
  @override
  PdfTask<void> signatureField(String name, PdfRect rect) =>
      _op('signatureField', {'name': name, ...codec.encodeRectArgs(rect)});
  @override
  PdfTask<void> radioGroup(
    String name,
    List<({String value, PdfRect rect})> options, {
    String? selected,
  }) => _op('radioGroup', {
    'name': name,
    'options': options
        .map(
          (o) => <String, Object?>{
            'value': o.value,
            ...codec.encodeRectArgs(o.rect),
          },
        )
        .toList(),
    'selected': selected,
  });
  @override
  PdfTask<void> fieldKeystroke(String script) =>
      _op('fieldKeystroke', {'script': script});
  @override
  PdfTask<void> fieldFormat(String script) =>
      _op('fieldFormat', {'script': script});
  @override
  PdfTask<void> fieldValidate(String script) =>
      _op('fieldValidate', {'script': script});
  @override
  PdfTask<void> fieldCalculate(String script) =>
      _op('fieldCalculate', {'script': script});
  @override
  PdfTask<void> linkUrl(String url) => _op('linkUrl', {'url': url});
  @override
  PdfTask<void> linkPage(int targetPage) =>
      _op('linkPage', {'targetPage': targetPage});
  @override
  PdfTask<void> footnote(String refMark, String noteText) =>
      _op('footnote', {'refMark': refMark, 'noteText': noteText});
  @override
  PdfTask<void> inline(String text) => _op('inline', {'text': text});
  @override
  PdfTask<void> inlineBold(String text) => _op('inlineBold', {'text': text});
  @override
  PdfTask<void> inlineItalic(String text) =>
      _op('inlineItalic', {'text': text});
  @override
  PdfTask<void> inlineColor(String text, PdfColor color) =>
      _op('inlineColor', {'text': text, ...codec.encodeColorArgs(color)});
  @override
  PdfTask<void> textInRect(
    PdfRect rect,
    String text, {
    PdfCellAlign align = PdfCellAlign.left,
  }) => _op('textInRect', {
    ...codec.encodeRectArgs(rect),
    'text': text,
    'align': align.index,
  });
  @override
  PdfTask<void> rect(PdfRect rect) =>
      _op('rect', {...codec.encodeRectArgs(rect)});
  @override
  PdfTask<void> filledRect(PdfRect rect, PdfColor color) => _op('filledRect', {
    ...codec.encodeRectArgs(rect),
    ...codec.encodeColorArgs(color),
  });
  @override
  PdfTask<void> line(double x1, double y1, double x2, double y2) =>
      _op('line', {'x1': x1, 'y1': y1, 'x2': x2, 'y2': y2});
  @override
  PdfTask<void> strokeRect(
    PdfRect rect, {
    double width = 1,
    PdfColor color = PdfColor.black,
    List<double>? dash,
    double dashPhase = 0,
  }) => _op('strokeRect', {
    ...codec.encodeRectArgs(rect),
    'lineWidth': width,
    ...codec.encodeColorArgs(color),
    'dash': dash,
    'dashPhase': dashPhase,
  });
  @override
  PdfTask<void> strokeLine(
    double x1,
    double y1,
    double x2,
    double y2, {
    double width = 1,
    PdfColor color = PdfColor.black,
    List<double>? dash,
    double dashPhase = 0,
  }) => _op('strokeLine', {
    'x1': x1,
    'y1': y1,
    'x2': x2,
    'y2': y2,
    'lineWidth': width,
    ...codec.encodeColorArgs(color),
    'dash': dash,
    'dashPhase': dashPhase,
  });
  @override
  PdfTask<void> highlight({PdfColor color = PdfColor.yellow}) =>
      _op('highlight', codec.encodeColorArgs(color));
  @override
  PdfTask<void> underline({PdfColor color = PdfColor.black}) =>
      _op('underline', codec.encodeColorArgs(color));
  @override
  PdfTask<void> strikeout({PdfColor color = PdfColor.black}) =>
      _op('strikeout', codec.encodeColorArgs(color));
  @override
  PdfTask<void> squiggly({PdfColor color = PdfColor.black}) =>
      _op('squiggly', codec.encodeColorArgs(color));
  @override
  PdfTask<void> stickyNote(String text, {double? x, double? y}) {
    if ((x == null) != (y == null)) {
      throw ArgumentError('stickyNote takes x and y together or neither');
    }
    return _op('stickyNote', {'text': text, 'x': x, 'y': y});
  }

  @override
  PdfTask<void> stamp(PdfStampType type) =>
      _op('stamp', {'stampType': stampWireName(type)});
  @override
  PdfTask<void> freeText(PdfRect rect, String text) =>
      _op('freeText', {...codec.encodeRectArgs(rect), 'text': text});
  @override
  PdfTask<void> watermarkConfidential() => _op('watermarkConfidential');
  @override
  PdfTask<void> watermarkDraft() => _op('watermarkDraft');
  @override
  PdfTask<void> linkNamed(String destination) =>
      _op('linkNamed', {'destination': destination});
  @override
  PdfTask<void> linkJavaScript(String script) =>
      _op('linkJavaScript', {'script': script});
  @override
  PdfTask<void> onOpen(String script) => _op('onOpen', {'script': script});
  @override
  PdfTask<void> onClose(String script) => _op('onClose', {'script': script});
  @override
  PdfTask<void> imageArtifact(DataSource imageData, PdfRect rect) =>
      _bridge._exec(
        EngineOp.builderPageOp,
        {
          'handleId': _handleId,
          'pageOp': 'imageArtifact',
          'sourceLength': imageData.length,
          ...codec.encodeRectArgs(rect),
        },
        sources: [imageData],
      );
  @override
  PdfTask<void> table(
    List<PdfTableColumn> columns,
    List<List<String>> rows, {
    bool hasHeader = true,
  }) {
    for (var i = 0; i < rows.length; i++) {
      if (rows[i].length != columns.length) {
        throw ArgumentError(
          'table row $i has ${rows[i].length} cells but the table has '
          '${columns.length} columns',
        );
      }
    }
    return _op('table', {
      'headers': columns.map((c) => c.header).toList(),
      'widths': columns.map((c) => c.width ?? -1.0).toList(),
      'aligns': columns.map((c) => c.align.index).toList(),
      'cells': [for (final row in rows) ...row],
      'columnCount': columns.length,
      'hasHeader': hasHeader,
    });
  }

  @override
  PdfTask<void> streamingTable(
    List<PdfTableColumn> columns,
    List<List<PdfTableCell>> rows, {
    bool repeatHeader = true,
    PdfTableSizing sizing = PdfTableSizing.fixed,
    int sampleRows = 50,
    double minColumnWidth = 20,
    double maxColumnWidth = 400,
    int maxRowSpan = 1,
  }) {
    for (final column in columns) {
      if (column.width == null) {
        throw ArgumentError(
          'streamingTable requires an explicit width on every column; '
          '"${column.header}" has none',
        );
      }
    }
    return _op('streamingTable', {
      'headers': columns.map((c) => c.header).toList(),
      'widths': columns.map((c) => c.width!).toList(),
      'aligns': columns.map((c) => c.align.index).toList(),
      'cells': [for (final row in rows) ...row.map((c) => c.text)],
      'rowSpans': [for (final row in rows) ...row.map((c) => c.rowSpan)],
      'columnCount': columns.length,
      'repeatHeader': repeatHeader,
      'mode': sizing.wire,
      'sampleRows': sampleRows,
      'minColWidth': minColumnWidth,
      'maxColWidth': maxColumnWidth,
      'maxRowSpan': maxRowSpan,
    });
  }

  @override
  PdfTask<void> columns(int columnCount, double gapPt, String text) => _op(
    'columns',
    {'columnCount': columnCount, 'gapPt': gapPt, 'text': text},
  );
  @override
  PdfTask<void> newline() => _op('newline');
  @override
  PdfTask<void> newPageSameSize() => _op('newPageSameSize');
  @override
  PdfTask<void> done() =>
      _bridge._exec(EngineOp.builderPageDone, {'handleId': _handleId});
}
