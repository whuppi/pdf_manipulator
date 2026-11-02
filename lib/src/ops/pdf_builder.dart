// PdfBuilder — create PDFs from scratch.
//
// build() → addPage(s) with content → save → dispose.
// Construction only — no reading or querying existing PDFs.
// Use PdfDoc for read queries, PdfEditor for mutations on existing PDFs.

part of 'pdf.dart';

/// Creates PDFs from scratch — add pages with content, then save.
class PdfBuilder implements _Handle {
  /// Internal constructor — use [Pdf.build] to create instances.
  PdfBuilder.internal(PdfBridge _, this._handle) {
    HandleBackstop.attach(this, 'PdfBuilder', _handle.dispose);
  }

  final BridgeBuilderHandle _handle;
  bool _disposed = false;

  @override
  void _scopeTo(CancelHook hook) => _handle.scope = hook;

  void _check() {
    if (_disposed) {
      throw const PdfDisposed('This PdfBuilder has been disposed');
    }
  }

  // ── Metadata ──

  /// Sets the document metadata. A null field is left out of the document.
  PdfTask<void> setMetadata(PdfMetadata metadata) {
    _check();
    return _handle.setMetadata(metadata);
  }

  // ── Pages ──

  /// Adds a page of [size] and returns its content builder.
  PdfTask<PdfPageBuilder> addPage([PdfPageSize size = const PdfPageSize.a4()]) {
    _check();
    return _handle.addPage(size).map((handle) => PdfPageBuilder(handle));
  }

  /// Adds a page holding [image] and returns its content builder.
  ///
  /// The page is the image's own size, one point per pixel, unless [page]
  /// names a size; [PdfImagePageOptions.fit] says how the image fills it.
  PdfTask<PdfPageBuilder> addImagePage(
    DataSource image, {
    PdfImagePageOptions page = const PdfImagePageOptions(),
  }) {
    _check();
    return _handle
        .addImagePage(image, page: page)
        .map((handle) => PdfPageBuilder(handle));
  }

  // ── Save ──

  /// Writes the document to [output]. Page content and font streams are
  /// Flate-compressed unless [deflate] is false, which keeps them
  /// readable as plain text for debugging.
  PdfTask<void> save(DataSink output, {bool deflate = true}) {
    _check();
    return _handle.save(output, deflate: deflate);
  }

  // ── Lifecycle ──

  /// Releases builder resources. Idempotent.
  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    HandleBackstop.detach(this);
    await _handle.dispose();
  }
}

/// Page content builder — add text, images, forms, watermarks.
class PdfPageBuilder {
  /// Creates a page builder wrapping the given handle.
  PdfPageBuilder(this._handle);

  final BridgePageBuilderHandle _handle;

  /// Sets the current font by [name] and [size] in points.
  PdfTask<void> font(String name, double size) => _handle.font(name, size);

  /// Moves the cursor to absolute position ([x], [y]) in points.
  PdfTask<void> at(double x, double y) => _handle.at(x, y);

  /// Writes [text] at the current cursor position, in the page's font
  /// unless [style] overrides it.
  PdfTask<void> text(String text, {PdfTextStyle? style}) =>
      _handle.text(text, style: style);

  /// Writes a heading at the given [level] (1-6).
  PdfTask<void> heading(int level, String text) => _handle.heading(level, text);

  /// Writes a paragraph of body text.
  PdfTask<void> paragraph(String text) => _handle.paragraph(text);

  /// Inserts vertical space of [points] height.
  PdfTask<void> space(double points) => _handle.space(points);

  /// Draws a horizontal rule across the page.
  PdfTask<void> horizontalRule() => _handle.horizontalRule();

  /// Places an image within [rect]. A null [altText] places it as an
  /// artifact, which assistive technology ignores.
  PdfTask<void> image(DataSource imageData, PdfRect rect, {String? altText}) =>
      _handle.image(imageData, rect, altText: altText);

  /// Adds a diagonal watermark text overlay on the page.
  PdfTask<void> watermark(String text) => _handle.watermark(text);

  // ── Form fields ──

  /// Adds an interactive text input field at [rect].
  PdfTask<void> textField(String name, PdfRect rect, {String? defaultValue}) =>
      _handle.textField(name, rect, defaultValue: defaultValue);

  /// Adds a checkbox form field at [rect].
  PdfTask<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _handle.checkbox(name, rect, checked: checked);

  /// Adds a dropdown combo box form field at [rect].
  PdfTask<void> comboBox(
    String name,
    PdfRect rect,
    List<String> options, {
    String? selected,
  }) => _handle.comboBox(name, rect, options, selected: selected);

  /// Adds a push button at [rect] with the given [caption].
  PdfTask<void> pushButton(String name, PdfRect rect, String caption) =>
      _handle.pushButton(name, rect, caption);

  /// Adds a digital signature field at [rect].
  PdfTask<void> signatureField(String name, PdfRect rect) =>
      _handle.signatureField(name, rect);

  // ── Radio group ──

  /// Adds a radio button group with the given [options].
  PdfTask<void> radioGroup(
    String name,
    List<({String value, PdfRect rect})> options, {
    String? selected,
  }) => _handle.radioGroup(name, options, selected: selected);

  // ── Field scripts ──

