import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_info.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/pdf_sink.dart';
import 'package:pdf_manipulator/src/core/pdf_source.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';
import 'package:pdf_manipulator/src/editor/pdf_editor.dart';
import 'package:pdf_manipulator/src/builder/pdf_builder.dart';
import 'package:pdf_manipulator/src/platform/pdf_platform.dart';
import 'package:pdf_manipulator/src/platform/platform.dart';

/// PDF manipulation — every instance is its own worker.
///
/// Input: [PdfSource] (random-access reader).
/// Output: [PdfSink] (sequential writer) for operations producing a PDF.
///
/// ```dart
/// final pdf = Pdf();
/// await pdf.merge([sourceA, sourceB], outputSink);
/// final text = await pdf.extractText(source);
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
  /// final editor = await Pdf.edit(source);
  /// await editor.setTitle('Report');
  /// await editor.rotatePage(0, degrees: 90);
  /// await editor.save(outputSink);
  /// editor.dispose();
  /// ```
  static Future<PdfEditor> edit(PdfSource source) async {
    final platform = createPlatform();
    final handle = await platform.openEditor(source);
    return PdfEditor.internal(platform, handle);
  }

  /// Create a new PDF from scratch.
  ///
  /// ```dart
  /// final builder = await Pdf.build();
  /// final page = await builder.addA4Page();
  /// await page.text('Hello');
  /// await page.done();
  /// await builder.save(outputSink);
  /// builder.dispose();
  /// ```
  static Future<PdfBuilder> build() async {
    final platform = createPlatform();
    final handle = await platform.createBuilder();
    return PdfBuilder.internal(platform, handle);
  }

  // ── Inspect ────────────────────────────────────────────────────

  Future<PdfDoc> open(PdfSource source, {String? password}) =>
      _platform.open(source, password: password);

  Future<PdfInfo> probe(PdfSource source) => _platform.probe(source);

  // ── Structural ─────────────────────────────────────────────────

  Future<void> merge(List<PdfSource> inputs, PdfSink output) =>
      _platform.merge(inputs, output);

  Future<void> split(PdfSource source, PdfSink Function(int index) sinkFactory,
          {required int every}) =>
      _platform.split(source, sinkFactory, every: every);

  Future<int> splitBySize(
          PdfSource source, PdfSink Function(int index) sinkFactory,
          {required int maxBytes}) =>
      _platform.splitBySize(source, sinkFactory, maxBytes: maxBytes);

  Future<void> extractPages(PdfSource source, PdfSink output,
          {required List<int> pages}) =>
      _platform.extractPages(source, output, pages: pages);

  Future<void> deletePages(PdfSource source, PdfSink output,
          {required List<int> pages}) =>
      _platform.deletePages(source, output, pages: pages);

  Future<void> reorderPages(PdfSource source, PdfSink output,
          {required List<int> order}) =>
      _platform.reorderPages(source, output, order: order);

  Future<void> movePage(PdfSource source, PdfSink output,
          {required int from, required int to}) =>
      _platform.movePage(source, output, from: from, to: to);

  Future<void> rotatePages(PdfSource source, PdfSink output,
          {required Map<int, int> pages}) =>
      _platform.rotatePages(source, output, pages: pages);

  Future<void> rotateAllPages(PdfSource source, PdfSink output,
          {required int degrees}) =>
      _platform.rotateAllPages(source, output, degrees: degrees);

  // ── Content ────────────────────────────────────────────────────

  Future<void> flattenForms(PdfSource source, PdfSink output) =>
      _platform.flattenForms(source, output);

  Future<void> applyRedactions(PdfSource source, PdfSink output) =>
      _platform.applyRedactions(source, output);

  Future<void> embedFile(PdfSource source, PdfSink output,
          {required String name, required Uint8List fileData}) =>
      _platform.embedFile(source, output, name: name, fileData: fileData);

  Future<void> eraseRegions(PdfSource source, PdfSink output,
          {required int page, required List<PdfRect> regions}) =>
      _platform.eraseRegions(source, output, page: page, regions: regions);

  Future<void> compress(PdfSource source, PdfSink output,
          {int imageQuality = 75,
          bool garbageCollect = true,
          bool linearize = false}) =>
      _platform.compress(source, output,
          imageQuality: imageQuality,
          garbageCollect: garbageCollect,
          linearize: linearize);

  // ── Extraction ─────────────────────────────────────────────────

  Future<String> extractText(PdfSource source,
          {int? page, String? password}) =>
      _platform.extractText(source, page: page, password: password);

  Future<String> toMarkdown(PdfSource source,
          {int? page, String? password}) =>
      _platform.toMarkdown(source, page: page, password: password);

  Future<String> toHtml(PdfSource source,
          {required int page, String? password}) =>
      _platform.toHtml(source, page: page, password: password);

  Future<String> toPlainText(PdfSource source,
          {required int page, String? password}) =>
      _platform.toPlainText(source, page: page, password: password);

  // ── Search ─────────────────────────────────────────────────────

  Future<List<SearchResult>> searchPage(PdfSource source,
          {required int page, required String query, String? password}) =>
      _platform.searchPage(source,
          page: page, query: query, password: password);

  Future<List<SearchResult>> searchAll(PdfSource source,
          {required String query, String? password}) =>
      _platform.searchAll(source, query: query, password: password);

  // ── Security ───────────────────────────────────────────────────

  Future<void> watermark(PdfSource source, PdfSink output,
          {required String text,
          List<int>? pages,
          double opacity = 0.3,
          double fontSize = 48,
          double rotation = 45,
          double r = 0.5,
          double g = 0.5,
          double b = 0.5}) =>
      _platform.watermark(source, output,
          text: text,
          pages: pages,
          opacity: opacity,
          fontSize: fontSize,
          rotation: rotation,
          r: r,
          g: g,
          b: b);

  Future<void> watermarkPositioned(PdfSource source, PdfSink output, {
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
  }) => _platform.watermarkPositioned(source, output,
      text: text, x: x, y: y, width: width, height: height,
      pages: pages, fontSize: fontSize, fontName: fontName,
      rotation: rotation, opacity: opacity, r: r, g: g, b: b,
      fixedPrint: fixedPrint, fixedPrintH: fixedPrintH, fixedPrintV: fixedPrintV);

  Future<void> encrypt(PdfSource source, PdfSink output,
          {required String ownerPassword, String userPassword = ''}) =>
      _platform.encrypt(source, output,
          ownerPassword: ownerPassword, userPassword: userPassword);

  Future<void> encryptFull(PdfSource source, PdfSink output, {
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
  }) => _platform.encryptFull(source, output,
      ownerPassword: ownerPassword, userPassword: userPassword,
      algorithm: algorithm,
      allowPrint: allowPrint, allowPrintHq: allowPrintHq,
      allowModify: allowModify, allowCopy: allowCopy,
      allowAnnotate: allowAnnotate, allowFillForms: allowFillForms,
      allowAccessibility: allowAccessibility, allowAssemble: allowAssemble);

  Future<void> decrypt(PdfSource source, PdfSink output,
          {required String password}) =>
      _platform.decrypt(source, output, password: password);

  Future<void> sign(PdfSource source, PdfSink output,
          {required Uint8List certificate,
          required String certificatePassword,
          String? reason,
          String? location}) =>
      _platform.sign(source, output,
          certificate: certificate,
          certificatePassword: certificatePassword,
          reason: reason,
          location: location);

  // ── Stamps ─────────────────────────────────────────────────────

  Future<void> addImageStamp(PdfSource source, PdfSink output, {
    required int page,
    required Uint8List imageBytes,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) async {
    final editor = await _platform.openEditor(source);
    try {
      await editor.addImageStamp(page, imageBytes,
          x: x, y: y, width: width, height: height, opacity: opacity);
      await editor.save(output);
    } finally {
      await editor.dispose();
    }
  }

  Future<void> addStamp(PdfSource source, PdfSink output, {
    required int page,
    required int stampType,
    String? customName,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) async {
    final editor = await _platform.openEditor(source);
    try {
      await editor.addStamp(page,
          stampType: stampType, customName: customName,
          x: x, y: y, width: width, height: height, opacity: opacity);
      await editor.save(output);
    } finally {
      await editor.dispose();
    }
  }

  // ── Creation ───────────────────────────────────────────────────

  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output) =>
      _platform.imagesToPdf(images, output);

  // ── Rendering ──────────────────────────────────────────────────

  Future<RenderedPage> renderPage(PdfSource source, int pageIndex,
          {String? password}) =>
      _platform.renderPage(source, pageIndex, password: password);

  Future<RenderedPage> renderPageFit(PdfSource source, int pageIndex,
          {required int width, required int height, String? password}) =>
      _platform.renderPageFit(source, pageIndex,
          width: width, height: height, password: password);

  Future<RenderedPage> renderPageThumbnail(PdfSource source, int pageIndex,
          {required int size, String? password}) =>
      _platform.renderPageThumbnail(source, pageIndex,
          size: size, password: password);

  Stream<RenderedPage> renderAllPages(PdfSource source,
          {required int width, required int height, String? password}) =>
      _platform.renderAllPages(source,
          width: width, height: height, password: password);

  // ── Image extraction ───────────────────────────────────────────

  Stream<PdfImage> extractImages(PdfSource source, int pageIndex,
          {String? password}) =>
      _platform.extractImages(source, pageIndex, password: password);

  Stream<PdfImage> extractAllImages(PdfSource source,
          {String? password}) =>
      _platform.extractAllImages(source, password: password);

  // ── Signatures ─────────────────────────────────────────────────

  Future<int> getSignatureCount(PdfSource source, {String? password}) =>
      _platform.getSignatureCount(source, password: password);

  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
          {String? password}) =>
      _platform.getSignatures(source, password: password);

  Future<bool> verifySignatures(PdfSource source, {String? password}) =>
      _platform.verifySignatures(source, password: password);

  // ── Validation ─────────────────────────────────────────────────

  Future<({bool compliant, int errors, int warnings})> validatePdfA(
          PdfSource source,
          {int level = 2,
          String? password}) =>
      _platform.validatePdfA(source, level: level, password: password);

  Future<bool> validatePdfUa(PdfSource source,
          {int level = 1, String? password}) =>
      _platform.validatePdfUa(source, level: level, password: password);

  // ── Encryption info ────────────────────────────────────────────

  Future<
      ({
        bool print,
        bool printHq,
        bool modify,
        bool copy,
        bool annotate,
        bool fillForms,
        bool accessibility,
        bool assemble
      })> getPermissions(PdfSource source, {String? password}) =>
      _platform.getPermissions(source, password: password);

  Future<int> getEncryptionAlgorithm(PdfSource source, {String? password}) =>
      _platform.getEncryptionAlgorithm(source, password: password);

  // ── Configuration ──────────────────────────────────────────────

  void configureWorkerUrl(String url) => _platform.configureWorkerUrl(url);

  // ── Lifecycle ──────────────────────────────────────────────────

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _p?.dispose();
    _p = null;
  }
}
