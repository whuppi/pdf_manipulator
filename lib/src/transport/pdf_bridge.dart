// Abstract bridge — NativeBridge and WebBridge extend this.
// Only FFI methods that go to the engine. No one-shot sugar.
// No algorithms. No helpers. Just the wire.
// INTERNAL — not exported from the package.

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/types/pdf_doc.dart';

/// Abstract bridge. Only FFI-level methods.
abstract class PdfBridge {
  // ── Inspect ──
  Future<PdfDoc> open(DataSource source, {String? password});

  // ── Extraction ──
  Future<String> extract(DataSource source,
      {required PdfPages pages,
      String? password,
      PdfExtractionFormat format = PdfExtractionFormat.auto});

  // ── Search ──
  Future<List<SearchResult>> search(DataSource source,
      {required String query, required PdfPages pages, String? password});

  // ── Standalone ops (own engine paths) ──
  Future<void> sign(DataSource source, DataSink output,
      {required PdfSigningCredentials credentials,
      String? reason,
      String? location});

  Future<void> imagesToPdf(List<DataSource> images, DataSink output);

  Future<void> convertTo(DataSource source, DataSink output,
      {required PdfDocumentFormat format, String? password});
  Future<void> convertToPdf(DataSource document, DataSink output,
      {required PdfDocumentFormat format});

  // ── Streaming reads ──
  Stream<RenderedPage> render(DataSource source,
      {required PdfPages pages, PdfRenderSize? size, String? password});

  Stream<PdfImage> extractImages(DataSource source,
      {required PdfPages pages, String? password});

  // ── Read-only queries ──
  Future<List<PdfSignatureInfo>> getSignatures(DataSource source,
      {String? password});
  Future<bool> verifySignatures(DataSource source, {String? password});

  Future<PdfValidationResult> validatePdfA(DataSource source,
      {int level = 2, String? password});
  Future<bool> validatePdfUa(DataSource source,
      {int level = 1, String? password});

  Future<List<PdfBookmarkSplit>> planSplitByBookmarks(DataSource source,
      {String? password});

  Future<PdfPageClassification> classifyPage(DataSource source,
      int page, {String? password});
  Future<PdfDocumentClassification> classifyDocument(DataSource source,
      {String? password});

  // ── Editor ──
  Future<BridgeEditorHandle> openEditor(DataSource source, {String? password});

  // ── Builder ──
  Future<BridgeBuilderHandle> createBuilder();

  // ── Lifecycle ──
  Future<void> dispose();
}

/// Handle to an open PDF editor session.
abstract class BridgeEditorHandle {
  Future<int> get pageCount;
  Future<String> get version;
  Future<bool> get isModified;

  // ── Metadata ──
  Future<String> getTitle();
  Future<void> setTitle(String value);
  Future<String> getAuthor();
  Future<void> setAuthor(String value);
  Future<String> getSubject();
  Future<void> setSubject(String value);
  Future<String> getKeywords();
  Future<void> setKeywords(String value);

  // ── Pages ──
  Future<void> rotatePage(int page, {required int degrees});
  Future<void> rotateAllPages({required int degrees});
  Future<PdfRect> getPageMediaBox(int page);
  Future<void> deletePage(int page);
  Future<void> movePage({required int from, required int to});
  Future<void> selectPages(List<int> pages);
  Future<void> mergeFrom(DataSource otherPdf);

  // ── Optimization ──
  Future<int> optimizeImages({int quality = 75});
  Future<int> unembedStandardFonts();

  // ── Watermark + stamps ──
  Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  });
  Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect,
    double opacity = 1.0,
  });
  Future<void> addImageStamp(int page, DataSource imageData, {
    required PdfRect rect, double opacity = 1.0,
  });

  // ── Content ──
  Future<void> embedFile(String name, DataSource data);
  Future<void> eraseRegions(int page, List<PdfRect> regions);
  Future<void> flattenForms();
  Future<void> flattenAllAnnotations();
  Future<void> setFormFieldValue(String fieldName, String value);
  Future<void> cropMargins({
    double left = 0, double right = 0,
    double top = 0, double bottom = 0,
  });
  Future<void> convertToPdfA({int level = 1});
  Future<void> resizeImage(int page, String imageName, {
    required double width, required double height,
  });

  // ── Redaction ──
  Future<void> addRedaction(int page, PdfRect region, {String? overlayText});
  Future<int> redactionCount(int page);
  Future<void> applyRedactions();
  Future<void> scrubMetadata();

  // ── Save ──
  Future<void> save(DataSink output, {PdfSaveOptions options = const PdfSaveOptions()});

  // ── Lifecycle ──
  Future<void> dispose();
}

/// Handle to a PDF builder session.
abstract class BridgeBuilderHandle {
  Future<void> setTitle(String value);
  Future<void> setAuthor(String value);
  Future<void> setSubject(String value);
  Future<void> setKeywords(String value);

  Future<BridgePageBuilderHandle> addA4Page();
  Future<BridgePageBuilderHandle> addLetterPage();
  Future<BridgePageBuilderHandle> addPage({required double width, required double height});

  Future<void> save(DataSink output);
  Future<void> dispose();
}

/// Handle to a single page being built.
abstract class BridgePageBuilderHandle {
  Future<void> font(String name, double size);
  Future<void> at(double x, double y);
  Future<void> text(String text);
  Future<void> heading(int level, String text);
  Future<void> paragraph(String text);
  Future<void> space(double points);
  Future<void> horizontalRule();
  Future<void> image(DataSource imageData, PdfRect rect, {String altText = ''});
  Future<void> watermark(String text);
  Future<void> textField(String name, PdfRect rect, {String? defaultValue});
  Future<void> checkbox(String name, PdfRect rect, {bool checked = false});
  Future<void> comboBox(String name, PdfRect rect, List<String> options, {String? selected});
  Future<void> pushButton(String name, PdfRect rect, String caption);
  Future<void> signatureField(String name, PdfRect rect);
  Future<void> radioGroup(String name, List<({String value, PdfRect rect})> options, {String? selected});
  Future<void> fieldKeystroke(String script);
  Future<void> fieldFormat(String script);
  Future<void> fieldValidate(String script);
  Future<void> fieldCalculate(String script);
  Future<void> linkUrl(String url);
  Future<void> linkPage(int targetPage);
  Future<void> footnote(String refMark, String noteText);
  Future<void> columns(int columnCount, double gapPt, String text);
  Future<void> newline();
  Future<void> newPageSameSize();
  Future<void> done();
}
