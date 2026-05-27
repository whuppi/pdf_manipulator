// PdfEditor — batch editing. Parse once, mutate many, save once.


import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

class PdfEditor {
  PdfEditor.internal(PdfBridge _, this._handle, {
    PdfEncryptionAlgorithm? sourceEncryption,
    PdfPermissions? sourcePermissions,
    String? password,
  }) : _sourceEncryption = sourceEncryption,
       _sourcePermissions = sourcePermissions,
       _password = password;

  final BridgeEditorHandle _handle;
  final PdfEncryptionAlgorithm? _sourceEncryption;
  final PdfPermissions? _sourcePermissions;
  final String? _password;
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

  Future<bool> get isModified {
    _check();
    return _handle.isModified;
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

  Future<void> selectPages(List<int> pages) {
    _check();
    return _handle.selectPages(pages);
  }

  Future<void> mergeFrom(DataSource otherPdf) {
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
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) {
    _check();
    return _handle.addWatermark(page, text, style: style, position: position, layer: layer);
  }

  Future<void> addStamp(int page, {
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addStamp(page,
        type: type, rect: rect, opacity: opacity);
  }

  Future<void> addImageStamp(int page, DataSource imageData, {
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addImageStamp(page, imageData,
        rect: rect, opacity: opacity);
  }

  // ── Content ──

  Future<void> embedFile(String name, DataSource data) {
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

  Future<void> resizeImage(int page, String imageName, {
    required double width, required double height,
  }) {
    _check();
    return _handle.resizeImage(page, imageName, width: width, height: height);
  }

  /// Apply best-effort PDF/A compliance fixes (output intent, font embedding,
  /// XMP metadata). Result is not guaranteed compliant — call
  /// [Pdf.validatePdfA] on the saved output to verify.
  Future<void> convertToPdfA({int level = 1}) {
    _check();
    return _handle.convertToPdfA(level: level);
  }

  // ── Redaction ──

  Future<void> addRedaction(int page, PdfRect region, {String? overlayText}) {
    _check();
    return _handle.addRedaction(page, region, overlayText: overlayText);
  }

  Future<int> redactionCount(int page) {
    _check();
    return _handle.redactionCount(page);
  }

  Future<void> applyRedactions() {
    _check();
    return _handle.applyRedactions();
  }

  Future<void> scrubMetadata() {
    _check();
    return _handle.scrubMetadata();
  }

  // ── Save ──

  Future<void> save(DataSink output, {
    PdfSaveOptions options = const PdfSaveOptions(),
  }) {
    _check();
    final resolved = _resolveEncryption(options);
    return _handle.save(output, options: resolved);
  }

  PdfSaveOptions _resolveEncryption(PdfSaveOptions options) {
    if (options.encryption is! PdfEncryptionKeep) return options;
    if (_sourceEncryption == null) return options;
    return PdfSaveOptions(
      compress: options.compress,
      garbageCollect: options.garbageCollect,
      encryption: PdfEncryption.config(
        ownerPassword: _password ?? '',
        userPassword: _password ?? '',
        algorithm: _sourceEncryption,
        permissions: _sourcePermissions ?? const PdfPermissions.all(),
      ),
    );
  }

  // ── Lifecycle ──

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _handle.dispose();
  }
}
