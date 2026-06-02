// Shared protocol codec — the SINGLE SOURCE OF TRUTH for encoding args
// and decoding results across native and web bridges.
//
// SharedBridge uses these decode functions. Encode side replaced by binary_codec.
// Adding a field here adds it to both platforms.
// Missing a field here breaks both platforms.
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
import 'package:pdf_manipulator/src/transport/protocol/op.dart';

// ════════════════════════════════════════════════════════════════════
// REQUEST ENCODING — Dart types → Map<String, Object?> for the wire
// ════════════════════════════════════════════════════════════════════

class EngineRequest {
  const EngineRequest(this.op, this.args);
  final EngineOp op;
  final Map<String, Object?> args;
}

// ── Inspect ──

EngineRequest openOp({String? password}) =>
    EngineRequest(EngineOp.open, {'password': password});

// ── Extraction ──

EngineRequest extractOp({
  required PdfExtractionFormat format,
  int? page,
  String? password,
}) => EngineRequest(EngineOp.extract, {
  'format': _encodeExtractionFormat(format),
  'password': password,
  if (page != null) 'page': page,
});

EngineRequest searchOp({
  required String query,
  int? page,
  String? password,
}) => EngineRequest(EngineOp.search, {
  'query': query,
  'password': password,
  if (page != null) 'page': page,
});

// ── Standalone write ──

