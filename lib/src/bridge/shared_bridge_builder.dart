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
      _bridge._exec(op, {'handleId': _handleId, ...args}, scope: scope);

  @override
  PdfTask<void> setMetadata(PdfMetadata metadata) =>
      _exec(EngineOp.builderSetMetadata, {
        'title': metadata.title,
        'author': metadata.author,
        'subject': metadata.subject,
        'keywords': metadata.keywords,
        'creator': metadata.creator,
        'producer': metadata.producer,
        'creationDate': _dateArg(metadata.creationDate),
        'modDate': _dateArg(metadata.modDate),
      });

  @override
  PdfTask<BridgePageBuilderHandle> addPage(PdfPageSize size) => _exec(
    EngineOp.builderAddPage,
    _pageSizeArgs(size),
  ).map((_) => _SharedPageBuilderHandle(_bridge, _handleId, scope));

  @override
  PdfTask<BridgePageBuilderHandle> addImagePage(
    DataSource imageData, {
    PdfImagePageOptions page = const PdfImagePageOptions(),
  }) => _bridge
      ._exec(
        EngineOp.builderAddImagePage,
        {
          'handleId': _handleId,
          'sourceLength': imageData.length,
          if (page.size != null) 'width': page.size!.width,
          if (page.size != null) 'height': page.size!.height,
          'fit': page.fit.wire,
          'margin': page.margin,
        },
        sources: [imageData],
        scope: scope,
      )
      .map((_) => _SharedPageBuilderHandle(_bridge, _handleId, scope));

  @override
  PdfTask<void> save(DataSink output, {bool deflate = true}) => _bridge._exec(
    EngineOp.builderSave,
    {'handleId': _handleId, 'deflate': deflate},
    sinks: [output],
    scope: scope,
  );
  @override
  Future<void> dispose() =>
      _bridge._exec(EngineOp.builderDispose, {'handleId': _handleId});
}

/// The wire arguments of a page size: the engine's own names for the two
/// it knows, the dimensions for the rest.
Map<String, Object?> _pageSizeArgs(PdfPageSize size) => switch (size) {
  PdfPageSizeA4() => {'pageType': 'a4'},
  PdfPageSizeLetter() => {'pageType': 'letter'},
  _ => {'width': size.width, 'height': size.height},
};

/// The wire argument of a date: the PDF date string the editor's dates
/// travel as.
String? _dateArg(DateTime? value) =>
    value == null ? null : codec.putDate(value);

// ══════════════════════════════════════════════════════════════════
// Page builder handle
// ══════════════════════════════════════════════════════════════════

class _SharedPageBuilderHandle extends BridgePageBuilderHandle {
  _SharedPageBuilderHandle(this._bridge, this._handleId, this._scope);
  final SharedBridge _bridge;
  final int _handleId;
  final CancelHook? _scope;

  PdfTask<void> _op(PageOp op, [Map<String, Object?> extra = const {}]) =>
      _bridge._exec(EngineOp.builderPageOp, {
        'handleId': _handleId,
        'pageOp': op.wire,
        ...extra,
      }, scope: _scope);

