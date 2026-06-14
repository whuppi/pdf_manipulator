// Shared protocol codec — the single source of truth for op argument
// maps and result decoding across native and web bridges.
//
// Division of labor: this file builds and decodes the FIELD MAPS;
// binary_codec.dart owns the byte-level wire format underneath them.
// Adding a field here adds it to both platforms; missing one breaks
// both platforms.
//
// No platform-specific code. No I/O. Pure data transformation.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/bridge/protocol/op.dart';

// ════════════════════════════════════════════════════════════════════
// REQUEST ENCODING — Dart types → Map<String, Object?> for the wire
// ════════════════════════════════════════════════════════════════════

/// Pairs an [EngineOp] with its serialised argument map.
class EngineRequest {
  /// Creates a request with the given [op] and [args].
  const EngineRequest(this.op, this.args);

  /// The operation to execute.
  final EngineOp op;

  /// Key-value argument map sent over the wire.
  final Map<String, Object?> args;
}

// ── Inspect ──

/// Builds an open-document request.
EngineRequest openOp({String? password}) =>
    EngineRequest(EngineOp.open, {'password': password});

// ── Extraction ──

/// Builds a text-extraction request.
EngineRequest extractOp({
  required PdfExtractionFormat format,
  int? page,
  String? password,
}) => EngineRequest(EngineOp.extract, {
  'format': _encodeExtractionFormat(format),
  'password': password,
  if (page != null) 'page': page,
});

/// Builds a full-text search request.
EngineRequest searchOp({required String query, int? page, String? password}) =>
    EngineRequest(EngineOp.search, {
      'query': query,
      'password': password,
      if (page != null) 'page': page,
    });

// ── Standalone write ──

/// Builds a digital-signing request.
EngineRequest signOp({
  required PdfSigningCredentials credentials,
  String? reason,
  String? location,
}) {
  final map = <String, dynamic>{'reason': reason, 'location': location};
  switch (credentials) {
    case PdfPkcs12Credentials(:final data, :final password):
      map['certificate'] = data;
      map['certificatePassword'] = password;
    case PdfPemCredentials(:final certPem, :final keyPem):
      map['certPem'] = certPem;
      map['keyPem'] = keyPem;
  }
  return EngineRequest(EngineOp.sign, map);
}

/// Builds a format-conversion request (PDF → other).
EngineRequest convertToOp({
  required PdfDocumentFormat format,
  String? password,
}) => EngineRequest(EngineOp.convertTo, {
  'format': format.name,
  'password': password,
});

/// Builds a format-conversion request (other → PDF).
EngineRequest convertToPdfOp({required PdfDocumentFormat format}) =>
    EngineRequest(EngineOp.convertToPdf, {'format': format.name});

// ── Streaming ──

/// Builds a page-rendering request.
EngineRequest renderOp({
  required List<int> pageIndices,
  int? maxWidth,
  int? maxHeight,
  String? password,
}) => EngineRequest(EngineOp.render, {
  'pageIndices': pageIndices,
  'password': password,
  if (maxWidth != null) 'maxWidth': maxWidth,
  if (maxHeight != null) 'maxHeight': maxHeight,
});

/// Builds an image-extraction request.
EngineRequest extractImagesOp({
  required List<int> pageIndices,
  String? password,
}) => EngineRequest(EngineOp.extractImages, {
  'pageIndices': pageIndices,
  'password': password,
});

// ── Read-only queries ──

/// Builds a get-signatures request.
EngineRequest getSignaturesOp({String? password}) =>
    EngineRequest(EngineOp.getSignatures, {'password': password});

/// Builds a verify-signatures request.
EngineRequest verifySignaturesOp({String? password}) =>
    EngineRequest(EngineOp.verifySignatures, {'password': password});

/// Builds a PDF/A validation request.
EngineRequest validatePdfAOp({int level = 2, String? password}) =>
    EngineRequest(EngineOp.validatePdfA, {
      'level': level,
      'password': password,
    });

/// Builds a PDF/UA validation request.
EngineRequest validatePdfUaOp({int level = 1, String? password}) =>
    EngineRequest(EngineOp.validatePdfUa, {
      'level': level,
      'password': password,
    });

/// Builds a bookmark-split planning request.
EngineRequest planSplitByBookmarksOp({String? password}) =>
    EngineRequest(EngineOp.planSplitByBookmarks, {'password': password});

/// Builds a page-classification request.
EngineRequest classifyPageOp({required int page, String? password}) =>
    EngineRequest(EngineOp.classifyPage, {'page': page, 'password': password});

