// Shared result parsing — decodes engine results into API types.
//
// Both NativeBridge and WebBridge call these to parse results from
// the engine worker. Same parsing logic, zero duplication.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_doc.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';

/// Parse an 'open' result into PdfDoc.
PdfDoc parseOpenResult(Map<String, Object?> r) {
  final pagesRaw = r['pages'] as List? ?? [];
  final pages = pagesRaw.map((p) {
    final m = _asMap(p);
    return PdfPageInfo(
      index: m['index'] as int,
      width: (m['width'] as num).toDouble(),
      height: (m['height'] as num).toDouble(),
      rotation: (m['rotation'] as num?)?.toInt() ?? 0,
    );
  }).toList();
  return PdfDoc(
    pageCount: r['pageCount'] as int,
    version: r['version'] as String? ?? '2.0',
    pages: pages,
    title: r['title'] as String?,
    author: r['author'] as String?,
    isTagged: r['isTagged'] as bool? ?? false,
    isEncrypted: r['isEncrypted'] as bool? ?? false,
  );
}

/// Parse a search result list.
List<SearchResult> parseSearchResults(Map<String, Object?> r) {
  final hits = r['hits'] as List? ?? [];
  return hits.map((h) {
    final m = _asMap(h);
    return SearchResult(
      page: m['page'] as int,
      text: m['text'] as String? ?? '',
      rect: PdfRect(
        x: (m['x'] as num).toDouble(),
        y: (m['y'] as num).toDouble(),
        width: (m['width'] as num).toDouble(),
        height: (m['height'] as num).toDouble(),
      ),
    );
  }).toList();
}

/// Parse a signatures result list.
List<PdfSignatureInfo> parseSignatures(Map<String, Object?> r) {
  final sigsRaw = r['signatures'] as List? ?? [];
  return sigsRaw.map((s) {
    final m = _asMap(s);
    final timeStr = m['signingTime'] as String?;
    return PdfSignatureInfo(
      signerName: m['signerName'] as String?,
      reason: m['reason'] as String?,
      location: m['location'] as String?,
      signingTime: timeStr != null ? DateTime.tryParse(timeStr) : null,
      isValid: m['isValid'] as bool? ?? false,
    );
  }).toList();
}

/// Parse a validation result.
PdfValidationResult parseValidationResult(Map<String, Object?> r) {
  return PdfValidationResult(
    compliant: r['compliant'] as bool? ?? false,
    errors: r['errors'] as int? ?? 0,
    warnings: r['warnings'] as int? ?? 0,
  );
}

/// Parse a rendered page from a stream item.
RenderedPage parseRenderedPage(Map<String, Object?> data) {
  return RenderedPage(
    width: data['width'] as int? ?? 0,
    height: data['height'] as int? ?? 0,
    data: _extractBytes(data['data']),
  );
}

/// Parse an extracted image from a stream item.
PdfImage parsePdfImage(Map<String, Object?> data) {
  return PdfImage(
    width: data['width'] as int? ?? 0,
    height: data['height'] as int? ?? 0,
    format: data['format'] as String? ?? '',
    colorSpace: data['colorSpace'] as String? ?? '',
    bitsPerComponent: data['bitsPerComponent'] as int? ?? 8,
    data: _extractBytes(data['data']),
  );
}

/// Parse editor metadata.
({int pageCount, String version, String title, String author, String subject, String keywords})
    parseEditorMetadata(Map<String, Object?> r) {
  return (
    pageCount: r['pageCount'] as int? ?? 0,
    version: r['version'] as String? ?? '2.0',
    title: r['title'] as String? ?? '',
    author: r['author'] as String? ?? '',
    subject: r['subject'] as String? ?? '',
    keywords: r['keywords'] as String? ?? '',
  );
}

/// Parse a page media box.
PdfRect parseMediaBox(Map<String, Object?> r) {
  return PdfRect(
    x: (r['x'] as num).toDouble(),
    y: (r['y'] as num).toDouble(),
    width: (r['width'] as num).toDouble(),
    height: (r['height'] as num).toDouble(),
  );
}

// ── Helpers ──

Map<String, Object?> _asMap(Object? obj) {
  if (obj is Map<String, Object?>) return obj;
  if (obj is Map) return obj.map((k, v) => MapEntry(k.toString(), v));
  return {};
}

Uint8List _extractBytes(Object? data) {
  if (data is Uint8List) return data;
  if (data is ByteBuffer) return Uint8List.view(data);
  return Uint8List(0);
}
