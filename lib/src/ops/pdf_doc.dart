// PdfDoc — read-only queries on a parsed PDF document.
//
// Open once, query many times, dispose when done. The Rust engine
// keeps the parsed PDF in memory (handle map) until dispose is called.
// Read-only — no mutations here. Use PdfEditor for mutations,
// PdfStandalone for one-shot export ops.

import 'package:pdf_manipulator/src/ops/pdf.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/trim/record_use_shim.dart';

/// A parsed PDF document — live handle to the Rust engine.
///
/// Created by [Pdf.open]. The document is parsed once and stays in
/// memory until [dispose] is called. All read operations reuse the
/// already-parsed structure — no re-opening.
class PdfDoc {
  /// Internal constructor — use [Pdf.open] to create instances.
  PdfDoc.internal(
    this._handle, {
    required this.pageCount,
    required this.version,
    required this.pages,
    this.title,
    this.author,
    this.subject,
    this.keywords,
    this.producer,
    this.creator,
    this.creationDate,
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

  /// Document producer (the software that produced the PDF), if present.
  final String? producer;

  /// Document creator (the source application), if present.
  final String? creator;

  /// Document creation date as a raw PDF date string
  /// (e.g. `D:20240101120000Z`), if present.
  final String? creationDate;

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
  PdfTask<String> extract({
    required PdfPages pages,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    _check();
    return _handle.extract(pages: pages, format: format);
  }

  /// Searches for [query] text across the specified [pages].
  PdfTask<List<SearchResult>> search({
    required String query,
    required PdfPages pages,
  }) {
    _check();
    return _handle.search(query: query, pages: pages);
  }

  /// Renders the specified [pages] as rasterized images.
  Stream<RenderedPage> render({required PdfPages pages, PdfRenderSize? size}) {
    TrimRecord.op('render');
    _check();
    return _handle.render(pages: pages, size: size);
  }

  /// Extracts embedded images from the specified [pages].
  Stream<PdfImage> extractImages({required PdfPages pages}) {
    _check();
    return _handle.extractImages(pages: pages);
  }

  /// Returns metadata for all digital signatures in the document.
  PdfTask<List<PdfSignatureInfo>> getSignatures() {
    TrimRecord.op('signatures');
    _check();
    return _handle.getSignatures();
  }

  /// Verifies all digital signatures — returns true if all are valid.
  PdfTask<bool> verifySignatures() {
    TrimRecord.op('signatures');
    _check();
    return _handle.verifySignatures();
  }

  /// Validates PDF/A conformance at the given [level] (1, 2, or 3).
  PdfTask<PdfValidationResult> validatePdfA({int level = 2}) {
    TrimRecord.op('pdfa');
    _check();
    return _handle.validatePdfA(level: level);
  }

  /// Validates PDF/UA (accessibility) conformance at the given [level].
  PdfTask<bool> validatePdfUa({int level = 1}) {
    TrimRecord.op('pdfa');
    _check();
    return _handle.validatePdfUa(level: level);
  }

  /// Returns bookmark-based split boundaries for the document.
  PdfTask<List<PdfBookmarkSplit>> planSplitByBookmarks() {
    _check();
    return _handle.planSplitByBookmarks();
  }

  /// Classifies a single [page] by its content type (text, image, mixed).
  PdfTask<PdfPageClassification> classifyPage(int page) {
    _check();
    return _handle.classifyPage(page);
  }

  /// Classifies the entire document by its overall content type.
  PdfTask<PdfDocumentClassification> classifyDocument() {
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