  /// Sets a JavaScript keystroke action on the most-recently-added
  /// form field. Throws [PdfInvalidArgument] when this page has no
  /// form field yet.
  PdfTask<void> fieldKeystroke(String script) => _handle.fieldKeystroke(script);

  /// Sets a JavaScript format action on the most-recently-added
  /// form field. Throws [PdfInvalidArgument] when this page has no
  /// form field yet.
  PdfTask<void> fieldFormat(String script) => _handle.fieldFormat(script);

  /// Sets a JavaScript validate action on the most-recently-added
  /// form field. Throws [PdfInvalidArgument] when this page has no
  /// form field yet.
  PdfTask<void> fieldValidate(String script) => _handle.fieldValidate(script);

  /// Sets a JavaScript calculate action on the most-recently-added
  /// form field. Throws [PdfInvalidArgument] when this page has no
  /// form field yet.
  PdfTask<void> fieldCalculate(String script) => _handle.fieldCalculate(script);

  // ── Links ──

  /// Adds a URL hyperlink anchored on the last-written text. Throws
  /// [PdfInvalidArgument] when this page has no text yet.
  PdfTask<void> linkUrl(String url) => _handle.linkUrl(url);

  /// Adds an internal link to [targetPage] (0-based), anchored on
  /// the last-written text. Throws [PdfInvalidArgument] when this
  /// page has no text yet.
  PdfTask<void> linkPage(int targetPage) => _handle.linkPage(targetPage);

  // ── Layout ──

  /// Adds a footnote with a reference mark and note text.
  PdfTask<void> footnote(String refMark, String noteText) =>
      _handle.footnote(refMark, noteText);

  /// Writes [text] at the cursor without moving to a new line, in the
  /// page's font unless [style] overrides it.
  PdfTask<void> inline(String text, {PdfTextStyle? style}) =>
      _handle.inline(text, style: style);

  /// Wraps [text] inside [rect], aligned per [align].
  PdfTask<void> textInRect(
    PdfRect rect,
    String text, {
    PdfAlignment align = PdfAlignment.left,
  }) => _handle.textInRect(rect, text, align: align);

  /// Draws [rect]: [fill] inside it, [stroke] around it. A null [fill]
  /// leaves it unfilled, a null [stroke] leaves it unstroked — unless
  /// both are null, which strokes it with the default stroke.
  PdfTask<void> rect(PdfRect rect, {PdfColor? fill, PdfStroke? stroke}) =>
      _handle.rect(rect, fill: fill, stroke: stroke);

  /// Draws a straight line from [from] to [to].
  PdfTask<void> line(
    (double, double) from,
    (double, double) to, {
    PdfStroke stroke = const PdfStroke(),
  }) => _handle.line(from, to, stroke: stroke);

  /// Adds a markup annotation of [kind] over the last-written text, in
  /// [color] or in the kind's own colour when [color] is null.
  PdfTask<void> markup(PdfMarkupKind kind, {PdfColor? color}) =>
      _handle.markup(kind, color: color);

  /// Adds a sticky note with [text] at [at], or at the cursor when [at]
  /// is null.
  PdfTask<void> stickyNote(String text, {(double, double)? at}) =>
      _handle.stickyNote(text, at: at);

  /// Adds a stamp annotation of [type] at the cursor.
  PdfTask<void> stamp(PdfStampType type) => _handle.stamp(type);

  /// Adds a FreeText annotation showing [text] inside [rect].
  PdfTask<void> freeText(PdfRect rect, String text) =>
      _handle.freeText(rect, text);

  /// Links the last-written text to the named [destination].
  PdfTask<void> linkNamed(String destination) => _handle.linkNamed(destination);

  /// Links the last-written text to a JavaScript [script] action.
  PdfTask<void> linkJavaScript(String script) => _handle.linkJavaScript(script);

  /// Runs [script] when the page opens.
  PdfTask<void> onOpen(String script) => _handle.onOpen(script);

  /// Runs [script] when the page closes.
  PdfTask<void> onClose(String script) => _handle.onClose(script);

  /// Lays out [rows] as a table under [columns]; every row one cell per
  /// column, and every [PdfTableCell.rowSpan] within the options' max row
  /// span: any other table throws [PdfInvalidArgument] before anything is
  /// sent.
  ///
  /// [PdfTableSizing.fixed] needs a width on every column; the sizing
  /// modes that measure ([PdfTableSizing.sample] and
  /// [PdfTableSizing.autoAll]) size the columns from their content.
  PdfTask<void> table(
    List<PdfTableColumn> columns,
    List<List<PdfTableCell>> rows, {
    PdfTableOptions options = const PdfTableOptions(),
  }) => _handle.table(columns, rows, options: options);

  /// Lays out [text] in [columnCount] columns with [gapPt] gap between them.
  PdfTask<void> columns(int columnCount, double gapPt, String text) =>
      _handle.columns(columnCount, gapPt, text);

  /// Inserts a line break at the current cursor position.
  PdfTask<void> newline() => _handle.newline();

  /// Adds a new page with the same dimensions as the current page.
  PdfTask<void> newPageSameSize() => _handle.newPageSameSize();

  /// Finalizes the page content — call after all content is added.
  PdfTask<void> done() => _handle.done();
}
