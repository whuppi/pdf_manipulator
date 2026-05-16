import 'dart:typed_data';

import 'package:pdf_manipulator/src/platform/pdf_platform.dart';

/// Create PDFs from scratch — text, images, forms, tables.
///
/// Created via `pdf.createBuilder()` where `pdf` is a [Pdf] instance.
///
/// ```dart
/// final pdf = Pdf();
/// final builder = await pdf.createBuilder();
/// await builder.setTitle('My Document');
/// final page = await builder.addA4Page();
/// await page.font('Helvetica', 14);
/// await page.text('Hello, world!');
/// await page.done();
/// final bytes = await builder.build();
/// await builder.dispose();
/// ```
class PdfBuilder {
  final PdfBuilderHandle _handle;
  PdfBuilder(this._handle);

  Future<void> setTitle(String value) => _handle.setTitle(value);
  Future<void> setAuthor(String value) => _handle.setAuthor(value);
  Future<void> setSubject(String value) => _handle.setSubject(value);
  Future<void> setKeywords(String value) => _handle.setKeywords(value);

  Future<PdfPageBuilder> addA4Page() async {
    final handle = await _handle.addA4Page();
    return PdfPageBuilder(handle);
  }

  Future<PdfPageBuilder> addLetterPage() async {
    final handle = await _handle.addLetterPage();
    return PdfPageBuilder(handle);
  }

  Future<PdfPageBuilder> addPage({required double width, required double height}) async {
    final handle = await _handle.addPage(width: width, height: height);
    return PdfPageBuilder(handle);
  }

  Future<Uint8List> build() => _handle.build();
  Future<Uint8List> buildEncrypted({required String ownerPassword,
      String userPassword = ''}) =>
      _handle.buildEncrypted(ownerPassword: ownerPassword,
          userPassword: userPassword);

  Future<void> dispose() => _handle.dispose();
}

/// Page content builder — add text, images, forms, watermarks.
class PdfPageBuilder {
  final PdfPageBuilderHandle _handle;
  PdfPageBuilder(this._handle);

  Future<void> font(String name, double size) => _handle.font(name, size);
  Future<void> at(double x, double y) => _handle.at(x, y);
  Future<void> text(String text) => _handle.text(text);
  Future<void> heading(int level, String text) => _handle.heading(level, text);
  Future<void> paragraph(String text) => _handle.paragraph(text);
  Future<void> space(double points) => _handle.space(points);
  Future<void> horizontalRule() => _handle.horizontalRule();
  Future<void> image(Uint8List imageBytes, double x, double y,
      double width, double height, {String altText = ''}) =>
      _handle.image(imageBytes, x, y, width, height, altText: altText);
  Future<void> watermark(String text) => _handle.watermark(text);

  Future<void> textField(String name, double x, double y, double w, double h,
      {String? defaultValue}) =>
      _handle.textField(name, x, y, w, h, defaultValue: defaultValue);
  Future<void> checkbox(String name, double x, double y, double w, double h,
      {bool checked = false}) =>
      _handle.checkbox(name, x, y, w, h, checked: checked);
  Future<void> comboBox(String name, double x, double y, double w, double h,
      List<String> options, {String? selected}) =>
      _handle.comboBox(name, x, y, w, h, options, selected: selected);
  Future<void> pushButton(String name, double x, double y, double w, double h,
      String caption) =>
      _handle.pushButton(name, x, y, w, h, caption);
  Future<void> signatureField(String name, double x, double y, double w,
      double h) =>
      _handle.signatureField(name, x, y, w, h);
  Future<void> radioGroup(String name, List<String> values,
      List<double> xs, List<double> ys, List<double> ws, List<double> hs,
      {String? selected}) =>
      _handle.radioGroup(name, values, xs, ys, ws, hs, selected: selected);
  Future<void> fieldKeystroke(String script) => _handle.fieldKeystroke(script);
  Future<void> fieldFormat(String script) => _handle.fieldFormat(script);
  Future<void> fieldValidate(String script) => _handle.fieldValidate(script);
  Future<void> fieldCalculate(String script) => _handle.fieldCalculate(script);
  Future<void> linkUrl(String url) => _handle.linkUrl(url);
  Future<void> linkPage(int targetPage) => _handle.linkPage(targetPage);
  Future<void> footnote(String refMark, String noteText) =>
      _handle.footnote(refMark, noteText);
  Future<void> columns(int columnCount, double gapPt, String text) =>
      _handle.columns(columnCount, gapPt, text);
  Future<void> newline() => _handle.newline();
  Future<void> newPageSameSize() => _handle.newPageSameSize();

  Future<void> done() => _handle.done();
}
