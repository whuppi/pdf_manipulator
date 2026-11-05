// PdfBuilder — create PDFs from scratch.
//
// build() → addPage(s) with content → save → dispose.
// Construction only — no reading or querying existing PDFs.
// Use PdfDocument for read queries, PdfEditor for mutations on existing PDFs.
//
// PdfBuilder is the view a scope body gets; PdfBuilderHandle is the one
// `Pdf.build` returns, and the only one that saves and disposes. A page ends
// when the next is added or when the document is saved.

part of 'pdf.dart';

/// Creates PDFs from scratch — what a [Pdf.buildTo] body adds pages to.
/// The library saves the document when the body returns.
base class PdfBuilder {
  /// Internal constructor — use [Pdf.build] to create instances.
  @internal
  PdfBuilder.internal(this._handle);

  final BridgeBuilderHandle _handle;
  bool _disposed = false;

  void _check() {
    if (_disposed) throw StateError('This PdfBuilder has been disposed');
  }

  // ── Metadata ──

  /// Sets the document metadata. A null field is left out of the document.
  PdfTask<void> setMetadata(PdfMetadata metadata) {
    _check();
    return _handle.setMetadata(metadata);
  }

  // ── Pages ──

  /// Adds a page of [size] and returns its content builder. The previous
  /// page ends.
  PdfTask<PdfPageBuilder> addPage([PdfPageSize size = const PdfPageSize.a4()]) {
    _check();
    return _handle.addPage(size).map((handle) => PdfPageBuilder(handle));
  }

  /// Adds a page holding [image] and returns its content builder. The
  /// previous page ends.
  ///
  /// The page is the image's own size, one point per pixel, unless [page]
  /// names a size; [PdfImagePage.fit] says how the image fills it.
  PdfTask<PdfPageBuilder> addImagePage(
    DataSource image, {
    PdfImagePage page = const PdfImagePage(),
  }) {
    _check();
    return _handle
        .addImagePage(image, page: page)
        .map((handle) => PdfPageBuilder(handle));
  }
}

/// A [PdfBuilder] you own: the handle [Pdf.build] returns.
///
/// Call [dispose] when you are done, on every path including errors. For one
/// document, use [Pdf.buildTo], which builds, saves and always closes. A
/// handle you forget is released when the garbage collector frees it, and in
/// debug mode the leak is logged with where it was opened; that is a
/// backstop, not a plan.
final class PdfBuilderHandle extends PdfBuilder implements _Handle {
  /// Internal constructor — use [Pdf.build] to create instances.
  @internal
  PdfBuilderHandle.internal(super.handle) : super.internal() {
    HandleBackstop.attach(this, 'PdfBuilderHandle', _handle.dispose);
  }

  @override
  void _scopeTo(CancelHook hook) => _handle.scope = hook;

  /// Writes the document to [output]. Page content and font streams are
  /// Flate-compressed unless [deflate] is false, which keeps them
  /// readable as plain text for debugging. The sink stays the caller's: it
  /// is aborted when the save fails or is cancelled, and never closed.
  PdfTask<void> save(DataSink output, {bool deflate = true}) {
    _check();
    return _handle.save(output, deflate: deflate);
  }

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
///
/// A link, a markup, a field script or a footnote's anchor is an argument of
/// the call that writes what it attaches to; the builder keeps no "last
/// written" target.
class PdfPageBuilder {
  /// Internal constructor — [PdfBuilder.addPage] creates instances.
  @internal
  PdfPageBuilder(this._handle);

  final BridgePageBuilderHandle _handle;

  /// Sets the current font by [name] and [size] in points.
  PdfTask<void> setFont(String name, double size) =>
      _handle.setFont(name, size);

  /// Moves the cursor to [point], in points.
  PdfTask<void> moveTo(PdfPoint point) => _handle.moveTo(point);

  /// Writes [text] at the current cursor position, in the page's font
  /// unless [style] overrides it. [link] makes the text a link and [markup]
  /// draws a markup annotation over it.
  PdfTask<void> text(
    String text, {
    PdfTextStyle? style,
    PdfLink? link,
    PdfMarkup? markup,
  }) => _handle.text(text, style: style, link: link, markup: markup);

  /// Writes [text] at the cursor without moving to a new line, in the page's
  /// font unless [style] overrides it. [link] and [markup] are as in [text].
  PdfTask<void> appendText(
    String text, {
    PdfTextStyle? style,
    PdfLink? link,
    PdfMarkup? markup,
  }) => _handle.appendText(text, style: style, link: link, markup: markup);

  /// Writes a heading at the given [level] (1-6).
  PdfTask<void> heading(int level, String text) => _handle.heading(level, text);

  /// Writes a paragraph of body text.
  PdfTask<void> paragraph(String text) => _handle.paragraph(text);

  /// Inserts vertical space of [points] height.
  PdfTask<void> space(double points) => _handle.space(points);

  /// Inserts a line break at the current cursor position.
  PdfTask<void> newline() => _handle.newline();

  /// Draws a horizontal rule across the page.
  PdfTask<void> horizontalRule() => _handle.horizontalRule();

  /// Wraps [text] inside [bounds], aligned per [align].
  PdfTask<void> textIn(
    PdfRect bounds,
    String text, {
    PdfAlignment align = PdfAlignment.left,
  }) => _handle.textIn(bounds, text, align: align);

  /// Places an image within [bounds]. A null [altText] places it as an
  /// artifact, which assistive technology ignores.
  PdfTask<void> image(DataSource image, PdfRect bounds, {String? altText}) =>
      _handle.image(image, bounds, altText: altText);

