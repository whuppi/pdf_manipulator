// Abstract bridge interface — the seam between Layer 1 (api/) and
// Layer 2 (bridge/). Mirrors every method on the Pdf class.
//
// NativeBridge and WebBridge implement this.
//
// Uses API_GOLD types: PdfPages, PdfSaveOptions, PdfExtractionFormat,
// PdfWatermarkStyle, PdfRenderSize, PdfEncryptionConfig, etc.
//
// INTERNAL — not exported from the package.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_sink.dart';
import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/api/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/api/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/api/types/pdf_params.dart';
import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';

/// Abstract bridge. Layer 1 calls this. Layer 2 implements it.
abstract class PdfBridge {
  // ── Inspect ──
  Future<PdfDoc> open(PdfSource source, {String? password});

  // ── Structural ──
  Future<void> merge(List<PdfSource> inputs, PdfSink output);
  Future<void> split(PdfSource source, PdfSink Function(int) sinkFactory,
      {required int every});
  Future<int> splitBySize(PdfSource source, PdfSink Function(int) sinkFactory,
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

  // ── Content ──
  Future<void> flattenForms(PdfSource source, PdfSink output);
  Future<void> applyRedactions(PdfSource source, PdfSink output);
  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData});
  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions});
  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75,
      bool garbageCollect = true,
      bool linearize = false});

  // ── Extraction (API_GOLD: one method, format + pages) ──
  Future<String> extract(PdfSource source,
      {required PdfPages pages,
      String? password,
      PdfExtractionFormat format = PdfExtractionFormat.auto});

  // ── Search (API_GOLD: one method, query + pages) ──
  Future<List<SearchResult>> search(PdfSource source,
      {required String query, required PdfPages pages, String? password});

  // ── Security ──
  Future<void> watermark(PdfSource source, PdfSink output,
      {required String text,
      PdfPages pages = const PdfPages.all(),
      PdfWatermarkStyle style = const PdfWatermarkStyle(),
      PdfWatermarkPosition? position});
  Future<void> encrypt(PdfSource source, PdfSink output,
      {required PdfEncryptionConfig encryption});
  Future<void> decrypt(PdfSource source, PdfSink output,
      {required String password});
  Future<void> sign(PdfSource source, PdfSink output,
      {required Uint8List certificate,
      required String certificatePassword,
      String? reason,
      String? location});

  // ── Stamps ──
  Future<void> addStamp(PdfSource source, PdfSink output,
      {required int page,
      required PdfStampType type,
      required PdfRect rect,
      String? customName,
      double opacity = 1.0});
  Future<void> addImageStamp(PdfSource source, PdfSink output,
      {required int page,
      required Uint8List imageBytes,
      required PdfRect rect,
      double opacity = 1.0});

  // ── Creation ──
  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output);

  // ── Rendering (API_GOLD: one method, pages + size) ──
  Stream<RenderedPage> render(PdfSource source,
      {required PdfPages pages, PdfRenderSize? size, String? password});

  // ── Image extraction (API_GOLD: one method, pages) ──
  Stream<PdfImage> extractImages(PdfSource source,
      {required PdfPages pages, String? password});

  // ── Signatures ──
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
      {String? password});
  Future<bool> verifySignatures(PdfSource source, {String? password});

  // ── Validation ──
  Future<PdfValidationResult> validatePdfA(PdfSource source,
      {int level = 2, String? password});
  Future<bool> validatePdfUa(PdfSource source,
      {int level = 1, String? password});

  // ── Editor ──
  Future<BridgeEditorHandle> openEditor(PdfSource source, {String? password});

  // ── Builder ──
  Future<BridgeBuilderHandle> createBuilder();

  // ── Lifecycle ──
  Future<void> dispose();
}

/// Handle to an open PDF editor session. Both NativeBridge and WebBridge
/// implement this with their own handle management.
abstract class BridgeEditorHandle {
  Future<int> get pageCount;
  Future<String> get version;

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
  Future<void> extractPages(List<int> pages, PdfSink output);
  Future<void> mergeFrom(PdfSource otherPdf);

  // ── Optimization ──
  Future<int> optimizeImages({int quality = 75});
  Future<int> unembedStandardFonts();

  // ── Watermark + stamps ──
  Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  });
  Future<void> addStamp(int page, {
    required PdfStampType type, required PdfRect rect,
    String? customName, double opacity = 1.0,
  });
  Future<void> addImageStamp(int page, Uint8List imageBytes, {
    required PdfRect rect, double opacity = 1.0,
  });

  // ── Content ──
  Future<void> embedFile(String name, Uint8List data);
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

  // ── Save ──
  Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()});

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

  Future<void> save(PdfSink output, {PdfSaveOptions options = const PdfSaveOptions()});
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
  Future<void> image(Uint8List imageBytes, PdfRect rect, {String altText = ''});
  Future<void> watermark(String text);
  Future<void> textField(String name, PdfRect rect, {String? defaultValue});
  Future<void> checkbox(String name, PdfRect rect, {bool checked = false});
  Future<void> comboBox(String name, PdfRect rect, List<String> options, {String? selected});
  Future<void> pushButton(String name, PdfRect rect, String caption);
  Future<void> signatureField(String name, PdfRect rect);
  Future<void> newline();
  Future<void> newPageSameSize();
  Future<void> done();
}

/// Counts bytes written without storing them.
/// Used by splitBySize to trial-check chunk sizes.
class ByteCountSink implements PdfSink {
  int _length = 0;
  int get length => _length;

  @override
  void write(Uint8List chunk) {
    _length += chunk.length;
  }
}
