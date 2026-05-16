import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_info.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';
import 'package:pdf_manipulator/src/editor/pdf_editor.dart';
import 'package:pdf_manipulator/src/builder/pdf_builder.dart';
import 'package:pdf_manipulator/src/platform/pdf_platform.dart';
import 'package:pdf_manipulator/src/platform/platform.dart';

/// PDF manipulation — every instance is its own worker.
///
/// ```dart
/// final pdf = Pdf();
/// final merged = await pdf.merge([bytesA, bytesB]);
/// final text = await pdf.extractText(bytes);
/// pdf.dispose();
/// ```
///
/// For batch editing, use [Pdf.edit]. For creating PDFs from scratch,
/// use [Pdf.build]. Each spawns its own worker — dispose when done.
class Pdf {
  PdfPlatform? _p;
  bool _disposed = false;

  Pdf() : _p = createPlatform();

  PdfPlatform get _platform {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
    return _p!;
  }

  // ── Static factories ──────────────────────────────────────────

  /// Open a PDF for batch editing. Parse once, mutate many times, save once.
  ///
  /// ```dart
  /// final editor = await Pdf.edit(bytes);
  /// await editor.setTitle('Report');
  /// await editor.rotatePage(0, degrees: 90);
  /// final result = await editor.save();
  /// editor.dispose();
  /// ```
  static Future<PdfEditor> edit(Uint8List bytes) async {
    final platform = createPlatform();
    final handle = await platform.openEditor(bytes);
    return PdfEditor.internal(platform, handle);
  }

  /// Create a new PDF from scratch.
  ///
  /// ```dart
  /// final builder = await Pdf.build();
  /// final page = await builder.addA4Page();
  /// await page.text('Hello');
  /// await page.done();
  /// final result = await builder.save();
  /// builder.dispose();
  /// ```
  static Future<PdfBuilder> build() async {
    final platform = createPlatform();
    final handle = await platform.createBuilder();
    return PdfBuilder.internal(platform, handle);
  }

  // ── Inspect ────────────────────────────────────────────────────

  Future<PdfDoc> open(Uint8List bytes, {String? password}) =>
      _platform.open(bytes, password: password);

  Future<PdfInfo> probe(Uint8List bytes) => _platform.probe(bytes);

  // ── Structural ─────────────────────────────────────────────────

  Future<Uint8List> merge(List<Uint8List> inputs) => _platform.merge(inputs);

  Future<List<Uint8List>> split(Uint8List bytes, {required int every}) =>
      _platform.split(bytes, every: every);

  Future<List<Uint8List>> splitBySize(Uint8List bytes, {required int maxBytes}) =>
      _platform.splitBySize(bytes, maxBytes: maxBytes);

  Future<Uint8List> extractPages(Uint8List bytes, {required List<int> pages}) =>
      _platform.extractPages(bytes, pages: pages);

  Future<Uint8List> deletePages(Uint8List bytes, {required List<int> pages}) =>
      _platform.deletePages(bytes, pages: pages);

  Future<Uint8List> reorderPages(Uint8List bytes, {required List<int> order}) =>
      _platform.reorderPages(bytes, order: order);

  Future<Uint8List> movePage(Uint8List bytes, {required int from, required int to}) =>
      _platform.movePage(bytes, from: from, to: to);

  Future<Uint8List> rotatePages(Uint8List bytes, {required Map<int, int> pages}) =>
      _platform.rotatePages(bytes, pages: pages);

  Future<Uint8List> rotateAllPages(Uint8List bytes, {required int degrees}) =>
      _platform.rotateAllPages(bytes, degrees: degrees);

  // ── Content ────────────────────────────────────────────────────

  Future<Uint8List> flattenForms(Uint8List bytes) => _platform.flattenForms(bytes);

  Future<Uint8List> applyRedactions(Uint8List bytes) => _platform.applyRedactions(bytes);

  Future<Uint8List> embedFile(Uint8List bytes,
      {required String name, required Uint8List fileData}) =>
      _platform.embedFile(bytes, name: name, fileData: fileData);