/// Builds a document-classification request.
EngineRequest classifyDocumentOp({String? password}) =>
    EngineRequest(EngineOp.classifyDocument, {'password': password});

// ── Editor handle ops ──

/// Builds an editor-open request.
EngineRequest editorOpenOp({String? password}) =>
    EngineRequest(EngineOp.editorOpen, {'password': password});

/// Builds an editor-dispose request.
EngineRequest editorDisposeOp({required int handleId}) =>
    EngineRequest(EngineOp.editorDispose, {'handleId': handleId});

/// Builds an editor-mutate request.
EngineRequest editorMutateOp({
  required int handleId,
  required String editOp,
  Map<String, Object?> extra = const {},
}) => EngineRequest(EngineOp.editorMutate, {
  'handleId': handleId,
  'editOp': editOp,
  ...extra,
});

/// Builds an editor-save request.
EngineRequest editorSaveOp({
  required int handleId,
  PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
}) => EngineRequest(EngineOp.editorSave, {
  'handleId': handleId,
  ...encodeSaveArgs(options),
});

/// Builds an editor get-metadata request.
EngineRequest editorGetMetadataOp({required int handleId}) =>
    EngineRequest(EngineOp.editorGetMetadata, {'handleId': handleId});

/// Builds an editor page-media-box request.
EngineRequest editorPageMediaBoxOp({
  required int handleId,
  required int page,
}) => EngineRequest(EngineOp.editorPageMediaBox, {
  'handleId': handleId,
  'page': page,
});

/// Builds an editor merge-from request.
EngineRequest editorMergeFromOp({
  required int handleId,
  required Uint8List otherBytes,
}) => EngineRequest(EngineOp.editorMergeFrom, {
  'handleId': handleId,
  'otherBytes': otherBytes,
});

// ── Builder handle ops ──

/// Builds a builder-create request.
EngineRequest builderCreateOp() =>
    const EngineRequest(EngineOp.builderCreate, {});

/// Builds a builder-dispose request.
EngineRequest builderDisposeOp({required int handleId}) =>
    EngineRequest(EngineOp.builderDispose, {'handleId': handleId});

/// Builds a builder set-metadata request.
EngineRequest builderSetMetadataOp({
  required int handleId,
  String? title,
  String? author,
  String? subject,
  String? keywords,
}) => EngineRequest(EngineOp.builderSetMetadata, {
  'handleId': handleId,
  if (title != null) 'title': title,
  if (author != null) 'author': author,
  if (subject != null) 'subject': subject,
  if (keywords != null) 'keywords': keywords,
});

/// Builds a builder add-page request.
EngineRequest builderAddPageOp({
  required int handleId,
  String? pageType,
  double? width,
  double? height,
}) => EngineRequest(EngineOp.builderAddPage, {
  'handleId': handleId,
  if (pageType != null) 'pageType': pageType,
  if (width != null) 'width': width,
  if (height != null) 'height': height,
});

/// Builds a builder page-operation request.
EngineRequest builderPageOpReq({
  required int handleId,
  required String pageOp,
  Map<String, Object?> extra = const {},
}) => EngineRequest(EngineOp.builderPageOp, {
  'handleId': handleId,
  'pageOp': pageOp,
  ...extra,
});

/// Builds a builder page-done request.
EngineRequest builderPageDoneOp({required int handleId}) =>
    EngineRequest(EngineOp.builderPageDone, {'handleId': handleId});

/// Builds a builder save request.
EngineRequest builderSaveOp({required int handleId}) =>
    EngineRequest(EngineOp.builderSave, {'handleId': handleId});

// ════════════════════════════════════════════════════════════════════
// RESPONSE DECODING — Map<String, Object?> from the wire → Dart types
// ════════════════════════════════════════════════════════════════════

/// Decodes the page list from an open-document response.
List<PdfPageInfo> decodePageList(Map<String, Object?> r) {
  final pagesRaw = r['pages'] as List? ?? [];
  return pagesRaw.map((p) {
    final m = _asMap(p);
    return PdfPageInfo(
      index: m['index'] as int,
      width: (m['width'] as num).toDouble(),
      height: (m['height'] as num).toDouble(),
      rotation: (m['rotation'] as num?)?.toInt() ?? 0,
    );
  }).toList();
}

/// Decodes the encryption algorithm from a response map.
PdfEncryptionAlgorithm? decodeEncryptionAlgorithmFromMap(
  Map<String, Object?> r,
) {
  final code = r['encryptionAlgorithm'] as int? ?? 0;
  return decodeEncryptionAlgorithm(code);
}

