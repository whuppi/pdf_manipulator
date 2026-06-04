// PdfEditor — mutations only. Parse once, mutate many, save once.
//
// Every method here modifies the document. Read/export ops that don't
// modify belong on PdfDoc (queries) or PdfStandalone (one-shot export).
// Even if the Rust engine implements a non-mutating op on DocumentEditor,
// it must NOT be exposed here — wrap it as standalone or doc query instead.

import 'package:pdf_manipulator/src/ops/pdf.dart';
import 'package:pdf_manipulator/src/ops/pdf_doc.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// Mutable PDF editor — parse once, mutate many times, save once.
class PdfEditor {
  /// Internal constructor — use [Pdf.edit] to create instances.
  PdfEditor.internal(PdfBridge _, this._handle, {
    PdfEncryptionAlgorithm? sourceEncryption,
    PdfPermissions? sourcePermissions,
    String? password,
    PdfDoc? sourceDoc,
  }) : _sourceEncryption = sourceEncryption,
       _sourcePermissions = sourcePermissions,
       _password = password,
       _sourceDoc = sourceDoc;

  final BridgeEditorHandle _handle;
  final PdfEncryptionAlgorithm? _sourceEncryption;
  final PdfPermissions? _sourcePermissions;
  final String? _password;
  final PdfDoc? _sourceDoc;
  bool _disposed = false;

  void _check() {
    if (_disposed) throw StateError('This PdfEditor has been disposed');
  }

  // ── Properties ──

  /// Current number of pages in the document.
  Future<int> get pageCount {
    _check();
    return _handle.pageCount;
  }

  /// PDF version string of the document.
  Future<String> get version {
    _check();
    return _handle.version;
  }

  /// Whether unsaved modifications exist.
  Future<bool> get isModified {
    _check();
    return _handle.isModified;
  }

  // ── Metadata ──

  /// Returns the document title metadata.
  Future<String> getTitle() { _check(); return _handle.getTitle(); }

  /// Sets the document title metadata.
  Future<void> setTitle(String value) { _check(); return _handle.setTitle(value); }

  /// Returns the document author metadata.
  Future<String> getAuthor() { _check(); return _handle.getAuthor(); }

  /// Sets the document author metadata.
  Future<void> setAuthor(String value) { _check(); return _handle.setAuthor(value); }

  /// Returns the document subject metadata.
  Future<String> getSubject() { _check(); return _handle.getSubject(); }

  /// Sets the document subject metadata.
  Future<void> setSubject(String value) { _check(); return _handle.setSubject(value); }

  /// Returns the document keywords metadata.
  Future<String> getKeywords() { _check(); return _handle.getKeywords(); }

  /// Sets the document keywords metadata.
  Future<void> setKeywords(String value) { _check(); return _handle.setKeywords(value); }

  // ── Pages ──

  /// Rotates a single [page] by [degrees] (must be a multiple of 90).
  Future<void> rotatePage(int page, {required int degrees}) {
    _check();
    return _handle.rotatePage(page, degrees: degrees);
  }

  /// Rotates all pages by [degrees] (must be a multiple of 90).
  Future<void> rotateAllPages({required int degrees}) {
    _check();
    return _handle.rotateAllPages(degrees: degrees);
  }

  /// Returns the media box (bounding rectangle) of the given [page].
  Future<PdfRect> getPageMediaBox(int page) {
    _check();
    return _handle.getPageMediaBox(page);
  }

  /// Removes the given [page] (0-based index) from the document.
  Future<void> deletePage(int page) {
    _check();
    return _handle.deletePage(page);
  }

  /// Moves the page at index [from] to index [to].
  Future<void> movePage({required int from, required int to}) {
    _check();
    return _handle.movePage(from: from, to: to);
  }

  /// Keeps only the specified [pages] (by index), removing all others.
  Future<void> selectPages(List<int> pages) {
    _check();
    return _handle.selectPages(pages);
  }

  /// Appends all pages from [otherPdf] at the end of this document.
  Future<void> mergeFrom(DataSource otherPdf) {
    _check();
    return _handle.mergeFrom(otherPdf);
  }

  // ── Optimization ──

  /// Recompresses images above [minSize] pixels at the given [quality].
  Future<int> optimizeImages({int quality = 75, int minSize = 128}) {
    _check();
    return _handle.optimizeImages(quality: quality, minSize: minSize);
  }