  Future<Uint8List> eraseRegions(Uint8List bytes,
      {required int page, required List<PdfRect> regions}) =>
      _platform.eraseRegions(bytes, page: page, regions: regions);

  Future<Uint8List> compress(Uint8List bytes,
      {int imageQuality = 75, bool garbageCollect = true, bool linearize = false}) =>
      _platform.compress(bytes, imageQuality: imageQuality,
          garbageCollect: garbageCollect, linearize: linearize);

  // ── Extraction ─────────────────────────────────────────────────

  Future<String> extractText(Uint8List bytes, {int? page, String? password}) =>
      _platform.extractText(bytes, page: page, password: password);

  Future<String> toMarkdown(Uint8List bytes, {int? page, String? password}) =>
      _platform.toMarkdown(bytes, page: page, password: password);

  Future<String> toHtml(Uint8List bytes, {required int page, String? password}) =>
      _platform.toHtml(bytes, page: page, password: password);

  Future<String> toPlainText(Uint8List bytes, {required int page, String? password}) =>
      _platform.toPlainText(bytes, page: page, password: password);

  // ── Search ─────────────────────────────────────────────────────

  Future<List<SearchResult>> searchPage(Uint8List bytes,
      {required int page, required String query, String? password}) =>
      _platform.searchPage(bytes, page: page, query: query, password: password);

  Future<List<SearchResult>> searchAll(Uint8List bytes,
      {required String query, String? password}) =>
      _platform.searchAll(bytes, query: query, password: password);

  // ── Security ───────────────────────────────────────────────────

  Future<Uint8List> watermark(Uint8List bytes,
      {required String text, List<int>? pages, double opacity = 0.3,
       double fontSize = 48, double rotation = 45,
       double r = 0.5, double g = 0.5, double b = 0.5}) =>
      _platform.watermark(bytes, text: text, pages: pages, opacity: opacity,
          fontSize: fontSize, rotation: rotation, r: r, g: g, b: b);

  Future<Uint8List> watermarkPositioned(Uint8List bytes, {
    required String text,
    required double x, required double y,
    required double width, required double height,
    List<int>? pages,
    double fontSize = 48, String? fontName,
    double rotation = 45, double opacity = 0.3,
    double r = 0.5, double g = 0.5, double b = 0.5,
    bool fixedPrint = false,
    double fixedPrintH = 0.0,
    double fixedPrintV = 0.0,
  }) => _platform.watermarkPositioned(bytes, text: text,
      x: x, y: y, width: width, height: height, pages: pages,
      fontSize: fontSize, fontName: fontName,
      rotation: rotation, opacity: opacity, r: r, g: g, b: b,
      fixedPrint: fixedPrint, fixedPrintH: fixedPrintH, fixedPrintV: fixedPrintV);