EngineRequest signOp({
  required PdfSigningCredentials credentials,
  String? reason,
  String? location,
}) {
  final map = <String, dynamic>{
    'reason': reason,
    'location': location,
  };
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

EngineRequest convertToOp({required PdfDocumentFormat format, String? password}) =>
    EngineRequest(EngineOp.convertTo, {'format': format.name, 'password': password});

EngineRequest convertToPdfOp({required PdfDocumentFormat format}) =>
    EngineRequest(EngineOp.convertToPdf, {'format': format.name});

// ── Streaming ──

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

EngineRequest extractImagesOp({
  required List<int> pageIndices,
  String? password,
}) => EngineRequest(EngineOp.extractImages, {
  'pageIndices': pageIndices,
  'password': password,
});

// ── Read-only queries ──

EngineRequest getSignaturesOp({String? password}) =>
    EngineRequest(EngineOp.getSignatures, {'password': password});

EngineRequest verifySignaturesOp({String? password}) =>
    EngineRequest(EngineOp.verifySignatures, {'password': password});

EngineRequest validatePdfAOp({int level = 2, String? password}) =>
    EngineRequest(EngineOp.validatePdfA, {'level': level, 'password': password});

EngineRequest validatePdfUaOp({int level = 1, String? password}) =>
    EngineRequest(EngineOp.validatePdfUa, {'level': level, 'password': password});

EngineRequest planSplitByBookmarksOp({String? password}) =>
    EngineRequest(EngineOp.planSplitByBookmarks, {'password': password});

EngineRequest classifyPageOp({required int page, String? password}) =>
    EngineRequest(EngineOp.classifyPage, {'page': page, 'password': password});

EngineRequest classifyDocumentOp({String? password}) =>
    EngineRequest(EngineOp.classifyDocument, {'password': password});

// ── Editor handle ops ──

EngineRequest editorOpenOp({String? password}) =>
    EngineRequest(EngineOp.editorOpen, {'password': password});

EngineRequest editorDisposeOp({required int handleId}) =>
    EngineRequest(EngineOp.editorDispose, {'handleId': handleId});

EngineRequest editorMutateOp({required int handleId, required String editOp, Map<String, Object?> extra = const {}}) =>
    EngineRequest(EngineOp.editorMutate, {'handleId': handleId, 'editOp': editOp, ...extra});

EngineRequest editorSaveOp({required int handleId, PdfSaveOptions options = const PdfSaveOptions.fullRewrite()}) =>
    EngineRequest(EngineOp.editorSave, {'handleId': handleId, ...encodeSaveArgs(options)});

EngineRequest editorGetMetadataOp({required int handleId}) =>
    EngineRequest(EngineOp.editorGetMetadata, {'handleId': handleId});

EngineRequest editorPageMediaBoxOp({required int handleId, required int page}) =>
    EngineRequest(EngineOp.editorPageMediaBox, {'handleId': handleId, 'page': page});

EngineRequest editorMergeFromOp({required int handleId, required Uint8List otherBytes}) =>
    EngineRequest(EngineOp.editorMergeFrom, {'handleId': handleId, 'otherBytes': otherBytes});

// ── Builder handle ops ──

EngineRequest builderCreateOp() =>
    const EngineRequest(EngineOp.builderCreate, {});

EngineRequest builderDisposeOp({required int handleId}) =>
    EngineRequest(EngineOp.builderDispose, {'handleId': handleId});

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

EngineRequest builderAddPageOp({required int handleId, String? pageType, double? width, double? height}) =>
    EngineRequest(EngineOp.builderAddPage, {
      'handleId': handleId,
      if (pageType != null) 'pageType': pageType,
      if (width != null) 'width': width,
      if (height != null) 'height': height,
    });

EngineRequest builderPageOpReq({required int handleId, required String pageOp, Map<String, Object?> extra = const {}}) =>
    EngineRequest(EngineOp.builderPageOp, {'handleId': handleId, 'pageOp': pageOp, ...extra});

EngineRequest builderPageDoneOp({required int handleId}) =>
    EngineRequest(EngineOp.builderPageDone, {'handleId': handleId});

EngineRequest builderSaveOp({required int handleId}) =>
    EngineRequest(EngineOp.builderSave, {'handleId': handleId});

// ════════════════════════════════════════════════════════════════════
// RESPONSE DECODING — Map<String, Object?> from the wire → Dart types
// ════════════════════════════════════════════════════════════════════

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

PdfEncryptionAlgorithm? decodeEncryptionAlgorithmFromMap(Map<String, Object?> r) {
  final code = r['encryptionAlgorithm'] as int? ?? 0;
  return decodeEncryptionAlgorithm(code);
}

PdfPermissions? decodePermissionsFromMap(Map<String, Object?> r) {
  final isEncrypted = r['isEncrypted'] as bool? ?? false;
  if (!isEncrypted) return null;
  final bits = r['permissionBits'] as int? ?? 0xFF;
  return decodePermissions(bits);
}

String decodeExtractResult(Map<String, Object?> r) =>
    r['text'] as String? ?? '';

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

bool decodeVerifySignatures(Map<String, Object?> r) =>
    r['valid'] as bool? ?? false;

PdfValidationResult decodeValidationResult(Map<String, Object?> r) =>
    PdfValidationResult(
      compliant: r['compliant'] as bool? ?? false,
      errors: r['errors'] as int? ?? 0,
      warnings: r['warnings'] as int? ?? 0,
    );

bool decodeValidatePdfUa(Map<String, Object?> r) =>
    r['valid'] as bool? ?? false;

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

PdfPageClassification decodeClassifyPage(Map<String, Object?> r) =>
    PdfPageClassification(
      type: r['type'] as String? ?? 'unknown',
      confidence: (r['confidence'] as num?)?.toDouble() ?? 0,
    );

PdfDocumentClassification decodeClassifyDocument(Map<String, Object?> r) =>
    PdfDocumentClassification(
      type: r['type'] as String? ?? 'unknown',
      confidence: (r['confidence'] as num?)?.toDouble() ?? 0,
      pageCount: r['pageCount'] as int? ?? 0,
    );

RenderedPage decodeRenderedPage(Map<String, Object?> data) =>
    RenderedPage(
      width: data['width'] as int? ?? 0,
      height: data['height'] as int? ?? 0,
      data: _extractBytes(data['data']),
    );

PdfImage decodePdfImage(Map<String, Object?> data) =>
    PdfImage(
      width: data['width'] as int? ?? 0,
      height: data['height'] as int? ?? 0,
      format: data['format'] as String? ?? '',
      colorSpace: data['colorSpace'] as String? ?? '',
      bitsPerComponent: data['bitsPerComponent'] as int? ?? 8,
      data: _extractBytes(data['data']),
    );

({int pageCount, String version, String title, String author, String subject, String keywords})
    decodeEditorMetadata(Map<String, Object?> r) => (
      pageCount: r['pageCount'] as int? ?? 0,
      version: r['version'] as String? ?? '2.0',
      title: r['title'] as String? ?? '',
      author: r['author'] as String? ?? '',
      subject: r['subject'] as String? ?? '',
      keywords: r['keywords'] as String? ?? '',
    );

PdfRect decodeMediaBox(Map<String, Object?> r) => PdfRect(
    x: (r['x'] as num).toDouble(),
    y: (r['y'] as num).toDouble(),
    width: (r['width'] as num).toDouble(),
    height: (r['height'] as num).toDouble(),
  );

int decodeEditorOpen(Map<String, Object?> r) =>
    r['handleId'] as int;

// ════════════════════════════════════════════════════════════════════
// HELPERS — shared encoding utilities
// ════════════════════════════════════════════════════════════════════

List<int> resolvePageIndices(PdfPages pages, int pageCount) => switch (pages) {
  PdfAllPages() => List.generate(pageCount, (i) => i),
  PdfSinglePage(:final index) => [index],
  PdfPageList(:final indices) => indices,
  PdfPageRange(:final start, :final end) => List.generate(end - start, (i) => start + i),
};

List<double> encodeRegions(List<PdfRect> regions) =>
    regions.expand((r) => [r.x, r.y, r.width, r.height]).toList();

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

Map<String, Object?> encodePosition(PdfWatermarkPosition position) => switch (position) {
  PdfWatermarkCenter() => {'posType': 0},
  PdfWatermarkCorner(:final corner, :final marginX, :final marginY) => {
    'posType': 1, 'corner': corner.index, 'marginX': marginX, 'marginY': marginY,
  },
  PdfWatermarkTiled(:final columns, :final rows) => {
    'posType': 2, 'columns': columns, 'rows': rows,
  },
  PdfWatermarkExact(:final x, :final y, :final width, :final height) => {
    'posType': 3, 'posX': x, 'posY': y, 'posW': width, 'posH': height,
  },
};

Map<String, Object?> encodeRectArgs(PdfRect rect) => {
  'x': rect.x, 'y': rect.y, 'width': rect.width, 'height': rect.height,
};

// ── Private helpers ──

Map<String, Object?> encodeSaveArgs(PdfSaveOptions options) {
  return switch (options) {
    PdfSaveFullRewrite(:final compress, :final garbageCollect, :final encryption) => {
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
  if (encryption case PdfEncryptionConfig c) {
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

PdfEncryptionAlgorithm? decodeEncryptionAlgorithm(int code) => switch (code) {
  1 => PdfEncryptionAlgorithm.rc4_40,
  2 => PdfEncryptionAlgorithm.rc4_128,
  3 => PdfEncryptionAlgorithm.aes128,
  4 => PdfEncryptionAlgorithm.aes256,
  _ => null,
};

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