  /// Removes embedded copies of standard PDF fonts to reduce file size.
  Future<int> unembedStandardFonts() {
    _check();
    return _handle.unembedStandardFonts();
  }

  // ── Watermark + stamps ──

  /// Adds a text watermark to the specified [page].
  Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) {
    _check();
    return _handle.addWatermark(page, text, style: style, position: position, layer: layer);
  }

  /// Adds a predefined stamp annotation to the specified [page].
  Future<void> addStamp(int page, {
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addStamp(page,
        type: type, rect: rect, opacity: opacity);
  }

  /// Adds a custom image stamp to the specified [page].
  Future<void> addImageStamp(int page, DataSource imageData, {
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addImageStamp(page, imageData,
        rect: rect, opacity: opacity);
  }

  // ── Content ──

  /// Embeds [data] as an attached file with the given [name].
  Future<void> embedFile(String name, DataSource data) {
    _check();
    return _handle.embedFile(name, data);
  }

  /// Erases content within the specified [regions] on [page].
  Future<void> eraseRegions(int page, List<PdfRect> regions) {
    _check();
    return _handle.eraseRegions(page, regions);
  }

  /// Flattens all interactive form fields into static content.
  Future<void> flattenForms() { _check(); return _handle.flattenForms(); }

  /// Flattens all annotations (forms, comments, stamps) into static content.
  Future<void> flattenAllAnnotations() { _check(); return _handle.flattenAllAnnotations(); }

  /// Sets the value of the form field identified by [fieldName].
  Future<void> setFormFieldValue(String fieldName, String value) {
    _check();
    return _handle.setFormFieldValue(fieldName, value);
  }

  /// Crops all pages by the specified margins (in points).
  Future<void> cropMargins({
    double left = 0, double right = 0,
    double top = 0, double bottom = 0,
  }) {
    _check();
    return _handle.cropMargins(
        left: left, right: right, top: top, bottom: bottom);
  }

  /// Resizes the named image on [page] to [width] x [height] points.
  Future<void> resizeImage(int page, String imageName, {
    required double width, required double height,
  }) {
    _check();
    return _handle.resizeImage(page, imageName, width: width, height: height);
  }

  /// Converts the document to PDF/A at the given conformance [level].
  Future<void> convertToPdfA({int level = 1}) {
    _check();
    return _handle.convertToPdfA(level: level);
  }

  // ── Redaction ──

  /// Marks a [region] on [page] for redaction (applied with [applyRedactions]).
  Future<void> addRedaction(int page, PdfRect region, {String? overlayText}) {
    _check();
    return _handle.addRedaction(page, region, overlayText: overlayText);
  }

  /// Returns the number of pending redaction marks on [page].
  Future<int> redactionCount(int page) {
    _check();
    return _handle.redactionCount(page);
  }

  /// Permanently applies all pending redaction marks, removing content.
  Future<void> applyRedactions() {
    _check();
    return _handle.applyRedactions();
  }

  /// Removes all document metadata (title, author, timestamps, etc.).
  Future<void> scrubMetadata() {
    _check();
    return _handle.scrubMetadata();
  }

  // ── Save ──

  /// Writes the modified document to [output] with the given save [options].
  Future<void> save(DataSink output, {
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  }) {
    _check();
    final resolved = _resolveEncryption(options);
    return _handle.save(output, options: resolved);
  }

  PdfSaveOptions _resolveEncryption(PdfSaveOptions options) {
    if (_sourceEncryption == null) return options;
    return switch (options) {
      PdfSaveFullRewrite(:final compress, :final garbageCollect, :final encryption) =>
        encryption is PdfEncryptionKeep
          ? PdfSaveOptions.fullRewrite(
              compress: compress,
              garbageCollect: garbageCollect,
              encryption: PdfEncryption.config(
                ownerPassword: _password ?? '',
                userPassword: _password ?? '',
                algorithm: _sourceEncryption,
                permissions: _sourcePermissions ?? const PdfPermissions.all(),
              ),
            )
          : options,
      PdfSaveIncremental() => options,
    };
  }

  // ── Lifecycle ──

  /// Releases editor resources and the underlying doc handle. Idempotent.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _handle.dispose();
    // Dispose the doc handle that edit() opened for metadata reading.
    // Without this, the doc handle leaks (pinned worker, held source).
    await _sourceDoc?.dispose();
  }
}
