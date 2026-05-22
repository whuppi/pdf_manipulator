import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_info.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/pdf_sink.dart';
import 'package:pdf_manipulator/src/core/pdf_source.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';

/// Platform-agnostic contract for all PDF operations.
///
/// Input: [PdfSource] (random-access reader).
/// Output: [PdfSink] (sequential writer) for operations producing a PDF.
/// No dart:io. No Uint8List for PDF I/O.
abstract interface class PdfPlatform {
  // ── Inspect ──────────────────────────────────────────────────────────────
  Future<PdfDoc> open(PdfSource source, {String? password});
  Future<PdfInfo> probe(PdfSource source);

  // ── Structural ───────────────────────────────────────────────────────────
  Future<void> merge(List<PdfSource> inputs, PdfSink output);
  Future<void> split(PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int every});
  Future<int> splitBySize(
      PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int maxBytes});
  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages});
  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages});
  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order});
  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to});
  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages});
  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees});

  // ── Content ──────────────────────────────────────────────────────────────
  Future<void> flattenForms(PdfSource source, PdfSink output);
  Future<void> applyRedactions(PdfSource source, PdfSink output);
  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData});
  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions});
  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75, bool garbageCollect = true, bool linearize = false});

  // ── Extraction ───────────────────────────────────────────────────────────
  Future<String> extractText(PdfSource source, {int? page, String? password});
  Future<String> toMarkdown(PdfSource source, {int? page, String? password});
  Future<String> toHtml(PdfSource source,
      {required int page, String? password});
  Future<String> toPlainText(PdfSource source,
      {required int page, String? password});

  // ── Search ───────────────────────────────────────────────────────────────
  Future<List<SearchResult>> searchPage(PdfSource source,
      {required int page, required String query, String? password});
  Future<List<SearchResult>> searchAll(PdfSource source,
      {required String query, String? password});

  // ── Security ─────────────────────────────────────────────────────────────
  Future<void> watermark(PdfSource source, PdfSink output,
      {required String text,
      List<int>? pages,
      double opacity = 0.3,
      double fontSize = 48,
      double rotation = 45,
      double r = 0.5,
      double g = 0.5,
      double b = 0.5});
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
  });
  Future<void> encrypt(PdfSource source, PdfSink output,
      {required String ownerPassword, String userPassword = ''});
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
  });
  Future<void> decrypt(PdfSource source, PdfSink output,
      {required String password});
  Future<void> sign(PdfSource source, PdfSink output,
      {required Uint8List certificate,
      required String certificatePassword,
      String? reason,
      String? location});

  // ── Creation ─────────────────────────────────────────────────────────────
  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output);

  // ── Rendering ────────────────────────────────────────────────────────────
  Future<RenderedPage> renderPage(PdfSource source, int pageIndex,
      {String? password});
  Future<RenderedPage> renderPageFit(PdfSource source, int pageIndex,
      {required int width, required int height, String? password});
  Future<RenderedPage> renderPageThumbnail(PdfSource source, int pageIndex,
      {required int size, String? password});
  Stream<RenderedPage> renderAllPages(PdfSource source,
      {required int width, required int height, String? password});

  // ── Image extraction ─────────────────────────────────────────────────────
  Stream<PdfImage> extractImages(PdfSource source, int pageIndex,
      {String? password});
  Stream<PdfImage> extractAllImages(PdfSource source, {String? password});

  // ── Signatures ───────────────────────────────────────────────────────────
  Future<int> getSignatureCount(PdfSource source, {String? password});
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
      {String? password});
  Future<bool> verifySignatures(PdfSource source, {String? password});

  // ── Validation ───────────────────────────────────────────────────────────
  Future<({bool compliant, int errors, int warnings})> validatePdfA(
      PdfSource source,
      {int level = 2,
      String? password});
  Future<bool> validatePdfUa(PdfSource source,
      {int level = 1, String? password});

  // ── Encryption info ──────────────────────────────────────────────────────
  Future<({bool print, bool printHq, bool modify, bool copy, bool annotate,
      bool fillForms, bool accessibility, bool assemble})>
    getPermissions(PdfSource source, {String? password});
  Future<int> getEncryptionAlgorithm(PdfSource source, {String? password});

  // ── Editor ───────────────────────────────────────────────────────────────
  Future<PdfEditorHandle> openEditor(PdfSource source);

  // ── Builder ──────────────────────────────────────────────────────────────
  Future<PdfBuilderHandle> createBuilder();

  // ── Configuration ─────────────────────────────────────────────────────────
  void configureWorkerUrl(String url);

  // ── Lifecycle ────────────────────────────────────────────────────────────
  Future<void> dispose();
}

/// Cross-platform handle for mutable PDF editing sessions.
abstract interface class PdfEditorHandle {
  Future<int> get pageCount;
  Future<String> get version;
  Future<bool> get isModified;

