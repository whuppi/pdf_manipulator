// PdfBuilder — create PDFs from scratch.
//
// build() → addPage(s) with content → save → dispose.
// Construction only — no reading or querying existing PDFs.
// Use PdfDoc for read queries, PdfEditor for mutations on existing PDFs.

import 'package:pdf_manipulator/src/ops/pdf.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// Creates PDFs from scratch — add pages with content, then save.
class PdfBuilder {
  /// Internal constructor — use [Pdf.build] to create instances.
  PdfBuilder.internal(PdfBridge _, this._handle);

  final BridgeBuilderHandle _handle;
  bool _disposed = false;

  void _check() {
    if (_disposed) throw StateError('This PdfBuilder has been disposed');
  }

  // ── Metadata ──

  /// Sets the document title metadata.
  Future<void> setTitle(String value) { _check(); return _handle.setTitle(value); }

  /// Sets the document author metadata.
  Future<void> setAuthor(String value) { _check(); return _handle.setAuthor(value); }

  /// Sets the document subject metadata.
  Future<void> setSubject(String value) { _check(); return _handle.setSubject(value); }

  /// Sets the document keywords metadata.
  Future<void> setKeywords(String value) { _check(); return _handle.setKeywords(value); }

  // ── Pages ──

  /// Adds a new A4-sized page and returns its content builder.
  Future<PdfPageBuilder> addA4Page() async {
    _check();
    return PdfPageBuilder(await _handle.addA4Page());
  }

  /// Adds a new US Letter-sized page and returns its content builder.
  Future<PdfPageBuilder> addLetterPage() async {
    _check();
    return PdfPageBuilder(await _handle.addLetterPage());
  }

  /// Adds a page with custom dimensions (in points) and returns its builder.
  Future<PdfPageBuilder> addPage(
      {required double width, required double height}) async {
    _check();
    return PdfPageBuilder(
        await _handle.addPage(width: width, height: height));
  }

  // ── Save ──

  /// Writes the constructed PDF to [output].
  Future<void> save(DataSink output) {
    _check();
    return _handle.save(output);
  }

  // ── Lifecycle ──

  /// Releases builder resources. Idempotent.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _handle.dispose();
  }
}

/// Page content builder — add text, images, forms, watermarks.
class PdfPageBuilder {
  /// Creates a page builder wrapping the given handle.
  PdfPageBuilder(this._handle);

  final BridgePageBuilderHandle _handle;

  /// Sets the current font by [name] and [size] in points.
  Future<void> font(String name, double size) => _handle.font(name, size);

  /// Moves the cursor to absolute position ([x], [y]) in points.
  Future<void> at(double x, double y) => _handle.at(x, y);

  /// Writes inline text at the current cursor position.
  Future<void> text(String text) => _handle.text(text);

  /// Writes a heading at the given [level] (1-6).
  Future<void> heading(int level, String text) => _handle.heading(level, text);

  /// Writes a paragraph of body text.
  Future<void> paragraph(String text) => _handle.paragraph(text);

  /// Inserts vertical space of [points] height.
  Future<void> space(double points) => _handle.space(points);

  /// Draws a horizontal rule across the page.
  Future<void> horizontalRule() => _handle.horizontalRule();

  /// Places an image within [rect], with optional [altText] for accessibility.
  Future<void> image(DataSource imageData, PdfRect rect,
          {String altText = ''}) =>
      _handle.image(imageData, rect, altText: altText);

  /// Adds a diagonal watermark text overlay on the page.
  Future<void> watermark(String text) => _handle.watermark(text);

  // ── Form fields ──

  /// Adds an interactive text input field at [rect].
  Future<void> textField(String name, PdfRect rect,
          {String? defaultValue}) =>
      _handle.textField(name, rect, defaultValue: defaultValue);

  /// Adds a checkbox form field at [rect].
  Future<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _handle.checkbox(name, rect, checked: checked);

  /// Adds a dropdown combo box form field at [rect].
  Future<void> comboBox(String name, PdfRect rect, List<String> options,
          {String? selected}) =>
      _handle.comboBox(name, rect, options, selected: selected);

  /// Adds a push button at [rect] with the given [caption].
  Future<void> pushButton(String name, PdfRect rect, String caption) =>
      _handle.pushButton(name, rect, caption);

  /// Adds a digital signature field at [rect].
  Future<void> signatureField(String name, PdfRect rect) =>
      _handle.signatureField(name, rect);

  // ── Radio group ──

  /// Adds a radio button group with the given [options].
  Future<void> radioGroup(String name,
          List<({String value, PdfRect rect})> options,
          {String? selected}) =>
      _handle.radioGroup(name, options, selected: selected);

  // ── Field scripts ──

  /// Sets a JavaScript keystroke action on the current form field.
  Future<void> fieldKeystroke(String script) => _handle.fieldKeystroke(script);

  /// Sets a JavaScript format action on the current form field.
  Future<void> fieldFormat(String script) => _handle.fieldFormat(script);

  /// Sets a JavaScript validate action on the current form field.
  Future<void> fieldValidate(String script) => _handle.fieldValidate(script);

  /// Sets a JavaScript calculate action on the current form field.
  Future<void> fieldCalculate(String script) => _handle.fieldCalculate(script);

  // ── Links ──

  /// Adds a URL hyperlink annotation.
  Future<void> linkUrl(String url) => _handle.linkUrl(url);

  /// Adds an internal link to [targetPage] (0-based).
  Future<void> linkPage(int targetPage) => _handle.linkPage(targetPage);

  // ── Layout ──

  /// Adds a footnote with a reference mark and note text.
  Future<void> footnote(String refMark, String noteText) =>
      _handle.footnote(refMark, noteText);

  /// Lays out [text] in [columnCount] columns with [gapPt] gap between them.
  Future<void> columns(int columnCount, double gapPt, String text) =>
      _handle.columns(columnCount, gapPt, text);

  /// Inserts a line break at the current cursor position.
  Future<void> newline() => _handle.newline();

  /// Adds a new page with the same dimensions as the current page.
  Future<void> newPageSameSize() => _handle.newPageSameSize();

  /// Finalizes the page content — call after all content is added.
  Future<void> done() => _handle.done();
}
