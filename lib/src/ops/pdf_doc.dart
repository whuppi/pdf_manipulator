// PdfDoc — read-only queries on a parsed PDF document.
//
// Open once, query many times, dispose when done. The Rust engine
// keeps the parsed PDF in memory (handle map) until dispose is called.
// Read-only — no mutations here. Use PdfEditor for mutations,
// PdfStandalone for one-shot export ops.

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';

/// A parsed PDF document — live handle to the Rust engine.
///
/// Created by [Pdf.open]. The document is parsed once and stays in
/// memory until [dispose] is called. All read operations reuse the
/// already-parsed structure — no re-opening.
class PdfDoc {
  PdfDoc.internal(this._handle, {
    required this.pageCount,
    required this.version,
    required this.pages,
    this.title,
    this.author,
    this.subject,
    this.keywords,
    this.isEncrypted = false,
    this.requiresPassword = false,
    this.isTagged = false,
    this.encryptionAlgorithm,
    this.permissions,
  });

  final BridgeDocHandle _handle;
  bool _disposed = false;

  // ── Metadata (populated at open time) ──

  final int pageCount;
  final String version;
  final List<PdfPageInfo> pages;
  final String? title;
  final String? author;
  final String? subject;
  final String? keywords;
  final bool isEncrypted;
  final bool requiresPassword;
  final bool isTagged;
  final PdfEncryptionAlgorithm? encryptionAlgorithm;
  final PdfPermissions? permissions;

  void _check() {
    if (_disposed) throw StateError('This PdfDoc has been disposed');
  }

  // ── Read operations (reuse the already-parsed document) ──

  Future<String> extract({
    required PdfPages pages,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    _check();
    return _handle.extract(pages: pages, format: format);
  }

  Future<List<SearchResult>> search({
    required String query,
    required PdfPages pages,
  }) {
    _check();
    return _handle.search(query: query, pages: pages);
  }

  Stream<RenderedPage> render({
    required PdfPages pages,
    PdfRenderSize? size,
  }) {
    _check();
    return _handle.render(pages: pages, size: size);
  }

  Stream<PdfImage> extractImages({required PdfPages pages}) {
    _check();
    return _handle.extractImages(pages: pages);
  }

  Future<List<PdfSignatureInfo>> getSignatures() {
    _check();
    return _handle.getSignatures();
  }

  Future<bool> verifySignatures() {
    _check();
    return _handle.verifySignatures();
  }

  Future<PdfValidationResult> validatePdfA({int level = 2}) {
    _check();
    return _handle.validatePdfA(level: level);
  }

  Future<bool> validatePdfUa({int level = 1}) {
    _check();
    return _handle.validatePdfUa(level: level);
  }

  Future<List<PdfBookmarkSplit>> planSplitByBookmarks() {
    _check();
    return _handle.planSplitByBookmarks();
  }

  Future<PdfPageClassification> classifyPage(int page) {
    _check();
    return _handle.classifyPage(page);
  }

  Future<PdfDocumentClassification> classifyDocument() {
    _check();
    return _handle.classifyDocument();
  }

  // ── Lifecycle ──

  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _handle.dispose();
  }
}