/// Decodes permission flags from a response map.
PdfPermissions? decodePermissionsFromMap(Map<String, Object?> r) {
  final isEncrypted = r['isEncrypted'] as bool? ?? false;
  if (!isEncrypted) return null;
  final bits = r['permissionBits'] as int? ?? 0xFF;
  return decodePermissions(bits);
}

/// Decodes extracted text from a response map.
String decodeExtractResult(Map<String, Object?> r) =>
    r['text'] as String? ?? '';

/// Decodes search hits from a response map.
List<SearchResult> decodeSearchResults(Map<String, Object?> r) {
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

/// Decodes digital-signature info from a response map.
List<PdfSignatureInfo> decodeSignatures(Map<String, Object?> r) {
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

/// Decodes the verify-signatures boolean result.
bool decodeVerifySignatures(Map<String, Object?> r) =>
    r['valid'] as bool? ?? false;

/// Decodes a PDF/A validation result.
PdfValidationResult decodeValidationResult(Map<String, Object?> r) =>
    PdfValidationResult(
      compliant: r['compliant'] as bool? ?? false,
      errors: r['errors'] as int? ?? 0,
      warnings: r['warnings'] as int? ?? 0,
    );

/// Decodes the PDF/UA validation boolean result.
bool decodeValidatePdfUa(Map<String, Object?> r) =>
    r['valid'] as bool? ?? false;

/// Decodes bookmark-based split plan entries.
List<PdfBookmarkSplit> decodeBookmarkSplits(Map<String, Object?> r) {
  final splits = r['splits'] as List? ?? [];
  return splits.map((s) {
    final m = _asMap(s);
    return PdfBookmarkSplit(
      title: m['title'] as String? ?? '',
      startPage: m['startPage'] as int? ?? 0,
      endPage: m['endPage'] as int? ?? 0,
    );
  }).toList();
}

/// Decodes a page-classification result.
PdfPageClassification decodeClassifyPage(Map<String, Object?> r) =>
    PdfPageClassification(
      type: r['type'] as String? ?? 'unknown',
      confidence: (r['confidence'] as num?)?.toDouble() ?? 0,
    );

/// Decodes a document-classification result.
PdfDocumentClassification decodeClassifyDocument(Map<String, Object?> r) =>
    PdfDocumentClassification(
      type: r['type'] as String? ?? 'unknown',
      confidence: (r['confidence'] as num?)?.toDouble() ?? 0,
      pageCount: r['pageCount'] as int? ?? 0,
    );

/// Decodes a rendered-page pixel buffer from a response map.
RenderedPage decodeRenderedPage(Map<String, Object?> data) => RenderedPage(
  width: data['width'] as int? ?? 0,
  height: data['height'] as int? ?? 0,
  data: _extractBytes(data['data']),
);

/// Decodes an extracted image from a response map.
PdfImage decodePdfImage(Map<String, Object?> data) => PdfImage(
  width: data['width'] as int? ?? 0,
  height: data['height'] as int? ?? 0,
  format: data['format'] as String? ?? '',
  colorSpace: data['colorSpace'] as String? ?? '',
  bitsPerComponent: data['bitsPerComponent'] as int? ?? 8,
  data: _extractBytes(data['data']),
);

/// Decodes editor metadata (page count, version, title, etc.).
({
  int pageCount,
  String version,
  String title,
  String author,
  String subject,
  String keywords,
})
decodeEditorMetadata(Map<String, Object?> r) => (
  pageCount: r['pageCount'] as int? ?? 0,
  version: r['version'] as String? ?? '2.0',
  title: r['title'] as String? ?? '',
  author: r['author'] as String? ?? '',
  subject: r['subject'] as String? ?? '',
  keywords: r['keywords'] as String? ?? '',
);

/// Decodes a media-box rectangle from a response map.
PdfRect decodeMediaBox(Map<String, Object?> r) => PdfRect(
  x: (r['x'] as num).toDouble(),
  y: (r['y'] as num).toDouble(),
  width: (r['width'] as num).toDouble(),
  height: (r['height'] as num).toDouble(),
);

/// Decodes the handle ID from an editor-open response.
int decodeEditorOpen(Map<String, Object?> r) => r['handleId'] as int;

// ════════════════════════════════════════════════════════════════════
// HELPERS — shared encoding utilities
// ════════════════════════════════════════════════════════════════════

/// Resolves [PdfPages] to a concrete list of 0-based page indices.
List<int> resolvePageIndices(PdfPages pages, int pageCount) => switch (pages) {
  PdfPagesAll() => List.generate(pageCount, (i) => i),
  PdfPagesSingle(:final index) => [index],
  PdfPagesList(:final indices) => indices,
  PdfPagesRange(:final start, :final end) => List.generate(
    end - start,
    (i) => start + i,
  ),
};

/// Encodes a list of rectangles as a flat doubles list.
List<double> encodeRegions(List<PdfRect> regions) =>
    regions.expand((r) => [r.x, r.y, r.width, r.height]).toList();

/// Encodes watermark arguments into a wire-ready map.
Map<String, Object?> encodeWatermarkArgs(
  String text,
  PdfWatermarkStyle style,
  PdfWatermarkPosition position,
  PdfWatermarkLayer layer,
) => {
  'text': text,
  'opacity': style.opacity,
  'fontSize': style.fontSize,
  'rotation': style.rotation,
  'r': style.color.r,
  'g': style.color.g,
  'b': style.color.b,
  'layer': layer.index,
  ...encodePosition(position),
};

/// Encodes a watermark position into a wire-ready map.
Map<String, Object?> encodePosition(PdfWatermarkPosition position) =>
    switch (position) {
      PdfWatermarkCenter() => {'posType': 0},
      PdfWatermarkCorner(:final corner, :final marginX, :final marginY) => {
        'posType': 1,
        'corner': corner.index,
        'marginX': marginX,
        'marginY': marginY,
      },
      PdfWatermarkTiled(:final columns, :final rows) => {
        'posType': 2,
        'columns': columns,
        'rows': rows,
      },
      PdfWatermarkExact(:final x, :final y, :final width, :final height) => {
        'posType': 3,
        'posX': x,
        'posY': y,
        'posW': width,
        'posH': height,
      },
    };

/// Encodes a [PdfRect] into a wire-ready map.
Map<String, Object?> encodeRectArgs(PdfRect rect) => {
  'x': rect.x,
  'y': rect.y,
  'width': rect.width,
  'height': rect.height,
};

// ── Private helpers ──

/// Encodes save options into a wire-ready map.
Map<String, Object?> encodeSaveArgs(PdfSaveOptions options) {
  return switch (options) {
    PdfSaveFullRewrite(
      :final compress,
      :final garbageCollect,
      :final encryption,
    ) =>
      {
        'saveMode': 0,
        'compress': compress,
        'garbageCollect': garbageCollect,
        ..._encodeEncryption(encryption),
      },
    PdfSaveIncremental() => {
      'saveMode': 1,
      'compress': false,
      'garbageCollect': false,
      ..._encodeEncryption(const PdfEncryption.keep()),
    },
  };
}

Map<String, Object?> _encodeEncryption(PdfEncryption encryption) {
  final encryptMode = switch (encryption) {
    PdfEncryptionKeep() => 0,
    PdfEncryptionRemove() => 1,
    PdfEncryptionConfig() => 2,
  };
  int encAlgo = 0;
  String encUserPw = '';
  String encOwnerPw = '';
  int encPerms = -1;
  if (encryption case final PdfEncryptionConfig c) {
    encAlgo = c.algorithm.index + 1;
    encUserPw = c.userPassword;
    encOwnerPw = c.ownerPassword;
    encPerms = c.permissions.toBits();
  }
  return {
    'encryptMode': encryptMode,
    'encryptAlgo': encAlgo,
    'encryptUserPw': encUserPw,
    'encryptOwnerPw': encOwnerPw,
    'encryptPermissions': encPerms,
  };
}

String _encodeExtractionFormat(PdfExtractionFormat format) => switch (format) {
  PdfExtractionFormat.auto => 'auto',
  PdfExtractionFormat.text => 'text',
  PdfExtractionFormat.markdown => 'markdown',
  PdfExtractionFormat.html => 'html',
  PdfExtractionFormat.plainText => 'plainText',
};

/// Decodes an encryption algorithm from its integer code.
PdfEncryptionAlgorithm? decodeEncryptionAlgorithm(int code) => switch (code) {
  1 => PdfEncryptionAlgorithm.rc4_40,
  2 => PdfEncryptionAlgorithm.rc4_128,
  3 => PdfEncryptionAlgorithm.aes128,
  4 => PdfEncryptionAlgorithm.aes256,
  _ => null,
};

/// Decodes permission bits into a [PdfPermissions] instance.
PdfPermissions decodePermissions(int bits) => PdfPermissions(
  print: bits & 1 != 0,
  printHq: bits & 2 != 0,
  modify: bits & 4 != 0,
  copy: bits & 8 != 0,
  annotate: bits & 16 != 0,
  fillForms: bits & 32 != 0,
  accessibility: bits & 64 != 0,
  assemble: bits & 128 != 0,
);

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
