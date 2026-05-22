import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_sink.dart';
import 'package:pdf_manipulator/src/core/pdf_source.dart';
import 'package:pdf_manipulator/src/platform/pdf_platform.dart';

/// Batch PDF editor — parse once, mutate many times, save once.
///
/// Created via [Pdf.edit]:
///
/// ```dart
/// final editor = await Pdf.edit(source);
/// await editor.setTitle('Updated');
/// await editor.rotatePage(0, degrees: 90);
/// await editor.save(outputSink);
/// editor.dispose();
/// ```
class PdfEditor {
  PdfPlatform? _platform;
  final PdfEditorHandle _handle;
  bool _disposed = false;

  /// Internal constructor — use [Pdf.edit] instead.
  PdfEditor.internal(this._platform, this._handle);

  void _check() {
    if (_disposed) throw StateError('This PdfEditor has been disposed');
  }

  // ── Properties ─────────────────────────────────────────────────

  Future<int> get pageCount { _check(); return _handle.pageCount; }
  Future<String> get version { _check(); return _handle.version; }
  Future<bool> get isModified { _check(); return _handle.isModified; }

  // ── Metadata ───────────────────────────────────────────────────

  Future<String> getTitle() { _check(); return _handle.getTitle(); }
  Future<void> setTitle(String value) { _check(); return _handle.setTitle(value); }
  Future<String> getAuthor() { _check(); return _handle.getAuthor(); }
  Future<void> setAuthor(String value) { _check(); return _handle.setAuthor(value); }
  Future<String> getSubject() { _check(); return _handle.getSubject(); }
  Future<void> setSubject(String value) { _check(); return _handle.setSubject(value); }
  Future<String> getKeywords() { _check(); return _handle.getKeywords(); }
  Future<void> setKeywords(String value) { _check(); return _handle.setKeywords(value); }

  // ── Pages ──────────────────────────────────────────────────────

  Future<void> rotatePage(int pageIndex, {required int degrees}) {
    _check(); return _handle.rotatePage(pageIndex, degrees: degrees);
  }
  Future<void> rotateAllPages({required int degrees}) {
    _check(); return _handle.rotateAllPages(degrees: degrees);
  }
  Future<PdfRect> getPageMediaBox(int pageIndex) {
    _check(); return _handle.getPageMediaBox(pageIndex);
  }
  Future<void> deletePage(int pageIndex) { _check(); return _handle.deletePage(pageIndex); }
  Future<void> movePage({required int from, required int to}) {
    _check(); return _handle.movePage(from: from, to: to);
  }
  Future<void> extractPages(List<int> pages, PdfSink output) {
    _check(); return _handle.extractPages(pages, output);
  }
  Future<void> mergeFrom(PdfSource otherPdf) { _check(); return _handle.mergeFrom(otherPdf); }

  // ── Optimization ───────────────────────────────────────────────

  Future<int> optimizeImages({int quality = 75}) {
    _check(); return _handle.optimizeImages(quality: quality);
  }
  Future<int> unembedStandardFonts() { _check(); return _handle.unembedStandardFonts(); }

  // ── Watermark + stamps ─────────────────────────────────────────

  Future<void> addWatermark(int pageIndex, String text,
      {double fontSize = 48, double rotation = 45, double opacity = 0.3,
       double r = 0.5, double g = 0.5, double b = 0.5}) {
    _check();
    return _handle.addWatermark(pageIndex, text,
        fontSize: fontSize, rotation: rotation, opacity: opacity,
        r: r, g: g, b: b);
  }

  Future<void> addWatermarkPositioned(int pageIndex, String text, {
    required double x, required double y,
    required double width, required double height,
    double fontSize = 48, String? fontName,
    double rotation = 45, double opacity = 0.3,
    double r = 0.5, double g = 0.5, double b = 0.5,
  }) {
    _check();
    return _handle.addWatermarkPositioned(pageIndex, text,
        x: x, y: y, width: width, height: height,
        fontSize: fontSize, fontName: fontName,
        rotation: rotation, opacity: opacity, r: r, g: g, b: b);
  }

  Future<void> addStamp(int pageIndex, {
    required int stampType,
    String? customName,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addStamp(pageIndex,
        stampType: stampType, customName: customName,
        x: x, y: y, width: width, height: height, opacity: opacity);
  }

  Future<void> addImageStamp(int pageIndex, Uint8List imageBytes, {
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addImageStamp(pageIndex, imageBytes,
        x: x, y: y, width: width, height: height, opacity: opacity);
  }

  // ── Content ────────────────────────────────────────────────────

  Future<void> embedFile(String name, Uint8List data) {
    _check(); return _handle.embedFile(name, data);
  }
  Future<void> eraseRegions(int pageIndex, List<PdfRect> regions) {
    _check(); return _handle.eraseRegions(pageIndex, regions);
  }
  Future<void> flattenForms() { _check(); return _handle.flattenForms(); }
  Future<void> flattenAllAnnotations() { _check(); return _handle.flattenAllAnnotations(); }
  Future<void> applyAllRedactions() { _check(); return _handle.applyAllRedactions(); }
  Future<void> setFormFieldValue(String fieldName, String value) {
    _check(); return _handle.setFormFieldValue(fieldName, value);
  }
  Future<void> cropMargins({double left = 0, double right = 0,
      double top = 0, double bottom = 0}) {
    _check();
    return _handle.cropMargins(left: left, right: right, top: top, bottom: bottom);
  }
  Future<void> convertToPdfA({int level = 1}) {
    _check(); return _handle.convertToPdfA(level: level);
  }
  Future<void> resizeImage(int pageIndex, String imageName,
      {required double width, required double height}) {
    _check();
    return _handle.resizeImage(pageIndex, imageName, width: width, height: height);
  }

  // ── Save ───────────────────────────────────────────────────────

  Future<void> save(PdfSink output) { _check(); return _handle.save(output); }

  Future<void> saveWithOptions(PdfSink output, {bool compress = true,
      bool garbageCollect = true, bool linearize = false}) {
    _check();
    return _handle.saveWithOptions(output, compress: compress,
        garbageCollect: garbageCollect, linearize: linearize);
  }

  Future<void> saveEncrypted(PdfSink output, {required String ownerPassword,
      String userPassword = ''}) {
    _check();
    return _handle.saveEncrypted(output, ownerPassword: ownerPassword,
        userPassword: userPassword);
  }

  Future<void> saveEncryptedFull(PdfSink output, {
      required String ownerPassword, String userPassword = '',
      int algorithm = 3,
      bool allowPrint = true, bool allowPrintHq = true,
      bool allowModify = true, bool allowCopy = true,
      bool allowAnnotate = true, bool allowFillForms = true,
      bool allowAccessibility = true, bool allowAssemble = true,
  }) {
    _check();
    return _handle.saveEncryptedFull(output,
        ownerPassword: ownerPassword, userPassword: userPassword,
        algorithm: algorithm,
        allowPrint: allowPrint, allowPrintHq: allowPrintHq,
        allowModify: allowModify, allowCopy: allowCopy,
        allowAnnotate: allowAnnotate, allowFillForms: allowFillForms,
        allowAccessibility: allowAccessibility, allowAssemble: allowAssemble);
  }

  // ── Lifecycle ──────────────────────────────────────────────────

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _handle.dispose();
    _platform?.dispose();
    _platform = null;
  }
}
