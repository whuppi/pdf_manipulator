// Pdf — every one-shot operation. One instance = one worker + one thread pool.
// From API_GOLD §6.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_editor.dart';
import 'package:pdf_manipulator/src/api/pdf_builder.dart';
import 'package:pdf_manipulator/src/api/pdf_sink.dart';
import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/api/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/api/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/api/types/pdf_config.dart';
import 'package:pdf_manipulator/src/api/types/pdf_params.dart';
import 'package:pdf_manipulator/src/bridge/bridge.dart';
import 'package:pdf_manipulator/src/bridge/bridge_factory.dart';
import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';
import 'package:pdf_manipulator/src/document/pdf_doc.dart';

class Pdf {
  Pdf({PdfConfig? config}) : _bridge = createBridge();

  final PdfBridge _bridge;
  bool _disposed = false;

  void _check() {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
  }

  // ── Static factories ──

  Future<PdfEditor> edit(PdfSource source, {String? password}) async {
    _check();
    final handle = await _bridge.openEditor(source, password: password);
    return PdfEditor.internal(_bridge, handle);
  }

  Future<PdfBuilder> build() async {
    _check();
    final handle = await _bridge.createBuilder();
    return PdfBuilder.internal(_bridge, handle);
  }

  // ── Inspect ──

  Future<PdfDoc> open(PdfSource source, {String? password}) {
    _check();
    return _bridge.open(source, password: password);
  }

  // ── Structural ──

  Future<void> merge(List<PdfSource> inputs, PdfSink output) {
    _check();
    return _bridge.merge(inputs, output);
  }

  Future<void> split(PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int every}) {
    _check();
    return _bridge.split(source, sinkFactory, every: every);
  }

  Future<int> splitBySize(
      PdfSource source, PdfSink Function(int index) sinkFactory,
      {required int maxBytes}) {
    _check();
    return _bridge.splitBySize(source, sinkFactory, maxBytes: maxBytes);
  }

  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages}) {
    _check();
    return _bridge.extractPages(source, output, pages: pages);
  }

  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages}) {
    _check();
    return _bridge.deletePages(source, output, pages: pages);
  }

  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order}) {
    _check();
    return _bridge.reorderPages(source, output, order: order);
  }

  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to}) {
    _check();
    return _bridge.movePage(source, output, from: from, to: to);
  }

  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages}) {
    _check();
    return _bridge.rotatePages(source, output, pages: pages);
  }

  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees}) {
    _check();
    return _bridge.rotateAllPages(source, output, degrees: degrees);
  }

  // ── Content ──

  Future<void> flattenForms(PdfSource source, PdfSink output) {
    _check();
    return _bridge.flattenForms(source, output);
  }

  Future<void> applyRedactions(PdfSource source, PdfSink output) {
    _check();
    return _bridge.applyRedactions(source, output);
  }

  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData}) {
    _check();
    return _bridge.embedFile(source, output, name: name, fileData: fileData);
  }

  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions}) {
    _check();
    return _bridge.eraseRegions(source, output, page: page, regions: regions);
  }

  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75,
      bool garbageCollect = true,
      bool linearize = false}) {
    _check();
    return _bridge.compress(source, output,
        imageQuality: imageQuality,
        garbageCollect: garbageCollect,
        linearize: linearize);
  }

  // ── Extraction (API_GOLD: one method, format + pages) ──

  Future<String> extract(PdfSource source,
      {required PdfPages pages,
      String? password,
      PdfExtractionFormat format = PdfExtractionFormat.auto}) {
    _check();
    return _bridge.extract(source,
        pages: pages, password: password, format: format);
  }

  // ── Search (API_GOLD: one method, query + pages) ──

  Future<List<SearchResult>> search(PdfSource source,
      {required String query, required PdfPages pages, String? password}) {
    _check();
    return _bridge.search(source,
        query: query, pages: pages, password: password);
  }

  // ── Security ──

  Future<void> watermark(PdfSource source, PdfSink output,
      {required String text,
      PdfPages pages = const PdfPages.all(),
      PdfWatermarkStyle style = const PdfWatermarkStyle(),
      PdfWatermarkPosition? position}) {
    _check();
    return _bridge.watermark(source, output,
        text: text, pages: pages, style: style, position: position);
  }

  Future<void> encrypt(PdfSource source, PdfSink output,
      {required PdfEncryptionConfig encryption}) {
    _check();
    return _bridge.encrypt(source, output, encryption: encryption);
  }

  Future<void> decrypt(PdfSource source, PdfSink output,
      {required String password}) {
    _check();
    return _bridge.decrypt(source, output, password: password);
  }

  Future<void> sign(PdfSource source, PdfSink output,
      {required Uint8List certificate,
      required String certificatePassword,
      String? reason,
      String? location}) {
    _check();
    return _bridge.sign(source, output,
        certificate: certificate,
        certificatePassword: certificatePassword,
        reason: reason,
        location: location);
  }

  // ── Stamps ──

  Future<void> addStamp(PdfSource source, PdfSink output,
      {required int page,
      required PdfStampType type,
      required PdfRect rect,
      String? customName,
      double opacity = 1.0}) {
    _check();
    return _bridge.addStamp(source, output,
        page: page,
        type: type,
        rect: rect,
        customName: customName,
        opacity: opacity);
  }

  Future<void> addImageStamp(PdfSource source, PdfSink output,
      {required int page,
      required Uint8List imageBytes,
      required PdfRect rect,
      double opacity = 1.0}) {
    _check();
    return _bridge.addImageStamp(source, output,
        page: page, imageBytes: imageBytes, rect: rect, opacity: opacity);
  }

  // ── Creation ──

  Future<void> imagesToPdf(List<Uint8List> images, PdfSink output) {
    _check();
    return _bridge.imagesToPdf(images, output);
  }

  // ── Rendering (API_GOLD: one method, pages + size) ──

  Stream<RenderedPage> render(PdfSource source,
      {required PdfPages pages, PdfRenderSize? size, String? password}) {
    _check();
    return _bridge.render(source, pages: pages, size: size, password: password);
  }

  // ── Image extraction (API_GOLD: one method, pages) ──

  Stream<PdfImage> extractImages(PdfSource source,
      {required PdfPages pages, String? password}) {
    _check();
    return _bridge.extractImages(source, pages: pages, password: password);
  }

  // ── Signatures ──

  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source,
      {String? password}) {
    _check();
    return _bridge.getSignatures(source, password: password);
  }

  Future<bool> verifySignatures(PdfSource source, {String? password}) {
    _check();
    return _bridge.verifySignatures(source, password: password);
  }

  // ── Validation ──

  Future<PdfValidationResult> validatePdfA(PdfSource source,
      {int level = 2, String? password}) {
    _check();
    return _bridge.validatePdfA(source, level: level, password: password);
  }

  Future<bool> validatePdfUa(PdfSource source,
      {int level = 1, String? password}) {
    _check();
    return _bridge.validatePdfUa(source, level: level, password: password);
  }

  // ── Lifecycle ──

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _bridge.dispose();
  }
}
