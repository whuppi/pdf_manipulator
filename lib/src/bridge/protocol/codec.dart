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

import 'package:pdf_manipulator/src/types/pdf_annotation.dart';
import 'package:pdf_manipulator/src/types/pdf_attachment.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_form_field.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_image_policy.dart';
import 'package:pdf_manipulator/src/types/pdf_image_report.dart';
import 'package:pdf_manipulator/src/types/pdf_matrix.dart';
import 'package:pdf_manipulator/src/types/pdf_page_image.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_report.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/pdf_xfa_info.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';

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
      label: m['label'] as String?,
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
  final bits = r['permissionBits'] as int? ?? -1;
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

/// Decodes the image placements of a page from a response map.
List<PdfPageImage> decodePageImages(Map<String, Object?> r) {
  final images = r['images'] as List? ?? [];
  return images.map((i) {
    final m = _asMap(i);
    double n(String k) => (m[k] as num).toDouble();
    return PdfPageImage(
      name: m['name'] as String? ?? '',
      bounds: PdfRect(
        x: n('x'),
        y: n('y'),
        width: n('width'),
        height: n('height'),
      ),
      transform: PdfMatrix(
        a: n('a'),
        b: n('b'),
        c: n('c'),
        d: n('d'),
        e: n('e'),
        f: n('f'),
      ),
    );
  }).toList();
}

/// Decodes the AcroForm fields of a document from a `formFields` response.
List<PdfFormField> decodeFormFields(Map<String, Object?> r) {
  final fields = r['fields'] as List? ?? [];
  return fields.map((f) {
    final m = _asMap(f);
    final type = _enumByWireName(
      PdfFormFieldType.values,
      m['type'],
      'field type',
    );
    final value = _decodeFormFieldValue(m);
    final tooltip = m['tooltip'] as String? ?? '';
    final maxLength = m['maxLength'] as int? ?? -1;
    final alignment = m['alignment'] as int? ?? -1;
    return PdfFormField(
      name: m['name'] as String? ?? '',
      type: type,
      value: value,
      tooltip: tooltip.isEmpty ? null : tooltip,
      bounds: m['hasBounds'] == true
          ? PdfRect(
              x: (m['x'] as num).toDouble(),
              y: (m['y'] as num).toDouble(),
              width: (m['width'] as num).toDouble(),
              height: (m['height'] as num).toDouble(),
            )
          : null,
      maxLength: maxLength < 0 ? null : maxLength,
      alignment: alignment < 0 ? null : PdfTextAlignment.values[alignment],
      readOnly: m['readOnly'] as bool? ?? false,
      required: m['required'] as bool? ?? false,
    );
  }).toList();
}

/// Decodes an `xfa` response. `null` means the document has no XFA.
PdfXfaInfo? decodeXfaInfo(Map<String, Object?> r) {
  if (r['has'] != true) return null;
  final fieldCount = r['fieldCount'] as int? ?? -1;
  final pageCount = r['pageCount'] as int? ?? -1;
  return PdfXfaInfo(
    fieldCount: fieldCount < 0 ? null : fieldCount,
    pageCount: pageCount < 0 ? null : pageCount,
    fieldTypes: (r['fieldTypes'] as List? ?? const []).cast<String>(),
  );
}

/// Decodes the embedded-file listing from an `attachments` response.
List<PdfAttachment> decodeAttachments(Map<String, Object?> r) {
  final items = r['attachments'] as List? ?? [];
  return items.map((a) {
    final m = _asMap(a);
    final size = (m['size'] as num?)?.toInt() ?? -1;
    final description = m['description'] as String? ?? '';
    final mimeType = m['mimeType'] as String? ?? '';
    return PdfAttachment(
      name: m['name'] as String? ?? '',
      size: size < 0 ? null : size,
      description: description.isEmpty ? null : description,
      mimeType: mimeType.isEmpty ? null : mimeType,
    );
  }).toList();
}