  Future<String> getTitle();
  Future<void> setTitle(String value);
  Future<String> getAuthor();
  Future<void> setAuthor(String value);
  Future<String> getSubject();
  Future<void> setSubject(String value);
  Future<String> getKeywords();
  Future<void> setKeywords(String value);

  Future<void> rotatePage(int pageIndex, {required int degrees});
  Future<void> rotateAllPages({required int degrees});
  Future<PdfRect> getPageMediaBox(int pageIndex);
  Future<void> deletePage(int pageIndex);
  Future<void> movePage({required int from, required int to});
  Future<void> extractPages(List<int> pages, PdfSink output);
  Future<void> mergeFrom(PdfSource otherPdf);

  Future<int> optimizeImages({int quality = 75});
  Future<int> unembedStandardFonts();
  Future<void> addWatermark(int pageIndex, String text,
      {double fontSize = 48, double rotation = 45, double opacity = 0.3,
       double r = 0.5, double g = 0.5, double b = 0.5});

  Future<void> embedFile(String name, Uint8List data);
  Future<void> eraseRegions(int pageIndex, List<PdfRect> regions);
  Future<void> flattenForms();
  Future<void> flattenAllAnnotations();
  Future<void> applyAllRedactions();
  Future<void> setFormFieldValue(String fieldName, String value);
  Future<void> cropMargins({double left = 0, double right = 0,
      double top = 0, double bottom = 0});
  Future<void> convertToPdfA({int level = 1});

  Future<void> addWatermarkPositioned(int pageIndex, String text, {
    required double x, required double y,
    required double width, required double height,
    double fontSize = 48, String? fontName,
    double rotation = 45, double opacity = 0.3,
    double r = 0.5, double g = 0.5, double b = 0.5,
  });
  Future<void> addStamp(int pageIndex, {
    required int stampType,
    String? customName,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  });
  Future<void> addImageStamp(int pageIndex, Uint8List imageBytes, {
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  });
  Future<void> resizeImage(int pageIndex, String imageName,
      {required double width, required double height});

  Future<void> save(PdfSink output);
  Future<void> saveWithOptions(PdfSink output, {bool compress = true,
      bool garbageCollect = true, bool linearize = false});
  Future<void> saveEncrypted(PdfSink output, {required String ownerPassword,
      String userPassword = ''});
  Future<void> saveEncryptedFull(PdfSink output, {
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
  });

  Future<void> dispose();
}

/// Cross-platform handle for creating PDFs from scratch.
abstract interface class PdfBuilderHandle {
  Future<void> setTitle(String value);
  Future<void> setAuthor(String value);
  Future<void> setSubject(String value);
  Future<void> setKeywords(String value);

  Future<PdfPageBuilderHandle> addA4Page();
  Future<PdfPageBuilderHandle> addLetterPage();
  Future<PdfPageBuilderHandle> addPage({required double width, required double height});

  Future<void> build(PdfSink output);
  Future<void> buildEncrypted(PdfSink output, {required String ownerPassword,
      String userPassword = ''});

  Future<void> dispose();
}

/// Cross-platform handle for building a single page.
abstract interface class PdfPageBuilderHandle {
  Future<void> font(String name, double size);
  Future<void> at(double x, double y);
  Future<void> text(String text);
  Future<void> heading(int level, String text);
  Future<void> paragraph(String text);
  Future<void> space(double points);
  Future<void> horizontalRule();
  Future<void> image(Uint8List imageBytes, double x, double y,
      double width, double height, {String altText = ''});
  Future<void> watermark(String text);

  // ── Form fields ────────────────────────────────────────────────────────
  Future<void> textField(String name, double x, double y, double w, double h,
      {String? defaultValue});
  Future<void> checkbox(String name, double x, double y, double w, double h,
      {bool checked = false});
  Future<void> comboBox(String name, double x, double y, double w, double h,
      List<String> options, {String? selected});
  Future<void> pushButton(String name, double x, double y, double w, double h,
      String caption);
  Future<void> signatureField(String name, double x, double y, double w,
      double h);

  // ── Radio group ────────────────────────────────────────────────────────
  Future<void> radioGroup(String name, List<String> values,
      List<double> xs, List<double> ys, List<double> ws, List<double> hs,
      {String? selected});

  // ── Field validation scripts (call after text_field / combo_box) ──────
  Future<void> fieldKeystroke(String script);
  Future<void> fieldFormat(String script);
  Future<void> fieldValidate(String script);
  Future<void> fieldCalculate(String script);

  // ── Links (attach to previous text element) ───────────────────────────
  Future<void> linkUrl(String url);
  Future<void> linkPage(int targetPage);

  // ── Layout ────────────────────────────────────────────────────────────
  Future<void> footnote(String refMark, String noteText);
  Future<void> columns(int columnCount, double gapPt, String text);
  Future<void> newline();
  Future<void> newPageSameSize();

  Future<void> done();
}
