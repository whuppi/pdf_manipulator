// PdfEditor — batch editing. Parse once, mutate many, save once.
// From API_GOLD §7.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_sink.dart';
import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/api/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/api/types/pdf_params.dart';
import 'package:pdf_manipulator/src/bridge/bridge.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';

class PdfEditor {
  PdfEditor.internal(PdfBridge _, this._handle);

  final BridgeEditorHandle _handle;
  bool _disposed = false;

  void _check() {
    if (_disposed) throw StateError('This PdfEditor has been disposed');
  }

  // ── Properties ──

  Future<int> get pageCount {
    _check();
    return _handle.pageCount;
  }

  Future<String> get version {
    _check();
    return _handle.version;
  }

  // ── Metadata ──

  Future<String> getTitle() { _check(); return _handle.getTitle(); }
  Future<void> setTitle(String value) { _check(); return _handle.setTitle(value); }
  Future<String> getAuthor() { _check(); return _handle.getAuthor(); }
  Future<void> setAuthor(String value) { _check(); return _handle.setAuthor(value); }
  Future<String> getSubject() { _check(); return _handle.getSubject(); }
  Future<void> setSubject(String value) { _check(); return _handle.setSubject(value); }
  Future<String> getKeywords() { _check(); return _handle.getKeywords(); }
  Future<void> setKeywords(String value) { _check(); return _handle.setKeywords(value); }

  // ── Pages ──

  Future<void> rotatePage(int page, {required int degrees}) {
    _check();
    return _handle.rotatePage(page, degrees: degrees);
  }

  Future<void> rotateAllPages({required int degrees}) {
    _check();
    return _handle.rotateAllPages(degrees: degrees);
  }

  Future<PdfRect> getPageMediaBox(int page) {
    _check();
    return _handle.getPageMediaBox(page);
  }

  Future<void> deletePage(int page) {
    _check();
    return _handle.deletePage(page);
  }

  Future<void> movePage({required int from, required int to}) {
    _check();
    return _handle.movePage(from: from, to: to);
  }

  Future<void> extractPages(List<int> pages, PdfSink output) {
    _check();
    return _handle.extractPages(pages, output);
  }

  Future<void> mergeFrom(PdfSource otherPdf) {
    _check();
    return _handle.mergeFrom(otherPdf);
  }

  // ── Optimization ──

  Future<int> optimizeImages({int quality = 75}) {
    _check();
    return _handle.optimizeImages(quality: quality);
  }

  Future<int> unembedStandardFonts() {
    _check();
    return _handle.unembedStandardFonts();
  }

  // ── Watermark + stamps ──

  Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  }) {
    _check();
    return _handle.addWatermark(page, text, style: style, position: position);
  }

  Future<void> addStamp(int page, {
    required PdfStampType type,
    required PdfRect rect,
    String? customName,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addStamp(page,
        type: type, rect: rect, customName: customName, opacity: opacity);
  }

  Future<void> addImageStamp(int page, Uint8List imageBytes, {
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addImageStamp(page, imageBytes,
        rect: rect, opacity: opacity);
  }

  // ── Content ──

  Future<void> embedFile(String name, Uint8List data) {
    _check();
    return _handle.embedFile(name, data);
  }

  Future<void> eraseRegions(int page, List<PdfRect> regions) {
    _check();
    return _handle.eraseRegions(page, regions);
  }

  Future<void> flattenForms() { _check(); return _handle.flattenForms(); }
  Future<void> flattenAllAnnotations() { _check(); return _handle.flattenAllAnnotations(); }

  Future<void> setFormFieldValue(String fieldName, String value) {
    _check();
    return _handle.setFormFieldValue(fieldName, value);
  }

  Future<void> cropMargins({
    double left = 0, double right = 0,
    double top = 0, double bottom = 0,
  }) {
    _check();
    return _handle.cropMargins(
        left: left, right: right, top: top, bottom: bottom);
  }

  Future<void> convertToPdfA({int level = 1}) {
    _check();
    return _handle.convertToPdfA(level: level);
  }

  Future<void> resizeImage(int page, String imageName, {
    required double width, required double height,
  }) {
    _check();
    return _handle.resizeImage(page, imageName, width: width, height: height);
  }

  // ── Save (API_GOLD: one method with PdfSaveOptions) ──

  Future<void> save(PdfSink output, {
    PdfSaveOptions options = const PdfSaveOptions(),
  }) {
    _check();
    return _handle.save(output, options: options);
  }

  // ── Lifecycle ──

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _handle.dispose();
  }
}