/// Decodes a page's annotation listing from an `annotations` response.
List<PdfAnnotationInfo> decodeAnnotations(Map<String, Object?> r) {
  final items = r['annotations'] as List? ?? [];
  return items.map((a) {
    final m = _asMap(a);
    final subtypeName = m['subtype'] as String? ?? '';
    final contents = m['contents'] as String? ?? '';
    final author = m['author'] as String? ?? '';
    final fieldName = m['fieldName'] as String? ?? '';
    return PdfAnnotationInfo(
      subtype: PdfAnnotationSubtype.fromPdfName(subtypeName),
      subtypeName: subtypeName,
      rect: (m['hasRect'] as bool? ?? false)
          ? PdfRect(
              x: (m['x'] as num?)?.toDouble() ?? 0,
              y: (m['y'] as num?)?.toDouble() ?? 0,
              width: (m['width'] as num?)?.toDouble() ?? 0,
              height: (m['height'] as num?)?.toDouble() ?? 0,
            )
          : null,
      contents: contents.isEmpty ? null : contents,
      author: author.isEmpty ? null : author,
      fieldName: fieldName.isEmpty ? null : fieldName,
    );
  }).toList();
}

PdfFormFieldValue _decodeFormFieldValue(Map<String, Object?> m) =>
    switch (m['valueKind'] as String? ?? 'none') {
      'text' => PdfTextValue(m['text'] as String? ?? ''),
      'checked' => PdfCheckedValue(m['checked'] as bool? ?? false),
      'choice' => PdfChoiceValue(m['text'] as String? ?? ''),
      'multiChoice' => PdfMultiChoiceValue(
        (m['choices'] as List? ?? []).cast<String>(),
      ),
      _ => const PdfNoValue(),
    };

/// Encodes an image policy as flat `reduceImages` arguments. A `null`
/// resolution is omitted, which the engine reads as "never downsample".
Map<String, Object?> encodeImagePolicy(PdfImagePolicy policy) => {
  if (policy.colorPpi != null) 'colorPpi': policy.colorPpi,
  if (policy.grayPpi != null) 'grayPpi': policy.grayPpi,
  if (policy.monoPpi != null) 'monoPpi': policy.monoPpi,
  'downsampleThreshold': policy.downsampleThreshold,
  'jpegQuality': policy.jpegQuality,
  'allowLossy': policy.allowLossy,
  'convertCmykToRgb': policy.convertCmykToRgb,
  'minPixels': policy.minPixels,
  'minSavings': policy.minSavings,
  'chroma': policy.chromaSubsampling.name,
  'recompressJpeg': policy.recompressJpeg,
};

T _enumByWireName<T extends Enum>(List<T> values, Object? wire, String what) {
  final name = wire as String? ?? '';
  for (final v in values) {
    if (v.name == name) return v;
  }
  throw PdfEngineError('unknown $what "$name" on the wire');
}

/// Decodes a `reduceImages` report from a response map.
PdfImageReport decodeImageReport(Map<String, Object?> r) {
  final rows = r['images'] as List? ?? [];
  return PdfImageReport(
    rows.map((i) {
      final m = _asMap(i);
      int n(String k) => (m[k] as num).toInt();
      final ppi = (m['ppiMin'] as num?)?.toDouble() ?? -1;
      final action = _enumByWireName(
        PdfImageAction.values,
        m['action'],
        'action',
      );
      final reason = m['keepReason'] as String? ?? '';
      return PdfImageOutcome(
        objectId: n('objectId'),
        encoding: _enumByWireName(
          PdfImageEncoding.values,
          m['encoding'],
          'encoding',
        ),
        color: _enumByWireName(PdfImageColor.values, m['color'], 'color'),
        indexed: m['indexed'] as bool? ?? false,
        bits: n('bits'),
        width: n('width'),
        height: n('height'),
        hasSoftMask: m['softMask'] as bool? ?? false,
        uses: n('uses'),
        ppiMin: ppi < 0 ? null : ppi,
        action: action,
        keepReason: reason.isEmpty
            ? null
            : _enumByWireName(PdfImageKeepReason.values, reason, 'keep reason'),
        bytesBefore: n('bytesBefore'),
        bytesAfter: n('bytesAfter'),
        widthAfter: n('widthAfter'),
        heightAfter: n('heightAfter'),
      );
    }).toList(),
  );
}

