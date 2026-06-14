// PdfBuilder — create PDFs from scratch.
//
// build() → addPage(s) with content → save → dispose.
// Construction only — no reading or querying existing PDFs.
// Use PdfDoc for read queries, PdfEditor for mutations on existing PDFs.

import 'package:pdf_manipulator/src/ops/pdf.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
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
  PdfTask<void> setTitle(String value) {
    _check();
    return _handle.setTitle(value);
  }

  /// Sets the document author metadata.
  PdfTask<void> setAuthor(String value) {
    _check();
    return _handle.setAuthor(value);
  }

  /// Sets the document subject metadata.
  PdfTask<void> setSubject(String value) {
    _check();
    return _handle.setSubject(value);
  }

  /// Sets the document keywords metadata.
  PdfTask<void> setKeywords(String value) {
    _check();
    return _handle.setKeywords(value);
  }

  // ── Pages ──

  /// Adds a new A4-sized page and returns its content builder.
  PdfTask<PdfPageBuilder> addA4Page() {
    _check();
    return _handle.addA4Page().map((handle) => PdfPageBuilder(handle));
  }

  /// Adds a new US Letter-sized page and returns its content builder.
  PdfTask<PdfPageBuilder> addLetterPage() {
    _check();
    return _handle.addLetterPage().map((handle) => PdfPageBuilder(handle));
  }

  /// Adds a page with custom dimensions (in points) and returns its builder.
  PdfTask<PdfPageBuilder> addPage({
    required double width,
    required double height,
  }) {
    _check();
    return _handle
        .addPage(width: width, height: height)
        .map((handle) => PdfPageBuilder(handle));
  }

  // ── Save ──

  /// Writes the constructed PDF to [output].
  PdfTask<void> save(DataSink output) {
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
  PdfTask<void> font(String name, double size) => _handle.font(name, size);

  /// Moves the cursor to absolute position ([x], [y]) in points.
  PdfTask<void> at(double x, double y) => _handle.at(x, y);

  /// Writes inline text at the current cursor position.
  PdfTask<void> text(String text) => _handle.text(text);

  /// Writes a heading at the given [level] (1-6).
  PdfTask<void> heading(int level, String text) => _handle.heading(level, text);

  /// Writes a paragraph of body text.
  PdfTask<void> paragraph(String text) => _handle.paragraph(text);

  /// Inserts vertical space of [points] height.
  PdfTask<void> space(double points) => _handle.space(points);

  /// Draws a horizontal rule across the page.
  PdfTask<void> horizontalRule() => _handle.horizontalRule();

  /// Places an image within [rect], with optional [altText] for accessibility.
  PdfTask<void> image(
    DataSource imageData,
    PdfRect rect, {
    String altText = '',
  }) => _handle.image(imageData, rect, altText: altText);

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
  /// form field. A page with no field yet ignores the call.
  PdfTask<void> fieldKeystroke(String script) => _handle.fieldKeystroke(script);

  /// Sets a JavaScript format action on the most-recently-added
  /// form field. A page with no field yet ignores the call.
  PdfTask<void> fieldFormat(String script) => _handle.fieldFormat(script);

  /// Sets a JavaScript validate action on the most-recently-added
  /// form field. A page with no field yet ignores the call.
  PdfTask<void> fieldValidate(String script) => _handle.fieldValidate(script);

  /// Sets a JavaScript calculate action on the most-recently-added
  /// form field. A page with no field yet ignores the call.
  PdfTask<void> fieldCalculate(String script) => _handle.fieldCalculate(script);

  // ── Links ──

  /// Adds a URL hyperlink anchored on the last-written text. A page
  /// with no text yet ignores the call.
  PdfTask<void> linkUrl(String url) => _handle.linkUrl(url);

  /// Adds an internal link to [targetPage] (0-based), anchored on
  /// the last-written text. A page with no text yet ignores the call.
  PdfTask<void> linkPage(int targetPage) => _handle.linkPage(targetPage);

  // ── Layout ──

  /// Adds a footnote with a reference mark and note text.
  PdfTask<void> footnote(String refMark, String noteText) =>
      _handle.footnote(refMark, noteText);

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
