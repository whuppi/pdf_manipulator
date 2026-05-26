// PdfBuilder — create PDFs from scratch.


import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

class PdfBuilder {
  PdfBuilder.internal(PdfBridge _, this._handle);

  final BridgeBuilderHandle _handle;
  bool _disposed = false;

  void _check() {
    if (_disposed) throw StateError('This PdfBuilder has been disposed');
  }

  // ── Metadata ──

  Future<void> setTitle(String value) { _check(); return _handle.setTitle(value); }
  Future<void> setAuthor(String value) { _check(); return _handle.setAuthor(value); }
  Future<void> setSubject(String value) { _check(); return _handle.setSubject(value); }
  Future<void> setKeywords(String value) { _check(); return _handle.setKeywords(value); }

  // ── Pages ──

  Future<PdfPageBuilder> addA4Page() async {
    _check();
    return PdfPageBuilder(await _handle.addA4Page());
  }

  Future<PdfPageBuilder> addLetterPage() async {
    _check();
    return PdfPageBuilder(await _handle.addLetterPage());
  }

  Future<PdfPageBuilder> addPage(
      {required double width, required double height}) async {
    _check();
    return PdfPageBuilder(
        await _handle.addPage(width: width, height: height));
  }


  Future<void> save(DataSink output) {
    _check();
    return _handle.save(output);
  }

  // ── Lifecycle ──

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _handle.dispose();
  }
}

/// Page content builder — add text, images, forms, watermarks.
class PdfPageBuilder {
  PdfPageBuilder(this._handle);

  final BridgePageBuilderHandle _handle;

  Future<void> font(String name, double size) => _handle.font(name, size);
  Future<void> at(double x, double y) => _handle.at(x, y);
  Future<void> text(String text) => _handle.text(text);
  Future<void> heading(int level, String text) => _handle.heading(level, text);
  Future<void> paragraph(String text) => _handle.paragraph(text);
  Future<void> space(double points) => _handle.space(points);
  Future<void> horizontalRule() => _handle.horizontalRule();
  Future<void> image(DataSource imageData, PdfRect rect,
          {String altText = ''}) =>
      _handle.image(imageData, rect, altText: altText);
  Future<void> watermark(String text) => _handle.watermark(text);


  Future<void> textField(String name, PdfRect rect,
          {String? defaultValue}) =>
      _handle.textField(name, rect, defaultValue: defaultValue);

  Future<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _handle.checkbox(name, rect, checked: checked);

  Future<void> comboBox(String name, PdfRect rect, List<String> options,
          {String? selected}) =>
      _handle.comboBox(name, rect, options, selected: selected);

  Future<void> pushButton(String name, PdfRect rect, String caption) =>
      _handle.pushButton(name, rect, caption);

  Future<void> signatureField(String name, PdfRect rect) =>
      _handle.signatureField(name, rect);

  // ── Radio group ──

  Future<void> radioGroup(String name,
          List<({String value, PdfRect rect})> options,
          {String? selected}) =>
      _handle.radioGroup(name, options, selected: selected);

  // ── Field scripts ──

  Future<void> fieldKeystroke(String script) => _handle.fieldKeystroke(script);
  Future<void> fieldFormat(String script) => _handle.fieldFormat(script);
  Future<void> fieldValidate(String script) => _handle.fieldValidate(script);
  Future<void> fieldCalculate(String script) => _handle.fieldCalculate(script);

  // ── Links ──

  Future<void> linkUrl(String url) => _handle.linkUrl(url);
  Future<void> linkPage(int targetPage) => _handle.linkPage(targetPage);

  // ── Layout ──

  Future<void> footnote(String refMark, String noteText) =>
      _handle.footnote(refMark, noteText);
  Future<void> columns(int columnCount, double gapPt, String text) =>
      _handle.columns(columnCount, gapPt, text);
  Future<void> newline() => _handle.newline();
  Future<void> newPageSameSize() => _handle.newPageSameSize();
  Future<void> done() => _handle.done();
}