  Future<Uint8List> addStamp(Uint8List bytes, {
    required int page, required int stampType,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) async {
    final editor = await Pdf.edit(bytes);
    try {
      await editor.addStamp(page,
          stampType: stampType, x: x, y: y, width: width, height: height,
          opacity: opacity);
      return editor.save();
    } finally {
      editor.dispose();
    }
  }

  Future<Uint8List> addImageStamp(Uint8List bytes, {
    required int page, required Uint8List imageBytes,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) async {
    final editor = await Pdf.edit(bytes);
    try {
      await editor.addImageStamp(page, imageBytes,
          x: x, y: y, width: width, height: height, opacity: opacity);
      return editor.save();
    } finally {
      editor.dispose();
    }
  }

  Future<Uint8List> encrypt(Uint8List bytes,
      {required String ownerPassword, String userPassword = ''}) =>
      _platform.encrypt(bytes, ownerPassword: ownerPassword, userPassword: userPassword);

  Future<Uint8List> encryptFull(Uint8List bytes, {
    required String ownerPassword,
    String userPassword = '',
    int algorithm = 3,
    bool allowPrint = true,
    bool allowPrintHq = true,
    bool allowModify = true,
    bool allowCopy = true,
    bool allowAnnotate = true,
    bool allowFillForms = true,
    bool allowAccessibility = true,
    bool allowAssemble = true,
  }) => _platform.encryptFull(bytes,
      ownerPassword: ownerPassword, userPassword: userPassword,
      algorithm: algorithm,
      allowPrint: allowPrint, allowPrintHq: allowPrintHq,
      allowModify: allowModify, allowCopy: allowCopy,
      allowAnnotate: allowAnnotate, allowFillForms: allowFillForms,
      allowAccessibility: allowAccessibility, allowAssemble: allowAssemble);

  Future<Uint8List> decrypt(Uint8List bytes, {required String password}) =>
      _platform.decrypt(bytes, password: password);

  Future<Uint8List> sign(Uint8List bytes,
      {required Uint8List certificate, required String certificatePassword,
       String? reason, String? location}) =>
      _platform.sign(bytes, certificate: certificate, certificatePassword: certificatePassword,
          reason: reason, location: location);

  // ── Creation ───────────────────────────────────────────────────

  Future<Uint8List> imagesToPdf(List<Uint8List> images) =>
      _platform.imagesToPdf(images);

  // ── Rendering ──────────────────────────────────────────────────

  Future<RenderedPage> renderPage(Uint8List bytes, int pageIndex,
      {String? password}) => _platform.renderPage(bytes, pageIndex, password: password);

  Future<RenderedPage> renderPageFit(Uint8List bytes, int pageIndex,
      {required int width, required int height, String? password}) =>
      _platform.renderPageFit(bytes, pageIndex, width: width, height: height, password: password);

  Future<RenderedPage> renderPageThumbnail(Uint8List bytes, int pageIndex,
      {required int size, String? password}) =>
      _platform.renderPageThumbnail(bytes, pageIndex, size: size, password: password);

  Future<List<RenderedPage>> renderAllPages(Uint8List bytes,
      {required int width, required int height, String? password}) =>
      _platform.renderAllPages(bytes, width: width, height: height, password: password);

  // ── Image extraction ───────────────────────────────────────────

  Future<List<PdfImage>> extractImages(Uint8List bytes, int pageIndex,
      {String? password}) => _platform.extractImages(bytes, pageIndex, password: password);

  Future<List<PdfImage>> extractAllImages(Uint8List bytes, {String? password}) =>
      _platform.extractAllImages(bytes, password: password);

  // ── Signatures ─────────────────────────────────────────────────

  Future<int> getSignatureCount(Uint8List bytes, {String? password}) =>
      _platform.getSignatureCount(bytes, password: password);

  Future<List<PdfSignatureInfo>> getSignatures(Uint8List bytes,
      {String? password}) => _platform.getSignatures(bytes, password: password);

  Future<bool> verifySignatures(Uint8List bytes, {String? password}) =>
      _platform.verifySignatures(bytes, password: password);

  // ── Validation ─────────────────────────────────────────────────

  Future<({bool compliant, int errors, int warnings})> validatePdfA(
      Uint8List bytes, {int level = 2, String? password}) =>
      _platform.validatePdfA(bytes, level: level, password: password);

  Future<bool> validatePdfUa(Uint8List bytes,
      {int level = 1, String? password}) =>
      _platform.validatePdfUa(bytes, level: level, password: password);

  // ── Encryption info ───────────────────────────────────────────

  Future<({bool print, bool printHq, bool modify, bool copy,
      bool annotate, bool fillForms, bool accessibility, bool assemble})>
    getPermissions(Uint8List bytes, {String? password}) =>
      _platform.getPermissions(bytes, password: password);

  Future<int> getEncryptionAlgorithm(Uint8List bytes,
      {String? password}) =>
      _platform.getEncryptionAlgorithm(bytes, password: password);

  // ── Configuration ──────────────────────────────────────────────

  void configureWorkerUrl(String url) => _platform.configureWorkerUrl(url);

  // ── Lifecycle ──────────────────────────────────────────────────

  /// Dispose this instance. Cancels all pending operations instantly.
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _p?.dispose();
    _p = null;
  }

  bool get isDisposed => _disposed;
}
