// Shared PdfBridge → EngineOp translation.
//
// Every PdfBridge method's args are built here. Both NativeBridge and
// WebBridge call these to construct the op + args, then hand them to
// their platform-specific transport (isolate SendPort / postMessage).
//
// This file is the SINGLE SOURCE OF TRUTH for what args each op takes.
// If native and web disagree on arg shapes, this file is wrong.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/protocol/op.dart';

/// A fully-built operation ready for transport.
class EngineRequest {
  const EngineRequest(this.op, this.args);
  final EngineOp op;
  final Map<String, Object?> args;
}

// ── Inspect ──

EngineRequest openOp({String? password}) =>
    EngineRequest(EngineOp.open, {'password': password});

// ── Structural ──

EngineRequest mergeOp({String? password, List<ByteBuffer>? secondaries}) =>
    EngineRequest(EngineOp.merge, {
      'password': password,
      if (secondaries != null) 'secondaries': secondaries,
    });

EngineRequest extractPagesOp({required List<int> pages}) =>
    EngineRequest(EngineOp.extractPages, {'pages': pages});

EngineRequest deletePagesOp({required List<int> pages}) =>
    EngineRequest(EngineOp.deletePages, {'pages': pages});

EngineRequest reorderPagesOp({required List<int> order}) =>
    EngineRequest(EngineOp.reorderPages, {'order': order});

EngineRequest movePageOp({required int from, required int to}) =>
    EngineRequest(EngineOp.movePage, {'from': from, 'to': to});

EngineRequest rotatePagesOp({required Map<int, int> rotations}) =>
    EngineRequest(EngineOp.rotatePages, {'rotations': rotations});

EngineRequest rotateAllPagesOp({required int degrees}) =>
    EngineRequest(EngineOp.rotateAllPages, {'degrees': degrees});

// ── Content ──

EngineRequest flattenFormsOp() =>
    const EngineRequest(EngineOp.flattenForms, {});

EngineRequest applyRedactionsOp() =>
    const EngineRequest(EngineOp.applyRedactions, {});

EngineRequest embedFileOp({required String name, required Uint8List fileData}) =>
    EngineRequest(EngineOp.embedFile, {'name': name, 'fileData': fileData});

EngineRequest eraseRegionsOp({required int page, required List<PdfRect> regions}) =>
    EngineRequest(EngineOp.eraseRegions, {
      'page': page,
      'regions': encodeRegions(regions),
    });

EngineRequest compressOp({
  int imageQuality = 75,
  bool garbageCollect = true,
  bool linearize = false,
}) => EngineRequest(EngineOp.compress, {
  'imageQuality': imageQuality,
  'garbageCollect': garbageCollect,
  'linearize': linearize,
});

// ── Extraction ──

EngineRequest extractOp({
  required PdfExtractionFormat format,
  int? page,
  String? password,
}) => EngineRequest(EngineOp.extract, {
  'format': encodeExtractionFormat(format),
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

// ── Security ──

EngineRequest watermarkOp({
  required String text,
  PdfWatermarkStyle style = const PdfWatermarkStyle(),
}) => EngineRequest(EngineOp.watermark, encodeWatermarkArgs(text, style));

EngineRequest encryptOp({required PdfEncryptionConfig encryption}) =>
    EngineRequest(EngineOp.encrypt, {
      'ownerPassword': encryption.ownerPassword,
      'userPassword': encryption.userPassword,
    });

EngineRequest decryptOp({required String password}) =>
    EngineRequest(EngineOp.decrypt, {'password': password});

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

// ── Stamps ──

EngineRequest addStampOp({
  required int page,
  required PdfStampType type,
  required PdfRect rect,
  double opacity = 1.0,
}) => EngineRequest(EngineOp.addStamp, {
  'page': page,
  'stampType': type.index,
  ...encodeRectArgs(rect),
  'opacity': opacity,
});

EngineRequest addImageStampOp({
  required int page,
  required Uint8List imageBytes,
  required PdfRect rect,
  double opacity = 1.0,
}) => EngineRequest(EngineOp.addImageStamp, {
  'page': page,
  'imageBytes': imageBytes,
  ...encodeRectArgs(rect),
  'opacity': opacity,
});

// ── Creation ──

EngineRequest imagesToPdfOp({required List<Uint8List> images}) =>
    EngineRequest(EngineOp.imagesToPdf, {'images': images});

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

// ── Signatures / Validation ──

EngineRequest getSignaturesOp({String? password}) =>
    EngineRequest(EngineOp.getSignatures, {'password': password});

EngineRequest verifySignaturesOp({String? password}) =>
    EngineRequest(EngineOp.verifySignatures, {'password': password});

EngineRequest validatePdfAOp({int level = 2, String? password}) =>
    EngineRequest(EngineOp.validatePdfA, {'level': level, 'password': password});

EngineRequest validatePdfUaOp({int level = 1, String? password}) =>
    EngineRequest(EngineOp.validatePdfUa, {'level': level, 'password': password});

// ── Bookmarks ──

EngineRequest planSplitByBookmarksOp({String? password}) =>
    EngineRequest(EngineOp.planSplitByBookmarks, {'password': password});

EngineRequest splitByBookmarksOp({String? password}) =>
    EngineRequest(EngineOp.splitByBookmarks, {'password': password});

// ── Classification ──

EngineRequest classifyPageOp({required int page, String? password}) =>
    EngineRequest(EngineOp.classifyPage, {'page': page, 'password': password});

EngineRequest classifyDocumentOp({String? password}) =>
    EngineRequest(EngineOp.classifyDocument, {'password': password});

// ── Conversion ──

EngineRequest convertToOp({required PdfDocumentFormat format, String? password}) =>
    EngineRequest(EngineOp.convertTo, {'format': format.name, 'password': password});

EngineRequest convertToPdfOp({required PdfDocumentFormat format}) =>
    EngineRequest(EngineOp.convertToPdf, {'format': format.name});

// ── Editor handle ops ──

EngineRequest editorOpenOp({String? password}) =>
    EngineRequest(EngineOp.editorOpen, {'password': password});

EngineRequest editorDisposeOp({required int handleId}) =>
    EngineRequest(EngineOp.editorDispose, {'handleId': handleId});

EngineRequest editorMutateOp({required int handleId, required String editOp, Map<String, Object?> extra = const {}}) =>
    EngineRequest(EngineOp.editorMutate, {'handleId': handleId, 'editOp': editOp, ...extra});

EngineRequest editorSaveOp({required int handleId, PdfSaveOptions options = const PdfSaveOptions()}) =>
    EngineRequest(EngineOp.editorSave, {'handleId': handleId, ...encodeSaveArgs(options)});

EngineRequest editorGetMetadataOp({required int handleId}) =>
    EngineRequest(EngineOp.editorGetMetadata, {'handleId': handleId});

EngineRequest editorPageMediaBoxOp({required int handleId, required int page}) =>
    EngineRequest(EngineOp.editorPageMediaBox, {'handleId': handleId, 'page': page});

EngineRequest editorExtractPagesOp({required int handleId, required List<int> pages}) =>
    EngineRequest(EngineOp.editorExtractPages, {'handleId': handleId, 'pages': pages});

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
