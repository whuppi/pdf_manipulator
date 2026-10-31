import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/pdf_wire.dart';

/// How an image's samples are stored.
enum PdfImageEncoding implements PdfWire {
  /// A lossless container (Flate, LZW, RunLength, ASCII, or none).
  raw('raw'),

  /// DCTDecode.
  jpeg('jpeg'),

  /// JPXDecode (JPEG 2000).
  jpx('jpx'),

  /// CCITTFaxDecode.
  ccitt('ccitt'),

  /// JBIG2Decode.
  jbig2('jbig2'),

  /// A filter the engine does not know.
  unknown('unknown');

  const PdfImageEncoding(this.wire);

  @override
  final String wire;
}

/// What an image's decoded samples mean.
enum PdfImageColor implements PdfWire {
  /// A stencil mask or a 1-bit gray image.
  bilevel('bilevel'),

  /// One component.
  gray('gray'),

  /// Three components.
  rgb('rgb'),

  /// Four components.
  cmyk('cmyk'),

  /// Lab, Separation, DeviceN, Pattern, or an unparsable base.
  other('other');

  const PdfImageColor(this.wire);

  @override
  final String wire;
}

/// What `PdfEditor.reduceImages` did to one image.
enum PdfImageAction implements PdfWire {
  /// Bytes unchanged.
  kept('kept'),

  /// Same pixel size, new encoding.
  recompressed('recompressed'),

  /// Fewer pixels, new encoding.
  downsampled('downsampled');

  const PdfImageAction(this.wire);

  @override
  final String wire;
}

/// Why an image was kept as stored.
enum PdfImageKeepReason implements PdfWire {
  /// JPEG 2000: not decoded in this build, never re-encoded.
  unsupportedJpx('unsupportedJpx'),

  /// JBIG2: no encoder.
  unsupportedJbig2('unsupportedJbig2'),

  /// A filter the engine cannot undo.
  unsupportedFilter('unsupportedFilter'),

  /// Lab, Separation, DeviceN, Pattern.
  unsupportedColor('unsupportedColor'),

  /// A `/Mask` stream (stencil) hangs off the image.
  unsupportedStencilMask('unsupportedStencilMask'),

  /// A `/Mask` colour-key array hangs off the image.
  unsupportedColorKeyMask('unsupportedColorKeyMask'),

  /// A JPX opacity channel.
  unsupportedSmaskInData('unsupportedSmaskInData'),

  /// Narrower or shorter than the policy's `minPixels`.
  tooSmall('tooSmall'),

  /// Drawn with a degenerate transform, or not at all.
  noPlacement('noPlacement'),

  /// At or under its target resolution.
  withinResolution('withinResolution'),

  /// Nothing the policy allows could make it smaller.
  alreadyOptimal('alreadyOptimal'),

  /// Re-encoding did not save the policy's `minSavings` share of the bytes.
  belowMinSavings('belowMinSavings'),

  /// The stored samples could not be decoded, so nothing was rewritten.
  undecodable('undecodable');

  const PdfImageKeepReason(this.wire);

  @override
  final String wire;
}

/// One image XObject's outcome from `PdfEditor.reduceImages`.
@immutable
class PdfImageOutcome {
  /// Creates an outcome row.
  const PdfImageOutcome({
    required this.objectId,
    required this.encoding,
    required this.color,
    required this.indexed,
    required this.bits,
    required this.width,
    required this.height,
    required this.hasSoftMask,
    required this.uses,
    required this.ppiMin,
    required this.action,
    required this.keepReason,
    required this.bytesBefore,
    required this.bytesAfter,
    required this.widthAfter,
    required this.heightAfter,
  });

  /// The XObject's object number in the source file.
  final int objectId;

  /// Storage before the operation.
  final PdfImageEncoding encoding;

  /// Colour model before the operation.
  final PdfImageColor color;

  /// Whether the stored samples were palette indices.
  final bool indexed;

  /// Bits per component before the operation.
  final int bits;

  /// Pixel width before.
  final int width;

  /// Pixel height before.
  final int height;

  /// Whether an `/SMask` was attached (it is co-processed).
  final bool hasSoftMask;

  /// Placements across the document's pages, forms included.
  final int uses;

  /// Effective resolution of the most demanding placement; `null` when
  /// no placement had a usable transform.
  final double? ppiMin;

  /// What happened.
  final PdfImageAction action;

  /// Why nothing happened; `null` unless [action] is
  /// [PdfImageAction.kept].
  final PdfImageKeepReason? keepReason;

  /// Stored bytes before, image plus soft mask.
  final int bytesBefore;

  /// Stored bytes after; equals [bytesBefore] when kept.
  final int bytesAfter;

  /// Pixel width after.
  final int widthAfter;

  /// Pixel height after.
  final int heightAfter;

  @override
  String toString() =>
      'PdfImageOutcome(#$objectId ${width}x$height ${encoding.name}/'
      '${color.name} → ${action.name}'
      '${keepReason == null ? '' : ' (${keepReason!.name})'}, '
      '$bytesBefore → $bytesAfter bytes)';
}

/// The result of `PdfEditor.reduceImages`: one row per image XObject
/// drawn on the document's pages, in ascending object number.
@immutable
class PdfImageReport {
  /// Creates a report.
  const PdfImageReport(this.images);

  /// One row per image.
  final List<PdfImageOutcome> images;

  /// Images whose bytes changed.
  int get changed =>
      images.where((o) => o.action != PdfImageAction.kept).length;

  /// Stored bytes before, summed over every row.
  int get bytesBefore => images.fold(0, (sum, o) => sum + o.bytesBefore);

  /// Stored bytes after, summed over every row.
  int get bytesAfter => images.fold(0, (sum, o) => sum + o.bytesAfter);

  @override
  String toString() =>
      'PdfImageReport(${images.length} images, $changed changed, '
      '$bytesBefore → $bytesAfter bytes)';
}