  /// Draws [rect]: [fill] inside it, [stroke] around it. A null [fill]
  /// leaves it unfilled and a null [stroke] unstroked.
  PdfTask<void> rect(
    PdfRect rect, {
    PdfColor? fill,
    PdfStroke? stroke = const PdfStroke(),
  }) => _handle.rect(rect, fill: fill, stroke: stroke);

  /// Draws a straight line from [from] to [to].
  PdfTask<void> line(
    PdfPoint from,
    PdfPoint to, {
    PdfStroke stroke = const PdfStroke(),
  }) => _handle.line(from, to, stroke: stroke);

  /// Adds a diagonal watermark text overlay on the page.
  PdfTask<void> watermark(String text) => _handle.watermark(text);

  /// Adds a stamp annotation of [stamp] at the cursor.
  PdfTask<void> stamp(PdfStamp stamp) => _handle.stamp(stamp);

  /// Adds a sticky note with [text] at [at], or at the cursor when [at]
  /// is null.
  PdfTask<void> stickyNote(String text, {PdfPoint? at}) =>
      _handle.stickyNote(text, at: at);

  /// Adds a FreeText annotation showing [text] inside [bounds].
  PdfTask<void> freeText(PdfRect bounds, String text) =>
      _handle.freeText(bounds, text);

  /// Adds a footnote with a reference [mark] and the [note] text.
  PdfTask<void> footnote(String mark, String note) =>
      _handle.footnote(mark, note);

  /// Lays out [text] in [count] columns with [gap] points between them.
  PdfTask<void> columns(int count, double gap, String text) =>
      _handle.columns(count, gap, text);

  /// Lays out [rows] as a table under [columns]; every row one cell per
  /// column, and every [PdfTableCell.rowSpan] within the options' max row
  /// span: any other table throws [ArgumentError] before anything is sent.
  ///
  /// [PdfTableSizing.fixed] needs a width on every column; the sizing
  /// modes that measure ([PdfTableSizing.sample] and
  /// [PdfTableSizing.autoAll]) size the columns from their content.
  PdfTask<void> table(
    List<PdfTableColumn> columns,
    List<List<PdfTableCell>> rows, {
    PdfTableOptions options = const PdfTableOptions(),
  }) {
    if (options.sizing == PdfTableSizing.fixed) {
      for (final column in columns) {
        if (column.width == null) {
          throw ArgumentError.value(
            columns,
            'columns',
            'a fixed table needs a width on every column; "${column.header}" '
                'has none — give every column a width, or use sizing: '
                'PdfTableSizing.sample or .autoAll',
          );
        }
      }
    }
    for (final row in rows) {
      if (row.length != columns.length) {
        throw ArgumentError.value(
          rows,
          'rows',
          'a row needs one cell per column: a row has ${row.length} cells '
              'for ${columns.length} columns',
        );
      }
      for (final cell in row) {
        if (cell.rowSpan < 1 || cell.rowSpan > options.maxRowSpan) {
          throw ArgumentError.value(
            rows,
            'rows',
            'cell "${cell.text}" spans ${cell.rowSpan} rows; rowSpan must be '
                '1 to maxRowSpan (${options.maxRowSpan})',
          );
        }
      }
    }
    return _handle.table(columns, rows, options: options);
  }

  // ── Form fields ──

  /// Adds an interactive text input field in [bounds], holding [value]. Its
  /// [scripts] run on the field.
  PdfTask<void> textField(
    String name,
    PdfRect bounds, {
    String? value,
    PdfFieldScripts scripts = const PdfFieldScripts(),
  }) => _handle.textField(name, bounds, value: value, scripts: scripts);

  /// Adds a checkbox form field in [bounds]. Its [scripts] run on the field.
  PdfTask<void> checkbox(
    String name,
    PdfRect bounds, {
    bool isChecked = false,
    PdfFieldScripts scripts = const PdfFieldScripts(),
  }) => _handle.checkbox(name, bounds, isChecked: isChecked, scripts: scripts);

  /// Adds a dropdown combo box form field in [bounds] offering [options].
  /// Its [scripts] run on the field.
  PdfTask<void> comboBox(
    String name,
    PdfRect bounds,
    List<String> options, {
    String? selected,
    PdfFieldScripts scripts = const PdfFieldScripts(),
  }) => _handle.comboBox(
    name,
    bounds,
    options,
    selected: selected,
    scripts: scripts,
  );

  /// Adds a radio button group with the given [options]. Its [scripts] run
  /// on the group.
  PdfTask<void> radioGroup(
    String name,
    List<PdfRadioOption> options, {
    String? selected,
    PdfFieldScripts scripts = const PdfFieldScripts(),
  }) => _handle.radioGroup(name, options, selected: selected, scripts: scripts);

  /// Adds a push button in [bounds] with the given [caption].
  PdfTask<void> pushButton(String name, PdfRect bounds, String caption) =>
      _handle.pushButton(name, bounds, caption);

  /// Adds a digital signature field in [bounds].
  PdfTask<void> signatureField(String name, PdfRect bounds) =>
      _handle.signatureField(name, bounds);

  // ── Page actions ──

  /// Runs [script] when the page opens.
  PdfTask<void> onOpen(String script) => _handle.onOpen(script);

  /// Runs [script] when the page closes.
  PdfTask<void> onClose(String script) => _handle.onClose(script);
}
