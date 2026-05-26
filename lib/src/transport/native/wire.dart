// Native wire decoder — binary bytes → typed Dart results.
//
// Translates Rust FFI binary responses (Uint8List, little-endian) into
// the same typed results that the consumer API expects. Calls codec.dart
// internally for the Map→typed step.
//
// Symmetric with web/wire.dart which does Map→typed (no binary step).

import 'dart:convert';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/transport/protocol/codec.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_doc.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';

// ── Open ──

PdfDoc wireDecodeOpen(Uint8List bytes) {
  return decodeOpenResult(_binaryToOpenMap(bytes));
}

// ── Text extraction ──

String wireDecodeText(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] == 0) throw StateError('Read operation failed');
  final bd = ByteData.sublistView(bytes);
  final textLen = bd.getUint32(1, Endian.little);
  return utf8.decode(bytes.sublist(5, 5 + textLen));
}

// ── Search ──

List<SearchResult> wireDecodeSearch(Uint8List bytes) {
  return decodeSearchResults(_binaryToSearchMap(bytes));
}

// ── Signatures ──

List<PdfSignatureInfo> wireDecodeSignatures(Uint8List bytes) {
  return decodeSignatures(_binaryToSignaturesMap(bytes));
}

// ── Verify signatures ──

bool wireDecodeVerifySignatures(Uint8List bytes) {
  return bytes.length > 1 && bytes[1] == 1;
}

// ── Validate PDF/A ──

PdfValidationResult wireDecodeValidation(Uint8List bytes) {
  return decodeValidationResult(_binaryToValidationMap(bytes));
}

// ── Validate PDF/UA ──

bool wireDecodeValidatePdfUa(Uint8List bytes) {
  return bytes.isNotEmpty && bytes[0] == 1 && bytes.length > 1 && bytes[1] == 1;
}

// ── Bookmark splits ──

List<PdfBookmarkSplit> wireDecodeBookmarkSplits(Uint8List bytes) {
  return decodeBookmarkSplits(_binaryToBookmarksMap(bytes));
}

// ── Classification ──

PdfPageClassification wireDecodeClassifyPage(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] != 1) {
    return const PdfPageClassification(type: 'unknown', confidence: 0);
  }
  final text = wireDecodeText(bytes);
  return PdfPageClassification(type: text, confidence: 1);
}

PdfDocumentClassification wireDecodeClassifyDocument(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] != 1) {
    return const PdfDocumentClassification(type: 'unknown', confidence: 0, pageCount: 0);
  }
  final text = wireDecodeText(bytes);
  return PdfDocumentClassification(type: text, confidence: 1, pageCount: 0);
}

// ── Rendered page (from streaming items) ──

RenderedPage wireDecodeRenderedPage(Uint8List itemBytes) {
  return decodeRenderedPage(_binaryToRenderedPageMap(itemBytes));
}

// ── Extracted image (from streaming items) ──

PdfImage wireDecodeImage(Uint8List itemBytes) {
  return decodePdfImage(_binaryToImageMap(itemBytes));
}

// ── Editor metadata ──

({int pageCount, String version, String title, String author, String subject, String keywords})
    wireDecodeEditorMetadata(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] != 1) throw StateError('Metadata read failed');
  final bd = ByteData.sublistView(bytes);
  final vMajor = bd.getUint8(1);
  final vMinor = bd.getUint8(2);
  var off = 7;
  String readStr() {
    final len = bd.getUint16(off, Endian.little); off += 2;
    final s = utf8.decode(bytes.sublist(off, off + len)); off += len;
    return s;
  }
  return (
    pageCount: bd.getInt32(3, Endian.little),
    version: '$vMajor.$vMinor',
    title: readStr(),
    author: readStr(),
    subject: readStr(),
    keywords: readStr(),
  );
}

// ── Error decoding ──

