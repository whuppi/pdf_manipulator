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
        'modDate': _dateArg(metadata.modificationDate),
      });

  @override
  PdfTask<BridgePageBuilderHandle> addPage(PdfPageSize size) => _exec(
    EngineOp.builderAddPage,
    _pageSizeArgs(size),
  ).map((_) => _SharedPageBuilderHandle(_bridge, _handleId, scope));

  @override
  PdfTask<BridgePageBuilderHandle> addImagePage(
    DataSource imageData, {
    PdfImagePage page = const PdfImagePage(),
  }) => _bridge
      ._exec(
        EngineOp.builderAddImagePage,
        {
          'handleId': _handleId,
          'sourceLength': imageData.length,
          if (page.size case PdfImagePageSizeFixed(:final size)) ...{
            'width': size.width,
            'height': size.height,
          },
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

  /// Runs [first], then each of [then] that is not null, one engine op
  /// after the other, in one cancellable task.
  PdfTask<void> _sequence(
    PdfTask<void> Function() first,
    List<PdfTask<void> Function()?> then,
  ) => PdfTask.group((hook) async {
    await hook.guard(first());
    for (final step in then) {
      if (step != null) await hook.guard(step());
    }
  });

  /// The ops that anchor [link] and [markup] on the text just written.
  List<PdfTask<void> Function()?> _anchors(PdfLink? link, PdfMarkup? markup) =>
      [
        if (link != null) () => _link(link),
        if (markup != null) () => _markup(markup),
      ];

  PdfTask<void> _link(PdfLink link) => switch (link) {
    PdfLinkUrl(:final url) => _op(PageOp.linkUrl, {'url': url}),
    PdfLinkPage(:final page) => _op(PageOp.linkPage, {'targetPage': page}),
    PdfLinkNamed(:final destination) => _op(PageOp.linkNamed, {
      'destination': destination,
    }),
    PdfLinkJavaScript(:final script) => _op(PageOp.linkJavaScript, {
      'script': script,
    }),
  };

  PdfTask<void> _markup(PdfMarkup markup) {
    final op = switch (markup.kind) {
      PdfMarkupKind.highlight => PageOp.highlight,
      PdfMarkupKind.underline => PageOp.underline,
      PdfMarkupKind.strikeout => PageOp.strikeout,
      PdfMarkupKind.squiggly => PageOp.squiggly,
    };
    final ink =
        markup.color ??
        (markup.kind == PdfMarkupKind.highlight
            ? PdfColor.yellow
            : PdfColor.black);
    return _op(op, codec.encodeColorArgs(ink));
  }

  /// The ops that attach the scripts a field carries, after the field.
  List<PdfTask<void> Function()?> _scripts(PdfFieldScripts scripts) => [
    if (scripts.keystroke case final script?)
      () => _op(PageOp.fieldKeystroke, {'script': script}),
    if (scripts.format case final script?)
      () => _op(PageOp.fieldFormat, {'script': script}),
    if (scripts.validate case final script?)
      () => _op(PageOp.fieldValidate, {'script': script}),
    if (scripts.calculate case final script?)
      () => _op(PageOp.fieldCalculate, {'script': script}),
  ];

  @override
  PdfTask<void> setFont(String name, double size) =>
      _op(PageOp.font, {'name': name, 'size': size});
  @override
  PdfTask<void> moveTo(PdfPoint point) =>
      _op(PageOp.at, {'x': point.x, 'y': point.y});
  @override
  PdfTask<void> text(
    String text, {
    PdfTextStyle? style,
    PdfLink? link,
    PdfMarkup? markup,
  }) => _sequence(
    () => _op(PageOp.text, {'text': text, ..._styleArgs(style)}),
    _anchors(link, markup),
  );
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
  PdfTask<void> image(
    DataSource imageData,
    PdfRect bounds, {
    String? altText,
  }) => _bridge._exec(
    EngineOp.builderPageOp,
    {
      'handleId': _handleId,
      'pageOp': (altText == null ? PageOp.imageArtifact : PageOp.image).wire,
      'sourceLength': imageData.length,
      ...codec.encodeRectArgs(bounds),
      if (altText != null) 'altText': altText,
    },
    sources: [imageData],
    scope: _scope,
  );
  @override
  PdfTask<void> watermark(String text) => _op(PageOp.watermark, {'text': text});
  @override
  PdfTask<void> textField(
    String name,
    PdfRect bounds, {
    String? value,
    required PdfFieldScripts scripts,
  }) => _sequence(
    () => _op(PageOp.textField, {
      'name': name,
      ...codec.encodeRectArgs(bounds),
      'defaultValue': value,
    }),
    _scripts(scripts),
  );
  @override
  PdfTask<void> checkbox(
    String name,
    PdfRect bounds, {
    required bool isChecked,
    required PdfFieldScripts scripts,
  }) => _sequence(
    () => _op(PageOp.checkbox, {
      'name': name,
      ...codec.encodeRectArgs(bounds),
      'checked': isChecked,
    }),
    _scripts(scripts),
  );
  @override
  PdfTask<void> comboBox(
    String name,
    PdfRect bounds,
    List<String> options, {
    String? selected,
    required PdfFieldScripts scripts,
  }) => _sequence(
    () => _op(PageOp.comboBox, {
      'name': name,
      ...codec.encodeRectArgs(bounds),
      'options': options,
      'selected': selected,
    }),
    _scripts(scripts),
  );
  @override
  PdfTask<void> pushButton(String name, PdfRect bounds, String caption) => _op(
    PageOp.pushButton,
    {'name': name, ...codec.encodeRectArgs(bounds), 'caption': caption},
  );
  @override
  PdfTask<void> signatureField(String name, PdfRect bounds) => _op(
    PageOp.signatureField,
    {'name': name, ...codec.encodeRectArgs(bounds)},
  );
  @override
  PdfTask<void> radioGroup(
    String name,
    List<PdfRadioOption> options, {
    String? selected,
    required PdfFieldScripts scripts,
  }) => _sequence(
    () => _op(PageOp.radioGroup, {
      'name': name,
      'options': options
          .map(
            (o) => <String, Object?>{
              'value': o.value,
              ...codec.encodeRectArgs(o.bounds),
            },
          )
          .toList(),
      'selected': selected,
    }),
    _scripts(scripts),
  );
  @override
  PdfTask<void> footnote(String mark, String note) =>
      _op(PageOp.footnote, {'refMark': mark, 'noteText': note});
  @override
  PdfTask<void> appendText(
    String text, {
    PdfTextStyle? style,
    PdfLink? link,
    PdfMarkup? markup,
  }) => _sequence(
    () => _op(PageOp.inline, {'text': text, ..._styleArgs(style)}),
    _anchors(link, markup),
  );
  @override
  PdfTask<void> textIn(
    PdfRect bounds,
    String text, {
    PdfAlignment align = PdfAlignment.left,
  }) => _op(PageOp.textIn, {
    ...codec.encodeRectArgs(bounds),
    'text': text,
    'align': codec.putEnum(align),
  });
  @override
  PdfTask<void> rect(PdfRect rect, {PdfColor? fill, PdfStroke? stroke}) {
    return PdfTask.group((hook) async {
      if (fill != null) {
        await hook.guard(
          _op(PageOp.filledRect, {
            ...codec.encodeRectArgs(rect),
            ...codec.encodeColorArgs(fill),
          }),
        );
      }
      if (stroke != null) {
        await hook.guard(
          _op(PageOp.strokeRect, {
            ...codec.encodeRectArgs(rect),
            ..._strokeArgs(stroke),
          }),
        );
      }
    });
  }

  @override
  PdfTask<void> line(
    PdfPoint from,
    PdfPoint to, {
    PdfStroke stroke = const PdfStroke(),
  }) => _op(PageOp.strokeLine, {
    'x1': from.x,
    'y1': from.y,
    'x2': to.x,
    'y2': to.y,
    ..._strokeArgs(stroke),
  });

  @override
  PdfTask<void> stickyNote(String text, {PdfPoint? at}) =>
      _op(PageOp.stickyNote, {'text': text, 'x': at?.x, 'y': at?.y});

  @override
  PdfTask<void> stamp(PdfStamp stamp) =>
      _op(PageOp.stamp, {'stampType': codec.putEnum(stamp)});
  @override
  PdfTask<void> freeText(PdfRect bounds, String text) =>
      _op(PageOp.freeText, {...codec.encodeRectArgs(bounds), 'text': text});
  @override
  PdfTask<void> table(
    List<PdfTableColumn> columns,
    List<List<PdfTableCell>> rows, {
    PdfTableOptions options = const PdfTableOptions(),
  }) {
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
  PdfTask<void> columns(int count, double gap, String text) =>
      _op(PageOp.columns, {'columnCount': count, 'gapPt': gap, 'text': text});
  @override
  PdfTask<void> newline() => _op(PageOp.newline);
  @override
  PdfTask<void> onOpen(String script) => _op(PageOp.onOpen, {'script': script});
  @override
  PdfTask<void> onClose(String script) =>
      _op(PageOp.onClose, {'script': script});
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
