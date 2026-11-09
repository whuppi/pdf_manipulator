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

import 'package:pdf_manipulator/src/bridge/protocol/pdf_date.dart';
import 'package:pdf_manipulator/src/types/pdf_annotation.dart';
import 'package:pdf_manipulator/src/types/pdf_attachment.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_form_field.dart';
import 'package:pdf_manipulator/src/types/pdf_image_info.dart';
import 'package:pdf_manipulator/src/types/pdf_image_policy.dart';
import 'package:pdf_manipulator/src/types/pdf_image_report.dart';
import 'package:pdf_manipulator/src/types/pdf_document_info.dart';
import 'package:pdf_manipulator/src/types/pdf_matrix.dart';
import 'package:pdf_manipulator/src/types/pdf_metadata.dart';
import 'package:pdf_manipulator/src/types/pdf_image_placement.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_overlay.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_report.dart';
import 'package:pdf_manipulator/src/types/pdf_render.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/pdf_wire.dart';
import 'package:pdf_manipulator/src/types/pdf_xfa_info.dart';
import 'package:pdf_manipulator/src/types/pdf_search_hit.dart';

// ════════════════════════════════════════════════════════════════════
// RESPONSE DECODING — Map<String, Object?> from the wire → Dart types
// ════════════════════════════════════════════════════════════════════

/// Decodes the per-page facts of a `pageInfo` response.
List<PdfPageInfo> decodePageInfos(Map<String, Object?> r) {
  final pagesRaw = r['pages'] as List? ?? [];
  return pagesRaw.map((p) {
    final m = _asMap(p);
    return PdfPageInfo(
      page: m['index'] as int,
      mediaBox: PdfRect(
        x: (m['x'] as num).toDouble(),
        y: (m['y'] as num).toDouble(),
        width: (m['width'] as num).toDouble(),
        height: (m['height'] as num).toDouble(),
      ),
      cropBox: m['hasCrop'] == true
          ? PdfRect(
              x: (m['cropX'] as num).toDouble(),
              y: (m['cropY'] as num).toDouble(),
              width: (m['cropWidth'] as num).toDouble(),
              height: (m['cropHeight'] as num).toDouble(),
            )
          : null,
      rotation: _enumByWireName(
        PdfRotation.values,
        m['rotation'] ?? PdfRotation.r0.wire,
        'rotation',
      ),
      label: m['label'] as String?,
    );
  }).toList();
}

/// Decodes the metadata of an open document: a field the file does not
/// carry comes back null, not empty.
PdfMetadata decodeMetadata(Map<String, Object?> r) => PdfMetadata(
  title: _text(r['title']),
  author: _text(r['author']),
  subject: _text(r['subject']),
  keywords: _text(r['keywords']),
  creator: _text(r['creator']),
  producer: _text(r['producer']),
  creationDate: decodeDate(r['creationDate']),
  modificationDate: decodeDate(r['modDate']),
);

/// Decodes how an open document is encrypted, or null when it is not
/// encrypted.
PdfEncryptionInfo? decodeEncryptionInfo(Map<String, Object?> r) {
  if (r['isEncrypted'] != true) return null;
  final algorithm = decodeEncryptionAlgorithmFromMap(r);
  if (algorithm == null) return null;
  return PdfEncryptionInfo(
    algorithm: algorithm,
    permissions: decodePermissions(r['permissionBits'] as int? ?? -1),
    requiresPassword: r['requiresPassword'] as bool? ?? false,
  );
}

/// A wire string as text: empty means the field is absent.
String? _text(Object? value) {
  final s = value as String?;
  return (s == null || s.isEmpty) ? null : s;
}

/// Decodes the encryption algorithm from a response map.
PdfEncryptionAlgorithm? decodeEncryptionAlgorithmFromMap(
  Map<String, Object?> r,
) {
  final code = r['encryptionAlgorithm'] as int? ?? 0;
  return decodeEncryptionAlgorithm(code);
}