  @override
  PdfTask<void> font(String name, double size) =>
      _op(PageOp.font, {'name': name, 'size': size});
  @override
  PdfTask<void> at(double x, double y) => _op(PageOp.at, {'x': x, 'y': y});
  @override
  PdfTask<void> text(String text, {PdfTextStyle? style}) =>
      _op(PageOp.text, {'text': text, ..._styleArgs(style)});
  @override
  PdfTask<void> heading(int level, String text) =>
      _op(PageOp.heading, {'level': level, 'text': text});
  @override
  PdfTask<void> paragraph(String text) => _op(PageOp.paragraph, {'text': text});
  @override
  PdfTask<void> space(double points) => _op(PageOp.space, {'points': points});
  @override
  PdfTask<void> horizontalRule() => _op(PageOp.horizontalRule);
  @override
  PdfTask<void> image(DataSource imageData, PdfRect rect, {String? altText}) =>
      _bridge._exec(
        EngineOp.builderPageOp,
        {
          'handleId': _handleId,
          'pageOp':
              (altText == null ? PageOp.imageArtifact : PageOp.image).wire,
          'sourceLength': imageData.length,
          ...codec.encodeRectArgs(rect),
          if (altText != null) 'altText': altText,
        },
        sources: [imageData],
        scope: _scope,
      );
  @override
  PdfTask<void> watermark(String text) => _op(PageOp.watermark, {'text': text});
  @override
  PdfTask<void> textField(String name, PdfRect rect, {String? defaultValue}) =>
      _op(PageOp.textField, {
        'name': name,
        ...codec.encodeRectArgs(rect),
        'defaultValue': defaultValue,
      });
  @override
  PdfTask<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _op(PageOp.checkbox, {
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
  }) => _op(PageOp.comboBox, {
    'name': name,
    ...codec.encodeRectArgs(rect),
    'options': options,
    'selected': selected,
  });
  @override
  PdfTask<void> pushButton(String name, PdfRect rect, String caption) => _op(
    PageOp.pushButton,
    {'name': name, ...codec.encodeRectArgs(rect), 'caption': caption},
  );
  @override
  PdfTask<void> signatureField(String name, PdfRect rect) =>
      _op(PageOp.signatureField, {'name': name, ...codec.encodeRectArgs(rect)});
  @override
  PdfTask<void> radioGroup(
    String name,
    List<({String value, PdfRect rect})> options, {
    String? selected,
  }) => _op(PageOp.radioGroup, {
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
      _op(PageOp.fieldKeystroke, {'script': script});
  @override
  PdfTask<void> fieldFormat(String script) =>
      _op(PageOp.fieldFormat, {'script': script});
  @override
  PdfTask<void> fieldValidate(String script) =>
      _op(PageOp.fieldValidate, {'script': script});
  @override
  PdfTask<void> fieldCalculate(String script) =>
      _op(PageOp.fieldCalculate, {'script': script});
  @override
  PdfTask<void> linkUrl(String url) => _op(PageOp.linkUrl, {'url': url});
  @override
  PdfTask<void> linkPage(int targetPage) =>
      _op(PageOp.linkPage, {'targetPage': targetPage});
  @override
  PdfTask<void> footnote(String refMark, String noteText) =>
      _op(PageOp.footnote, {'refMark': refMark, 'noteText': noteText});
  @override
  PdfTask<void> inline(String text, {PdfTextStyle? style}) =>
      _op(PageOp.inline, {'text': text, ..._styleArgs(style)});
  @override
  PdfTask<void> textInRect(
    PdfRect rect,
    String text, {
    PdfAlignment align = PdfAlignment.left,
  }) => _op(PageOp.textInRect, {
    ...codec.encodeRectArgs(rect),
    'text': text,
    'align': codec.putEnum(align),
  });
  @override
  PdfTask<void> rect(PdfRect rect, {PdfColor? fill, PdfStroke? stroke}) {
    // No fill and no stroke means a stroked rectangle with the default
    // stroke; a fill alone leaves the outline off.
    final outline = stroke ?? (fill == null ? const PdfStroke() : null);
    return PdfTask.group((hook) async {
      if (fill != null) {
        await hook.guard(
          _op(PageOp.filledRect, {
            ...codec.encodeRectArgs(rect),
            ...codec.encodeColorArgs(fill),
          }),
        );
      }
      if (outline != null) {
        await hook.guard(
          _op(PageOp.strokeRect, {
            ...codec.encodeRectArgs(rect),
            ..._strokeArgs(outline),
          }),
        );
      }
    });
  }

  @override
  PdfTask<void> line(
    (double, double) from,
    (double, double) to, {
    PdfStroke stroke = const PdfStroke(),
  }) => _op(PageOp.strokeLine, {
    'x1': from.$1,
    'y1': from.$2,
    'x2': to.$1,
    'y2': to.$2,
    ..._strokeArgs(stroke),
  });
  @override
  PdfTask<void> markup(PdfMarkupKind kind, {PdfColor? color}) {
    final op = switch (kind) {
      PdfMarkupKind.highlight => PageOp.highlight,
      PdfMarkupKind.underline => PageOp.underline,
      PdfMarkupKind.strikeout => PageOp.strikeout,
      PdfMarkupKind.squiggly => PageOp.squiggly,
    };
    final ink =
        color ??
        (kind == PdfMarkupKind.highlight ? PdfColor.yellow : PdfColor.black);
    return _op(op, codec.encodeColorArgs(ink));
  }

  @override
  PdfTask<void> stickyNote(String text, {(double, double)? at}) =>
      _op(PageOp.stickyNote, {'text': text, 'x': at?.$1, 'y': at?.$2});

  @override
  PdfTask<void> stamp(PdfStampType type) =>
      _op(PageOp.stamp, {'stampType': codec.putEnum(type)});
  @override
  PdfTask<void> freeText(PdfRect rect, String text) =>
      _op(PageOp.freeText, {...codec.encodeRectArgs(rect), 'text': text});
  @override
  PdfTask<void> linkNamed(String destination) =>
      _op(PageOp.linkNamed, {'destination': destination});
  @override
  PdfTask<void> linkJavaScript(String script) =>
      _op(PageOp.linkJavaScript, {'script': script});
  @override
  PdfTask<void> onOpen(String script) => _op(PageOp.onOpen, {'script': script});
  @override
  PdfTask<void> onClose(String script) =>
      _op(PageOp.onClose, {'script': script});
  @override
  PdfTask<void> table(
    List<PdfTableColumn> columns,
    List<List<PdfTableCell>> rows, {
    PdfTableOptions options = const PdfTableOptions(),
  }) {
    if (options.sizing == PdfTableSizing.fixed) {
      for (final column in columns) {
        if (column.width == null) {
          throw PdfInvalidArgument(
            'table needs an explicit width on every column; '
            '"${column.header}" has none — give every column a width, or '
            'use sizing: PdfTableSizing.sample or .autoAll',
          );
        }
      }
    }
    for (final row in rows) {
      if (row.length != columns.length) {
        throw PdfInvalidArgument(
          'table rows need one cell per column: a row has '
          '${row.length} cells for ${columns.length} columns',
        );
      }
      for (final cell in row) {
        if (cell.rowSpan < 1 || cell.rowSpan > options.maxRowSpan) {
          throw PdfInvalidArgument(
            'table cell "${cell.text}" spans ${cell.rowSpan} rows; '
            'rowSpan must be 1 to maxRowSpan (${options.maxRowSpan})',
          );
        }
      }
    }
    // The engine measures one sample of rows: `autoAll` sends every row
    // as the sample, so every row is measured.
    final autoAll = options.sizing == PdfTableSizing.autoAll;
    final mode = autoAll ? PdfTableSizing.sample : options.sizing;
    return _op(PageOp.streamingTable, {
      'headers': columns.map((c) => c.header).toList(),
      // -1: no width; the engine keeps its own default for the column.
      'widths': columns.map((c) => c.width ?? -1.0).toList(),
      'aligns': codec.putEnums(columns.map((c) => c.align)),
      'cells': [for (final row in rows) ...row.map((c) => c.text)],
      'rowSpans': [for (final row in rows) ...row.map((c) => c.rowSpan)],
      'columnCount': columns.length,
      'repeatHeader': options.repeatHeader,
      'hasHeader': options.header,
      'mode': mode.wire,
      'sampleRows': autoAll
          ? (rows.isEmpty ? 1 : rows.length)
          : options.sampleRows,
      'minColWidth': options.minColumnWidth,
      'maxColWidth': options.maxColumnWidth,
      'maxRowSpan': options.maxRowSpan,
    });
  }

  @override
  PdfTask<void> columns(int columnCount, double gapPt, String text) => _op(
    PageOp.columns,
    {'columnCount': columnCount, 'gapPt': gapPt, 'text': text},
  );
  @override
  PdfTask<void> newline() => _op(PageOp.newline);
  @override
  PdfTask<void> newPageSameSize() => _op(PageOp.newPageSameSize);
  @override
  PdfTask<void> done() => _bridge._exec(EngineOp.builderPageDone, {
    'handleId': _handleId,
  }, scope: _scope);
}

/// The wire arguments of a text style: only the fields that override the
/// page's own font, size and colour travel.
Map<String, Object?> _styleArgs(PdfTextStyle? style) {
  if (style == null) return const {};
  final color = style.color;
  return {
    if (style.font != null) 'font': style.font,
    if (style.size != null) 'size': style.size,
    if (style.bold) 'bold': true,
    if (style.italic) 'italic': true,
    if (color != null) ...codec.encodeColorArgs(color),
  };
}

/// The wire arguments of a stroke: the line width, its colour and its
/// dash pattern.
Map<String, Object?> _strokeArgs(PdfStroke stroke) => {
  'lineWidth': stroke.width,
  ...codec.encodeColorArgs(stroke.color),
  'dash': stroke.dash,
  'dashPhase': stroke.dashPhase,
};
