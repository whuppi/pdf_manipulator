// Pdf — entry point + standalone FFI methods.
// One instance = one bridge = one worker isolate.

import 'package:pdf_manipulator/src/ops/pdf_editor.dart';
import 'package:pdf_manipulator/src/ops/pdf_builder.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/create.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/types/pdf_doc.dart';

class Pdf {
  Pdf({PdfConfig? config}) : _bridge = createBridge(config: config);

  final PdfBridge _bridge;
  bool _disposed = false;

  void _check() {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
  }

  // ── Editor + Builder entry points ──

  Future<PdfEditor> edit(DataSource source, {String? password}) async {
    _check();
    final doc = await open(source, password: password);
    final handle = await _bridge.openEditor(source, password: password);
    return PdfEditor.internal(_bridge, handle,
        sourceEncryption: doc.encryptionAlgorithm,
        sourcePermissions: doc.permissions,
        password: password);
  }

  Future<PdfBuilder> build() async {
    _check();
    final handle = await _bridge.createBuilder();
    return PdfBuilder.internal(_bridge, handle);
  }

  // ── Standalone FFI — read-only ──

  Future<PdfDoc> open(DataSource source, {String? password}) {
    _check();
    return _bridge.open(source, password: password);
  }

  Future<String> extract(DataSource source, {
    required PdfPages pages,
    String? password,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    _check();
    return _bridge.extract(source,
        pages: pages, password: password, format: format);
  }

  Future<List<SearchResult>> search(DataSource source, {
    required String query,
    required PdfPages pages,
    String? password,
  }) {
    _check();
    return _bridge.search(source,
        query: query, pages: pages, password: password);
  }

  Stream<RenderedPage> render(DataSource source, {
    required PdfPages pages,
    PdfRenderSize? size,
    String? password,
  }) {
    _check();
    return _bridge.render(source, pages: pages, size: size, password: password);
  }

  Stream<PdfImage> extractImages(DataSource source, {
    required PdfPages pages,
    String? password,
  }) {
    _check();
    return _bridge.extractImages(source, pages: pages, password: password);
  }

  Future<List<PdfSignatureInfo>> getSignatures(DataSource source, {
    String? password,
  }) {
    _check();
    return _bridge.getSignatures(source, password: password);
  }

  Future<bool> verifySignatures(DataSource source, {String? password}) {
    _check();
    return _bridge.verifySignatures(source, password: password);
  }

  Future<PdfValidationResult> validatePdfA(DataSource source, {
    int level = 2,
    String? password,
  }) {
    _check();
    return _bridge.validatePdfA(source, level: level, password: password);
  }

  Future<bool> validatePdfUa(DataSource source, {
    int level = 1,
    String? password,
  }) {
    _check();
    return _bridge.validatePdfUa(source, level: level, password: password);
  }

  Future<List<PdfBookmarkSplit>> planSplitByBookmarks(DataSource source, {
    String? password,
  }) {
    _check();
    return _bridge.planSplitByBookmarks(source, password: password);
  }

  Future<PdfPageClassification> classifyPage(DataSource source, int page, {
    String? password,
  }) {
    _check();
    return _bridge.classifyPage(source, page, password: password);
  }

  Future<PdfDocumentClassification> classifyDocument(DataSource source, {
    String? password,
  }) {
    _check();
    return _bridge.classifyDocument(source, password: password);
  }

  // ── Standalone FFI — write ──

  Future<void> sign(DataSource source, DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  }) {
    _check();
    return _bridge.sign(source, output,
        credentials: credentials, reason: reason, location: location);
  }

  Future<void> imagesToPdf(List<DataSource> images, DataSink output) {
    _check();
    return _bridge.imagesToPdf(images, output);
  }

  Future<void> convertTo(DataSource source, DataSink output, {
    required PdfDocumentFormat format,
    String? password,
  }) {
    _check();
    return _bridge.convertTo(source, output, format: format, password: password);
  }

  Future<void> convertToPdf(DataSource document, DataSink output, {
    required PdfDocumentFormat format,
  }) {
    _check();
    return _bridge.convertToPdf(document, output, format: format);
  }

  // ── Lifecycle ──

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _bridge.dispose();
  }
}
