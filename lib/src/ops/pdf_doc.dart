// PdfDoc — read-only queries on a parsed PDF document.
//
// Open once, query many times, dispose when done. The Rust engine
// keeps the parsed PDF in memory (handle map) until dispose is called.
// Read-only — no mutations here. Use PdfEditor for mutations,
// PdfStandalone for one-shot export ops.

import 'package:pdf_manipulator/src/ops/pdf.dart';
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
  /// Internal constructor — use [Pdf.open] to create instances.
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

  /// Total number of pages in the document.
  final int pageCount;

  /// PDF version string (e.g. "1.7", "2.0").
  final String version;

  /// Per-page metadata (dimensions, rotation, label).
  final List<PdfPageInfo> pages;

  /// Document title from metadata, if present.
  final String? title;

  /// Document author from metadata, if present.
  final String? author;

  /// Document subject from metadata, if present.
  final String? subject;

  /// Document keywords from metadata, if present.
  final String? keywords;

  /// Whether the document is encrypted.
  final bool isEncrypted;

  /// Whether a password is required to open the document.
  final bool requiresPassword;

  /// Whether the document contains tagged (accessible) structure.
  final bool isTagged;

  /// Encryption algorithm used, if the document is encrypted.
  final PdfEncryptionAlgorithm? encryptionAlgorithm;

  /// Permission flags, if the document is encrypted.
  final PdfPermissions? permissions;

  void _check() {
    if (_disposed) throw StateError('This PdfDoc has been disposed');
  }

  // ── Read operations (reuse the already-parsed document) ──

  /// Extracts text from the specified [pages] in the given [format].
  Future<String> extract({
    required PdfPages pages,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    _check();
    return _handle.extract(pages: pages, format: format);
  }

  /// Searches for [query] text across the specified [pages].
  Future<List<SearchResult>> search({
    required String query,
    required PdfPages pages,
  }) {
    _check();
    return _handle.search(query: query, pages: pages);
  }

  /// Renders the specified [pages] as rasterized images.
  Stream<RenderedPage> render({
    required PdfPages pages,
    PdfRenderSize? size,
  }) {
    _check();
    return _handle.render(pages: pages, size: size);
  }

  /// Extracts embedded images from the specified [pages].
  Stream<PdfImage> extractImages({required PdfPages pages}) {
    _check();
    return _handle.extractImages(pages: pages);
  }

  /// Returns metadata for all digital signatures in the document.
  Future<List<PdfSignatureInfo>> getSignatures() {
    _check();
    return _handle.getSignatures();
  }

  /// Verifies all digital signatures — returns true if all are valid.
  Future<bool> verifySignatures() {
    _check();
    return _handle.verifySignatures();
  }

  /// Validates PDF/A conformance at the given [level] (1, 2, or 3).
  Future<PdfValidationResult> validatePdfA({int level = 2}) {
    _check();
    return _handle.validatePdfA(level: level);
  }

  /// Validates PDF/UA (accessibility) conformance at the given [level].
  Future<bool> validatePdfUa({int level = 1}) {
    _check();
    return _handle.validatePdfUa(level: level);
  }

  /// Returns bookmark-based split boundaries for the document.
  Future<List<PdfBookmarkSplit>> planSplitByBookmarks() {
    _check();
    return _handle.planSplitByBookmarks();
  }

  /// Classifies a single [page] by its content type (text, image, mixed).
  Future<PdfPageClassification> classifyPage(int page) {
    _check();
    return _handle.classifyPage(page);
  }

  /// Classifies the entire document by its overall content type.
  Future<PdfDocumentClassification> classifyDocument() {
    _check();
    return _handle.classifyDocument();
  }

  // ── Lifecycle ──

  /// Releases the parsed document handle. Idempotent.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _handle.dispose();
  }
}