/// Decodes digital-signature info from a response map.
List<PdfSignatureInfo> decodeSignatures(Map<String, Object?> r) {
  final sigsRaw = r['signatures'] as List? ?? [];
  return sigsRaw.map((s) {
    final m = _asMap(s);
    final secs = m['signingTime'] as int?;
    return PdfSignatureInfo(
      signerName: m['signerName'] as String?,
      reason: m['reason'] as String?,
      location: m['location'] as String?,
      signingTime: secs != null
          ? DateTime.fromMillisecondsSinceEpoch(secs * 1000, isUtc: true)
          : null,
      isValid: m['isValid'] as bool? ?? false,
      coversWholeDocument: m['coversWholeDocument'] as bool? ?? false,
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
  String producer,
  String creationDate,
})
decodeEditorMetadata(Map<String, Object?> r) => (
  pageCount: r['pageCount'] as int? ?? 0,
  version: r['version'] as String? ?? '2.0',
  title: r['title'] as String? ?? '',
  author: r['author'] as String? ?? '',
  subject: r['subject'] as String? ?? '',
  keywords: r['keywords'] as String? ?? '',
  producer: r['producer'] as String? ?? '',
  creationDate: r['creationDate'] as String? ?? '',
);

/// Decodes a media-box rectangle from a response map.
PdfRect decodeMediaBox(Map<String, Object?> r) => PdfRect(
  x: (r['x'] as num).toDouble(),
  y: (r['y'] as num).toDouble(),
  width: (r['width'] as num).toDouble(),
  height: (r['height'] as num).toDouble(),
);

/// Decodes a crop-box rectangle from a response map; `null` when the page
/// has no `/CropBox` (`has: false`).
PdfRect? decodeCropBox(Map<String, Object?> r) {
  if (r['has'] != true) return null;
  return PdfRect(
    x: (r['x'] as num).toDouble(),
    y: (r['y'] as num).toDouble(),
    width: (r['width'] as num).toDouble(),
    height: (r['height'] as num).toDouble(),
  );
}

/// Decodes a redaction report from an `applyRedactions` response map.
PdfRedactionReport decodeRedactionReport(Map<String, Object?> r) =>
    PdfRedactionReport(
      regions: r['regions'] as int? ?? 0,
      glyphsRemoved: r['glyphsRemoved'] as int? ?? 0,
      imagesModified: r['imagesModified'] as int? ?? 0,
      imagesRemoved: r['imagesRemoved'] as int? ?? 0,
      pathsPruned: r['pathsPruned'] as int? ?? 0,
      xobjectsSpecialized: r['xobjectsSpecialized'] as int? ?? 0,
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

/// Encodes a [PdfColor] as flat `r`, `g`, `b` wire arguments.
Map<String, Object?> encodeColorArgs(PdfColor color) => {
  'r': color.r,
  'g': color.g,
  'b': color.b,
};

/// Encodes a set of [PdfFormFieldFlag] as the OR of their `/Ff` bits.
int encodeFormFieldFlags(Set<PdfFormFieldFlag> flags) =>
    flags.fold(0, (acc, f) => acc | f.bit);

// ── Private helpers ──

/// Encodes save options into a wire-ready map.
Map<String, Object?> encodeSaveArgs(PdfSaveOptions options) {
  return switch (options) {
    PdfSaveFullRewrite(
      :final compress,
      :final garbageCollect,
      :final objectStreams,
      :final deflateLevel,
      :final mergeDuplicates,
      :final encryption,
    ) =>
      {
        'saveMode': 0,
        'compress': compress,
        'garbageCollect': garbageCollect,
        'objectStreams': objectStreams,
        'deflateLevel': _checkedDeflateLevel(deflateLevel),
        'mergeDuplicates': mergeDuplicates,
        ..._encodeEncryption(encryption),
      },
    PdfSaveIncremental() => {
      'saveMode': 1,
      ..._encodeEncryption(const PdfEncryption.keep()),
    },
  };
}

int _checkedDeflateLevel(int level) {
  if (level < 0 || level > 9) {
    throw ArgumentError.value(level, 'deflateLevel', 'must be 0 to 9');
  }
  return level;
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
  print: bits & (1 << 2) != 0,
  modify: bits & (1 << 3) != 0,
  copy: bits & (1 << 4) != 0,
  annotate: bits & (1 << 5) != 0,
  fillForms: bits & (1 << 8) != 0,
  accessibility: bits & (1 << 9) != 0,
  assemble: bits & (1 << 10) != 0,
  printHq: bits & (1 << 11) != 0,
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
