/// Platform-agnostic contract for all PDF operations.
import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_info.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';

abstract interface class PdfPlatform {
  // ── Inspect ──────────────────────────────────────────────────────────────
  Future<PdfDoc> open(Uint8List bytes, {String? password});
  Future<PdfInfo> probe(Uint8List bytes);

  // ── Structural ───────────────────────────────────────────────────────────
  Future<Uint8List> merge(List<Uint8List> inputs);
  Future<List<Uint8List>> split(Uint8List bytes, {required int every});
  Future<List<Uint8List>> splitBySize(Uint8List bytes, {required int maxBytes});
  Future<Uint8List> extractPages(Uint8List bytes, {required List<int> pages});
  Future<Uint8List> deletePages(Uint8List bytes, {required List<int> pages});
  Future<Uint8List> reorderPages(Uint8List bytes, {required List<int> order});
  Future<Uint8List> movePage(Uint8List bytes,
      {required int from, required int to});
  Future<Uint8List> rotatePages(Uint8List bytes,
      {required Map<int, int> pages});
  Future<Uint8List> rotateAllPages(Uint8List bytes, {required int degrees});

  // ── Content ──────────────────────────────────────────────────────────────
  Future<Uint8List> flattenForms(Uint8List bytes);
  Future<Uint8List> applyRedactions(Uint8List bytes);
  Future<Uint8List> embedFile(Uint8List bytes,
      {required String name, required Uint8List fileData});
  Future<Uint8List> eraseRegions(Uint8List bytes,
      {required int page, required List<PdfRect> regions});
  Future<Uint8List> compress(Uint8List bytes,
      {int imageQuality = 75, bool garbageCollect = true, bool linearize = false});

  // ── Extraction ───────────────────────────────────────────────────────────
  Future<String> extractText(Uint8List bytes, {int? page, String? password});
  Future<String> toMarkdown(Uint8List bytes, {int? page, String? password});
  Future<String> toHtml(Uint8List bytes,
      {required int page, String? password});
  Future<String> toPlainText(Uint8List bytes,
      {required int page, String? password});

  // ── Search ───────────────────────────────────────────────────────────────
  Future<List<SearchResult>> searchPage(Uint8List bytes,
      {required int page, required String query, String? password});
  Future<List<SearchResult>> searchAll(Uint8List bytes,
      {required String query, String? password});

  // ── Security ─────────────────────────────────────────────────────────────
  Future<Uint8List> watermark(Uint8List bytes,
      {required String text,
      List<int>? pages,
      double opacity = 0.3,
      double fontSize = 48,
      double rotation = 45,
      double r = 0.5,
      double g = 0.5,
      double b = 0.5});
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
  });
  Future<Uint8List> encrypt(Uint8List bytes,
      {required String ownerPassword, String userPassword = ''});
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
  });
  Future<Uint8List> decrypt(Uint8List bytes, {required String password});
  Future<Uint8List> sign(Uint8List bytes,
      {required Uint8List certificate,
      required String certificatePassword,
      String? reason,
      String? location});

  // ── Creation ─────────────────────────────────────────────────────────────
  Future<Uint8List> imagesToPdf(List<Uint8List> images);

  // ── Rendering ────────────────────────────────────────────────────────────
  Future<RenderedPage> renderPage(Uint8List bytes, int pageIndex,
      {String? password});
  Future<RenderedPage> renderPageFit(Uint8List bytes, int pageIndex,
      {required int width, required int height, String? password});
  Future<RenderedPage> renderPageThumbnail(Uint8List bytes, int pageIndex,
      {required int size, String? password});
  Future<List<RenderedPage>> renderAllPages(Uint8List bytes,
      {required int width, required int height, String? password});

  // ── Image extraction ─────────────────────────────────────────────────────
  Future<List<PdfImage>> extractImages(Uint8List bytes, int pageIndex,
      {String? password});
  Future<List<PdfImage>> extractAllImages(Uint8List bytes, {String? password});

  // ── Signatures ───────────────────────────────────────────────────────────
  Future<int> getSignatureCount(Uint8List bytes, {String? password});
  Future<List<PdfSignatureInfo>> getSignatures(Uint8List bytes,
      {String? password});
  Future<bool> verifySignatures(Uint8List bytes, {String? password});

  // ── Validation ───────────────────────────────────────────────────────────
  Future<({bool compliant, int errors, int warnings})> validatePdfA(
      Uint8List bytes,
      {int level = 2,
      String? password});
  Future<bool> validatePdfUa(Uint8List bytes,
      {int level = 1, String? password});

  // ── Encryption info ──────────────────────────────────────────────────────
  Future<({bool print, bool printHq, bool modify, bool copy, bool annotate,
      bool fillForms, bool accessibility, bool assemble})>
    getPermissions(Uint8List bytes, {String? password});
  Future<int> getEncryptionAlgorithm(Uint8List bytes, {String? password});

  // ── Editor ───────────────────────────────────────────────────────────────
  Future<PdfEditorHandle> openEditor(Uint8List bytes);

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
  Future<Uint8List> extractPages(List<int> pages);
  Future<void> mergeFrom(Uint8List otherPdf);

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

  Future<Uint8List> save();
  Future<Uint8List> saveWithOptions({bool compress = true,
      bool garbageCollect = true, bool linearize = false});
  Future<Uint8List> saveEncrypted({required String ownerPassword,
      String userPassword = ''});
  Future<Uint8List> saveEncryptedFull({
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

  Future<Uint8List> build();
  Future<Uint8List> buildEncrypted({required String ownerPassword,
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