PdfError wireDecodeError(Uint8List bytes) {
  if (bytes.isEmpty) return const PdfEngineError('Unknown error');
  final bd = ByteData.sublistView(bytes);
  final code = bytes.length >= 5 ? bd.getInt32(1, Endian.little) : 0;
  String msg = 'Error code $code';
  if (bytes.length >= 7) {
    final msgLen = bd.getUint16(5, Endian.little);
    if (bytes.length >= 7 + msgLen) {
      msg = utf8.decode(bytes.sublist(7, 7 + msgLen));
    }
  }
  return switch (code) {
    1 => PdfInvalidArgument(msg),
    2 => PdfIoError(msg),
    3 => PdfCorrupted(msg),
    _ => PdfEngineError(msg),
  };
}

// ══════════════════════════════════════════════════════════════════════
// Private: binary → Map (intermediate step before codec)
// ══════════════════════════════════════════════════════════════════════

Map<String, Object?> _binaryToOpenMap(Uint8List bytes) {
  final data = ByteData.sublistView(bytes);
  var offset = 0;

  final status = bytes[offset]; offset += 1;
  if (status == 0) _throwWireError(bytes, data, offset);

  final pageCount = data.getInt32(offset, Endian.little); offset += 4;
  final major = bytes[offset]; offset += 1;
  final minor = bytes[offset]; offset += 1;
  final isEncrypted = bytes[offset] != 0; offset += 1;
  final requiresPassword = bytes[offset] != 0; offset += 1;
  final isTagged = bytes[offset] != 0; offset += 1;
  final encAlgo = bytes[offset]; offset += 1;
  final permBits = bytes[offset]; offset += 1;

  final pages = <Map<String, Object?>>[];
  for (var i = 0; i < pageCount; i++) {
    final width = data.getFloat64(offset, Endian.little); offset += 8;
    final height = data.getFloat64(offset, Endian.little); offset += 8;
    final rotation = data.getInt32(offset, Endian.little); offset += 4;
    pages.add({'index': i, 'width': width, 'height': height, 'rotation': rotation});
  }

  String readStr() {
    final len = data.getUint16(offset, Endian.little); offset += 2;
    if (len == 0) return '';
    final s = String.fromCharCodes(bytes, offset, offset + len);
    offset += len;
    return s;
  }

  final title = readStr();
  final author = readStr();
  final subject = readStr();
  final keywords = readStr();

  return {
    'pageCount': pageCount,
    'version': '$major.$minor',
    'pages': pages,
    'title': title.isEmpty ? null : title,
    'author': author.isEmpty ? null : author,
    'subject': subject.isEmpty ? null : subject,
    'keywords': keywords.isEmpty ? null : keywords,
    'isTagged': isTagged,
    'isEncrypted': isEncrypted,
    'requiresPassword': requiresPassword,
    'encryptionAlgorithm': encAlgo,
    'permissionBits': permBits,
  };
}

Map<String, Object?> _binaryToSearchMap(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] == 0) throw StateError('Search failed');
  final rbd = ByteData.sublistView(bytes);
  final count = rbd.getInt32(1, Endian.little);
  var off = 5;
  final hits = <Map<String, Object?>>[];
  for (var i = 0; i < count; i++) {
    final page = rbd.getInt32(off, Endian.little); off += 4;
    final x = rbd.getFloat32(off, Endian.little); off += 4;
    final y = rbd.getFloat32(off, Endian.little); off += 4;
    final w = rbd.getFloat32(off, Endian.little); off += 4;
    final h = rbd.getFloat32(off, Endian.little); off += 4;
    final textLen = rbd.getUint16(off, Endian.little); off += 2;
    final text = utf8.decode(bytes.sublist(off, off + textLen)); off += textLen;
    hits.add({
      'page': page, 'text': text,
      'x': x.toDouble(), 'y': y.toDouble(),
      'width': w.toDouble(), 'height': h.toDouble(),
    });
  }
  return {'hits': hits};
}