/// Decodes extracted text from a response map.
String decodeExtractResult(Map<String, Object?> r) =>
    r['text'] as String? ?? '';

/// Decodes search hits from a response map.
List<PdfSearchHit> decodeSearchHits(Map<String, Object?> r) {
  final hits = r['hits'] as List? ?? [];
  return hits.map((h) {
    final m = _asMap(h);
    return PdfSearchHit(
      page: m['page'] as int,
      text: m['text'] as String? ?? '',
      bounds: PdfRect(
        x: (m['x'] as num).toDouble(),
        y: (m['y'] as num).toDouble(),
        width: (m['width'] as num).toDouble(),
        height: (m['height'] as num).toDouble(),
      ),
    );
  }).toList();
}

/// Decodes the image placements of a page from a response map.
List<PdfImagePlacement> decodePageImages(Map<String, Object?> r) {
  final images = r['images'] as List? ?? [];
  return images.map((i) {
    final m = _asMap(i);
    double n(String k) => (m[k] as num).toDouble();
    return PdfImagePlacement(
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
      PdfFormFieldKind.values,
      m['type'],
      'field type',
    );
    final value = _decodeFormFieldValue(m);
    final tooltip = m['tooltip'] as String? ?? '';
    final maxLength = m['maxLength'] as int? ?? -1;
    final alignment = m['alignment'] as String? ?? '';
    return PdfFormField(
      name: m['name'] as String? ?? '',
      kind: type,
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
      alignment: alignment.isEmpty
          ? null
          : _enumByWireName(PdfAlignment.values, alignment, 'alignment'),
      isReadOnly: m['readOnly'] as bool? ?? false,
      isRequired: m['required'] as bool? ?? false,
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
      sizeBytes: size < 0 ? null : size,
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
      bounds: (m['hasRect'] as bool? ?? false)
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
  if (policy.bilevelPpi != null) 'monoPpi': policy.bilevelPpi,
  'downsampleThreshold': policy.downsampleThreshold,
  'jpegQuality': policy.jpegQuality,
  'allowLossy': policy.allowLossy,
  'convertCmykToRgb': policy.convertCmykToRgb,
  'minPixels': policy.minPixels,
  'minSavings': policy.minSavingsRatio,
  'chroma': putEnum(policy.chromaSubsampling),
  'recompressJpeg': policy.recompressJpeg,
};

/// An enum field of a request. Only a [PdfWire] reaches the wire, by its
/// stable name; the byte codec refuses any other enum.
String putEnum(PdfWire e) => e.wire;

/// A list of enum fields of a request, one wire name per value.
List<String> putEnums(Iterable<PdfWire> values) => [
  for (final v in values) putEnum(v),
];

T _enumByWireName<T extends PdfWire>(
  List<T> values,
  Object? wire,
  String what,
) {
  final name = wire as String? ?? '';
  for (final v in values) {
    if (v.wire == name) return v;
  }
  throw PdfEngineFailed('unknown $what "$name" on the wire');
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
        isIndexed: m['indexed'] as bool? ?? false,
        bitsPerComponent: n('bits'),
        width: n('width'),
        height: n('height'),
        hasSoftMask: m['softMask'] as bool? ?? false,
        placementCount: n('uses'),
        minPpi: ppi < 0 ? null : ppi,
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
PdfValidationReport decodeValidationResult(Map<String, Object?> r) =>
    PdfValidationReport(
      isCompliant: r['compliant'] as bool? ?? false,
      errorCount: r['errors'] as int? ?? 0,
      warningCount: r['warnings'] as int? ?? 0,
    );

/// Decodes bookmark-based split plan entries.
List<PdfBookmarkRange> decodeBookmarkRanges(Map<String, Object?> r) {
  final splits = r['splits'] as List? ?? [];
  return splits.map((s) {
    final m = _asMap(s);
    return PdfBookmarkRange(
      title: m['title'] as String?,
      start: m['startPage'] as int? ?? 0,
      end: m['endPage'] as int? ?? 0,
    );
  }).toList();
}

/// Decodes a page-classification result.
PdfPageClassification decodeClassifyPage(
  Map<String, Object?> r, {
  required int page,
}) => PdfPageClassification(
  page: page,
  kind: _enumByWireName(PdfPageKind.values, r['type'], 'page type'),
);

/// Decodes a document-classification result.
PdfDocumentKind decodeClassifyDocument(Map<String, Object?> r) =>
    _enumByWireName(PdfDocumentKind.values, r['type'], 'document type');

/// One frame of a chunked sequence of items (the pages of a render, the
/// images of an extraction): an item opens with a [ChunkHeader] holding its
/// descriptor of type [H], carries its bytes in [ChunkData] frames of at most
/// 256 KiB, and closes with a [ChunkEnd].
sealed class ChunkFrame<H> {
  const ChunkFrame();
}

/// The first frame of an item: its descriptor, before any byte.
final class ChunkHeader<H> extends ChunkFrame<H> {
  /// Creates a header frame.
  const ChunkHeader(this.item);

  /// The item the frames that follow belong to.
  final H item;
}

/// A run of the item's bytes, in order.
final class ChunkData<H> extends ChunkFrame<H> {
  /// Creates a data frame.
  const ChunkData(this.bytes);

  /// The bytes, a view into the frame.
  final Uint8List bytes;
}

/// The last frame of an item.
final class ChunkEnd<H> extends ChunkFrame<H> {
  /// Creates an end frame.
  const ChunkEnd();
}

/// Decodes a chunked frame from a response map by its `kind`; [item] builds
/// the descriptor from a header, and [what] names the sequence in an error.
ChunkFrame<H> decodeChunkFrame<H>(
  Map<String, Object?> data,
  H Function(Map<String, Object?> header) item,
  String what,
) {
  final kind = data['kind'] as String? ?? '';
  return switch (kind) {
    'header' => ChunkHeader<H>(item(data)),
    'data' => ChunkData<H>(_extractBytes(data['data'])),
    'end' => ChunkEnd<H>(),
    _ => throw PdfEngineFailed('unknown $what frame "$kind" on the wire'),
  };
}

/// Decodes a chunked-render frame: its pages carry [PdfRenderedPage]
/// descriptors.
ChunkFrame<PdfRenderedPage> decodeRenderFrame(Map<String, Object?> data) =>
    decodeChunkFrame(data, (header) {
      // The format is the caller's own choice; the engine echoes it, and an
      // unknown one is a wire fault.
      _enumByWireName(
        PdfRenderFormat.values,
        header['format'],
        'render format',
      );
      return PdfRenderedPage(
        page: header['page'] as int? ?? 0,
        width: header['width'] as int? ?? 0,
        height: header['height'] as int? ?? 0,
      );
    }, 'render');

/// Decodes a chunked-extraction frame: its items carry [PdfImageInfo]
/// descriptors.
ChunkFrame<PdfImageInfo> decodeImageFrame(Map<String, Object?> data) =>
    decodeChunkFrame(
      data,
      (header) => PdfImageInfo(
        page: header['page'] as int? ?? 0,
        index: header['index'] as int? ?? 0,
        width: header['width'] as int? ?? 0,
        height: header['height'] as int? ?? 0,
        bitsPerComponent: header['bitsPerComponent'] as int? ?? 8,
        encoding: _enumByWireName(
          PdfImageEncoding.values,
          header['encoding'],
          'image encoding',
        ),
        color: _enumByWireName(
          PdfImageColor.values,
          header['color'],
          'image color',
        ),
      ),
      'image',
    );

/// Decodes editor metadata: the page count, the version and the standard
/// info fields; a field the document does not carry is null.
({int pageCount, String version, PdfMetadata metadata}) decodeEditorMetadata(
  Map<String, Object?> r,
) => (
  pageCount: r['pageCount'] as int? ?? 0,
  version: r['version'] as String? ?? '2.0',
  metadata: decodeMetadata(r),
);

/// Decodes a date field. The wire keeps the PDF date string; an absent or
/// unparseable one is `null`.
DateTime? decodeDate(Object? wire) => parsePdfDate(wire as String?);

/// Encodes a date field as a PDF date string in UTC.
String putDate(DateTime value) => formatPdfDate(value);

/// Encodes [metadata] as the wire arguments of `setMetadata`: a field it
/// leaves null is left out, which removes it from the document.
Map<String, Object?> encodeMetadataArgs(PdfMetadata metadata) => {
  if (metadata.title != null) 'title': metadata.title,
  if (metadata.author != null) 'author': metadata.author,
  if (metadata.subject != null) 'subject': metadata.subject,
  if (metadata.keywords != null) 'keywords': metadata.keywords,
  if (metadata.creator != null) 'creator': metadata.creator,
  if (metadata.producer != null) 'producer': metadata.producer,
  if (metadata.creationDate != null)
    'creationDate': putDate(metadata.creationDate!),
  if (metadata.modificationDate != null)
    'modDate': putDate(metadata.modificationDate!),
};

/// Encodes the pages an edit targets for the engine to resolve against the
/// document as it is when the edit runs: every page, or the listed ones.
Map<String, Object?> encodePages(PdfPages pages) => switch (pages) {
  PdfPagesAll() => {'pagesAll': true},
  PdfPagesSingle(:final index) => {
    'pages': [index],
  },
  PdfPagesList(:final indices) => {'pages': indices},
  PdfPagesRange(:final start, :final end) => {
    'pages': [for (var i = start; i < end; i++) i],
  },
  // The page count is the document's as it is when the edit runs, which
  // only the editor handle can ask for: it resolves a `from` selection into
  // a range before it encodes.
  PdfPagesFrom() => throw const PdfEngineFailed(
    'a page selection from a page is resolved before it is encoded',
  ),
};

/// Decodes a redaction report from an `applyRedactions` response map.
PdfRedactionReport decodeRedactionReport(Map<String, Object?> r) =>
    PdfRedactionReport(
      regionCount: r['regions'] as int? ?? 0,
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

/// Resolves [pages] to a concrete list of 0-based page indices.
List<int> resolvePageIndices(PdfPages pages, int pageCount) => switch (pages) {
  PdfPagesAll() => List.generate(pageCount, (i) => i),
  // A start past the last page names that page alone: the engine refuses it
  // with the page and the page count.
  PdfPagesFrom(:final start) =>
    start < pageCount
        ? List.generate(pageCount - start, (i) => start + i)
        : [start],
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
  'rotation': style.angleDegrees,
  'r': style.color.r,
  'g': style.color.g,
  'b': style.color.b,
  'layer': putEnum(layer),
  ...encodePosition(position),
};

/// Encodes a watermark position into a wire-ready map.
Map<String, Object?> encodePosition(PdfWatermarkPosition position) =>
    switch (position) {
      PdfWatermarkCenter() => {'posType': 0},
      PdfWatermarkCorner(:final corner, :final marginX, :final marginY) => {
        'posType': 1,
        'corner': putEnum(corner),
        'marginX': marginX,
        'marginY': marginY,
      },
      PdfWatermarkTiled(:final columns, :final rows) => {
        'posType': 2,
        'columns': columns,
        'rows': rows,
      },
      PdfWatermarkExact(:final rect) => {
        'posType': 3,
        'posX': rect.x,
        'posY': rect.y,
        'posW': rect.width,
        'posH': rect.height,
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

/// Encodes a redaction overlay: `overlay` `fill` with `r`, `g`, `b`, or
/// `overlay` `none`.
Map<String, Object?> encodeRedactionOverlay(PdfRedactionOverlay overlay) =>
    switch (overlay) {
      PdfRedactionFill(:final color) => {
        'overlay': 'fill',
        ...encodeColorArgs(color),
      },
      PdfRedactionNone() => {'overlay': 'none'},
    };

/// Encodes a set of [PdfFormFieldFlag] as the OR of their `/Ff` bits.
int encodeFormFieldFlags(Set<PdfFormFieldFlag> flags) =>
    flags.fold(0, (acc, f) => acc | f.bit);

// ── Private helpers ──

/// Encodes save options into a wire-ready map.
Map<String, Object?> encodeSaveArgs(PdfSave save) {
  return switch (save) {
    PdfSaveFull(
      :final deflate,
      :final garbageCollect,
      :final useObjectStreams,
      :final mergeDuplicates,
      :final pruneResources,
      :final encryption,
    ) =>
      {
        'saveMode': 0,
        'deflate': deflate != PdfDeflate.none,
        'garbageCollect': garbageCollect,
        'objectStreams': useObjectStreams,
        'deflateLevel': _deflateLevel(deflate),
        'mergeDuplicates': mergeDuplicates,
        'pruneResources': pruneResources,
        ..._encodeEncryption(encryption),
      },
    PdfSaveIncremental() => {
      'saveMode': 1,
      ..._encodeEncryption(const PdfEncryption.keep()),
    },
    // The editor handle tries the incremental save and, when it is refused,
    // writes the fallback: no single save is encoded for it.
    PdfSaveIncrementalOrFull() => throw const PdfEngineFailed(
      'an incremental-or-full save is resolved before it is encoded',
    ),
  };
}

/// The zlib level the engine deflates at for [deflate].
int _deflateLevel(PdfDeflate deflate) => switch (deflate) {
  PdfDeflate.none => 0,
  PdfDeflate.fast => 1,
  PdfDeflate.balanced => 6,
  PdfDeflate.best => 9,
};

Map<String, Object?> _encodeEncryption(PdfEncryption encryption) {
  final encryptMode = switch (encryption) {
    PdfEncryptionKeep() => 0,
    PdfEncryptionRemove() => 1,
    PdfEncryptionPassword() => 2,
  };
  String? encAlgo;
  String encUserPw = '';
  String encOwnerPw = '';
  int encPerms = -1;
  if (encryption case final PdfEncryptionPassword c) {
    encAlgo = putEnum(c.algorithm);
    encUserPw = c.userPassword ?? '';
    encOwnerPw = c.ownerPassword;
    encPerms = c.permissions.bits;
  }
  return {
    'encryptMode': encryptMode,
    if (encAlgo != null) 'encryptAlgo': encAlgo,
    'encryptUserPw': encUserPw,
    'encryptOwnerPw': encOwnerPw,
    'encryptPermissions': encPerms,
  };
}

/// Decodes an encryption algorithm from its integer code.
PdfEncryptionAlgorithm? decodeEncryptionAlgorithm(int code) => switch (code) {
  1 => PdfEncryptionAlgorithm.rc4Key40,
  2 => PdfEncryptionAlgorithm.rc4Key128,
  3 => PdfEncryptionAlgorithm.aesKey128,
  4 => PdfEncryptionAlgorithm.aesKey256,
  _ => null,
};

/// Decodes permission bits into a [PdfPermissions] instance.
PdfPermissions decodePermissions(int bits) => PdfPermissions(
  canPrint: bits & (1 << 2) != 0,
  canModify: bits & (1 << 3) != 0,
  canCopy: bits & (1 << 4) != 0,
  canAnnotate: bits & (1 << 5) != 0,
  canFillForms: bits & (1 << 8) != 0,
  canExtractForAccessibility: bits & (1 << 9) != 0,
  canAssemble: bits & (1 << 10) != 0,
  canPrintHighQuality: bits & (1 << 11) != 0,
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
