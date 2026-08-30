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
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/keep/record_use_shim.dart';

/// Mutable PDF editor — parse once, mutate many times, save once.
class PdfEditor {
  /// Internal constructor — use [Pdf.edit] to create instances.
  PdfEditor.internal(
    PdfBridge _,
    this._handle, {
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
  PdfTask<int> get pageCount {
    _check();
    return _handle.pageCount;
  }

  /// PDF version string of the document.
  PdfTask<String> get version {
    _check();
    return _handle.version;
  }

  /// Whether unsaved modifications exist.
  PdfTask<bool> get isModified {
    _check();
    return _handle.isModified;
  }

  // ── Metadata ──

  /// Returns the document title metadata.
  PdfTask<String> getTitle() {
    _check();
    return _handle.getTitle();
  }

  /// Sets the document title metadata.
  PdfTask<void> setTitle(String value) {
    _check();
    return _handle.setTitle(value);
  }

  /// Returns the document author metadata.
  PdfTask<String> getAuthor() {
    _check();
    return _handle.getAuthor();
  }

  /// Sets the document author metadata.
  PdfTask<void> setAuthor(String value) {
    _check();
    return _handle.setAuthor(value);
  }

  /// Returns the document subject metadata.
  PdfTask<String> getSubject() {
    _check();
    return _handle.getSubject();
  }

  /// Sets the document subject metadata.
  PdfTask<void> setSubject(String value) {
    _check();
    return _handle.setSubject(value);
  }

  /// Returns the document keywords metadata.
  PdfTask<String> getKeywords() {
    _check();
    return _handle.getKeywords();
  }

  /// Sets the document keywords metadata.
  PdfTask<void> setKeywords(String value) {
    _check();
    return _handle.setKeywords(value);
  }

  /// Returns the document producer metadata (the software that produced the PDF).
  PdfTask<String> getProducer() {
    _check();
    return _handle.getProducer();
  }

  /// Sets the document producer metadata.
  PdfTask<void> setProducer(String value) {
    _check();
    return _handle.setProducer(value);
  }

  /// Returns the document creation date as a raw PDF date string
  /// (e.g. `D:20240101120000Z`).
  PdfTask<String> getCreationDate() {
    _check();
    return _handle.getCreationDate();
  }

  /// Sets the document creation date. Expects a raw PDF date string
  /// (e.g. `D:20240101120000Z`).
  PdfTask<void> setCreationDate(String value) {
    _check();
    return _handle.setCreationDate(value);
  }

  // ── Pages ──

  /// Rotates a single [page] by [degrees] (must be a multiple of 90).
  PdfTask<void> rotatePage(int page, {required int degrees}) {
    _check();
    return _handle.rotatePage(page, degrees: degrees);
  }

  /// Rotates all pages by [degrees] (must be a multiple of 90).
  PdfTask<void> rotateAllPages({required int degrees}) {
    _check();
    return _handle.rotateAllPages(degrees: degrees);
  }

  /// Returns the media box (bounding rectangle) of the given [page].
  PdfTask<PdfRect> getPageMediaBox(int page) {
    _check();
    return _handle.getPageMediaBox(page);
  }

  /// Removes the given [page] (0-based index) from the document.
  PdfTask<void> deletePage(int page) {
    _check();
    return _handle.deletePage(page);
  }

  /// Moves the page at index [from] to index [to].
  PdfTask<void> movePage({required int from, required int to}) {
    _check();
    return _handle.movePage(from: from, to: to);
  }

  /// Keeps only the specified [pages] (by index), removing all others.
  PdfTask<void> selectPages(List<int> pages) {
    _check();
    return _handle.selectPages(pages);
  }

  /// Appends all pages from [otherPdf] at the end of this document.
  PdfTask<void> mergeFrom(DataSource otherPdf) {
    _check();
    return _handle.mergeFrom(otherPdf);
  }

  // ── Optimization ──

  /// Recompresses images above [minSize] pixels at the given [quality].
  PdfTask<int> optimizeImages({int quality = 75, int minSize = 128}) {
    KeepRecord.op('render');
    _check();
    return _handle.optimizeImages(quality: quality, minSize: minSize);
  }

  /// Removes embedded copies of standard PDF fonts to reduce file size.
  PdfTask<int> unembedStandardFonts() {
    _check();
    return _handle.unembedStandardFonts();
  }

  // ── Watermark + stamps ──

  /// Adds a text watermark to the specified [page].
  PdfTask<void> addWatermark(
    int page,
    String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) {
    _check();
    return _handle.addWatermark(
      page,
      text,
      style: style,
      position: position,
      layer: layer,
    );
  }

  /// Adds a predefined stamp annotation to the specified [page].
  PdfTask<void> addStamp(
    int page, {
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addStamp(page, type: type, rect: rect, opacity: opacity);
  }

  /// Adds a custom image stamp to the specified [page].
  PdfTask<void> addImageStamp(
    int page,
    DataSource imageData, {
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addImageStamp(page, imageData, rect: rect, opacity: opacity);
  }

  // ── Content ──

  /// Embeds [data] as an attached file with the given [name].
  PdfTask<void> embedFile(String name, DataSource data) {
    _check();
    return _handle.embedFile(name, data);
  }

  /// Erases content within the specified [regions] on [page].
  PdfTask<void> eraseRegions(int page, List<PdfRect> regions) {
    _check();
    return _handle.eraseRegions(page, regions);
  }

  /// Flattens all interactive form fields into static content.
  PdfTask<void> flattenForms() {
    _check();
    return _handle.flattenForms();
  }

  /// Flattens all annotations (forms, comments, stamps) into static content.
  PdfTask<void> flattenAllAnnotations() {
    _check();
    return _handle.flattenAllAnnotations();
  }

  /// Sets the value of the form field identified by [fieldName].
  ///
  /// On a button field (checkbox or radio) the value is an appearance-state
  /// name — `'Yes'`, `'On'`, or whichever name that widget uses — not text to
  /// draw. Pass `'Off'` to clear it. Use [setCheckboxFieldValue] for a
  /// checkbox whose state name you do not know.
  PdfTask<void> setFormFieldValue(String fieldName, String value) {
    _check();
    return _handle.setFormFieldValue(fieldName, value);
  }

  /// Checks or clears the checkbox field identified by [fieldName].
  ///
  /// Selects whichever on-state the widget offers, so this works on a box
  /// named `/On` or `/1` as well as the common `/Yes`.
  PdfTask<void> setCheckboxFieldValue(String fieldName, bool checked) {
    _check();
    return _handle.setCheckboxFieldValue(fieldName, checked);
  }

  /// Crops all pages by the specified margins (in points).
  PdfTask<void> cropMargins({
    double left = 0,
    double right = 0,
    double top = 0,
    double bottom = 0,
  }) {
    _check();
    return _handle.cropMargins(
      left: left,
      right: right,
      top: top,
      bottom: bottom,
    );
  }

  /// Resizes the named image on [page] to [width] x [height] points.
  PdfTask<void> resizeImage(
    int page,
    String imageName, {
    required double width,
    required double height,
  }) {
    _check();
    return _handle.resizeImage(page, imageName, width: width, height: height);
  }

  /// Converts the document to PDF/A at the given conformance [level].
  PdfTask<void> convertToPdfA({int level = 1}) {
    KeepRecord.op('pdfa');
    _check();
    return _handle.convertToPdfA(level: level);
  }

  // ── Redaction ──

  /// Marks a [region] on [page] for redaction (applied with [applyRedactions]).
  PdfTask<void> addRedaction(int page, PdfRect region, {String? overlayText}) {
    _check();
    return _handle.addRedaction(page, region, overlayText: overlayText);
  }

  /// Returns the number of pending redaction marks on [page].
  PdfTask<int> redactionCount(int page) {
    _check();
    return _handle.redactionCount(page);
  }

  /// Permanently applies all pending redaction marks, removing content.
  PdfTask<void> applyRedactions() {
    _check();
    return _handle.applyRedactions();
  }

  /// Removes all document metadata (title, author, timestamps, etc.).
  PdfTask<void> scrubMetadata() {
    _check();
    return _handle.scrubMetadata();
  }

  // ── Save ──

  /// Writes the modified document to [output] with the given save [options].
  PdfTask<void> save(
    DataSink output, {
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  }) {
    _check();
    final resolved = _resolveEncryption(options);
    return _handle.save(output, options: resolved);
  }

  PdfSaveOptions _resolveEncryption(PdfSaveOptions options) {
    if (_sourceEncryption == null) return options;
    return switch (options) {
      PdfSaveFullRewrite(
        :final compress,
        :final garbageCollect,
        :final encryption,
      ) =>
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