Map<String, Object?> _binaryToSignaturesMap(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] == 0) throw StateError('Signatures read failed');
  final rbd = ByteData.sublistView(bytes);
  final count = rbd.getInt32(1, Endian.little);
  var off = 5;
  final sigs = <Map<String, Object?>>[];
  for (var i = 0; i < count; i++) {
    final nameLen = rbd.getUint16(off, Endian.little); off += 2;
    final name = utf8.decode(bytes.sublist(off, off + nameLen)); off += nameLen;
    final reasonLen = rbd.getUint16(off, Endian.little); off += 2;
    final reason = utf8.decode(bytes.sublist(off, off + reasonLen)); off += reasonLen;
    final locLen = rbd.getUint16(off, Endian.little); off += 2;
    final loc = utf8.decode(bytes.sublist(off, off + locLen)); off += locLen;
    sigs.add({
      'signerName': name.isEmpty ? null : name,
      'reason': reason.isEmpty ? null : reason,
      'location': loc.isEmpty ? null : loc,
      'isValid': false,
    });
  }
  return {'signatures': sigs};
}

Map<String, Object?> _binaryToValidationMap(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] == 0) throw StateError('Validation failed');
  final bd = ByteData.sublistView(bytes);
  return {
    'compliant': bytes[1] == 1,
    'errors': bd.getInt32(2, Endian.little),
    'warnings': bd.getInt32(6, Endian.little),
  };
}

Map<String, Object?> _binaryToBookmarksMap(Uint8List bytes) {
  if (bytes.isEmpty || bytes[0] != 1) return {'splits': <Map<String, Object?>>[]};
  final bd = ByteData.sublistView(bytes);
  final count = bd.getInt32(1, Endian.little);
  final splits = <Map<String, Object?>>[];
  var off = 5;
  for (var i = 0; i < count; i++) {
    final titleLen = bd.getInt32(off, Endian.little); off += 4;
    final title = utf8.decode(bytes.sublist(off, off + titleLen)); off += titleLen;
    final startPage = bd.getInt32(off, Endian.little); off += 4;
    final endPage = bd.getInt32(off, Endian.little); off += 4;
    splits.add({'title': title, 'startPage': startPage, 'endPage': endPage});
  }
  return {'splits': splits};
}

Map<String, Object?> _binaryToRenderedPageMap(Uint8List itemBytes) {
  final ibd = ByteData.sublistView(itemBytes);
  return {
    'width': ibd.getInt32(1, Endian.little),
    'height': ibd.getInt32(5, Endian.little),
    'data': Uint8List.sublistView(itemBytes, 9),
  };
}

Map<String, Object?> _binaryToImageMap(Uint8List itemBytes) {
  final ibd = ByteData.sublistView(itemBytes);
  var offset = 1;
  final w = ibd.getInt32(offset, Endian.little); offset += 4;
  final h = ibd.getInt32(offset, Endian.little); offset += 4;
  final fmtLen = itemBytes[offset]; offset += 1;
  final format = String.fromCharCodes(itemBytes, offset, offset + fmtLen); offset += fmtLen;
  final csLen = itemBytes[offset]; offset += 1;
  final colorSpace = String.fromCharCodes(itemBytes, offset, offset + csLen); offset += csLen;
  final bpc = ibd.getInt32(offset, Endian.little); offset += 4;
  final dataLen = ibd.getInt32(offset, Endian.little); offset += 4;
  final data = Uint8List.sublistView(itemBytes, offset, offset + dataLen);
  return {
    'width': w, 'height': h, 'format': format,
    'colorSpace': colorSpace, 'bitsPerComponent': bpc, 'data': data,
  };
}

Never _throwWireError(Uint8List bytes, ByteData data, int offset) {
  final errorCode = data.getInt32(offset, Endian.little); offset += 4;
  final msgLen = data.getUint16(offset, Endian.little); offset += 2;
  final message = String.fromCharCodes(bytes, offset, offset + msgLen);
  throw StateError('PDF open failed (code $errorCode): $message');
}
