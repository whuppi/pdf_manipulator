import 'package:meta/meta.dart';

/// How far `PdfEditor.reduceImages` may go.
///
/// Resolution targets are pixels per inch as the image is drawn on the
/// page (a 128-pixel image drawn 32 points wide is 288 ppi). An image is
/// downsampled only when it exceeds its target by more than [downsampleThreshold],
/// and only to the target — never below what the largest placement needs.
/// The presets are the three resolutions print and screen workflows have
/// settled on: 72, 150 and 300 ppi.
@immutable
class PdfImagePolicy {
  /// Creates a policy; every `null` resolution means "never downsample
  /// that class of image".
  const PdfImagePolicy({
    this.colorPpi,
    this.grayPpi,
    this.monoPpi,
    this.downsampleThreshold = 1.5,
    this.jpegQuality = 75,
    this.allowLossy = true,
    this.convertCmykToRgb = false,
    this.minPixels = 32,
    this.minSavings = 0.10,
    this.chromaSubsampling = PdfChromaSubsampling.auto,
    this.recompressJpeg = false,
  });

  /// On-screen reading: 72 ppi colour and gray, 300 ppi bilevel,
  /// JPEG quality 60, CMYK converted to RGB.
  static const screen = PdfImagePolicy(
    colorPpi: 72,
    grayPpi: 72,
    monoPpi: 300,
    jpegQuality: 60,
    convertCmykToRgb: true,
  );

  /// E-readers and tablets: 150 ppi colour and gray, 300 ppi bilevel,
  /// JPEG quality 75, CMYK converted to RGB.
  static const ebook = PdfImagePolicy(
    colorPpi: 150,
    grayPpi: 150,
    monoPpi: 300,
    convertCmykToRgb: true,
  );

  /// Desktop printing: 300 ppi colour and gray, 1200 ppi bilevel,
  /// JPEG quality 85 with full chroma, CMYK kept.
  static const print = PdfImagePolicy(
    colorPpi: 300,
    grayPpi: 300,
    monoPpi: 1200,
    jpegQuality: 85,
    chromaSubsampling: PdfChromaSubsampling.full,
  );

  /// Shrink without changing a pixel: no downsampling, no lossy codec;
  /// lossless sources are re-packed with predictors, bilevel images
  /// become CCITT Group 4.
  static const lossless = PdfImagePolicy(allowLossy: false, minSavings: 0);

  /// Target resolution for RGB and CMYK images.
  final double? colorPpi;

  /// Target resolution for gray images.
  final double? grayPpi;

  /// Target resolution for bilevel images (scans, stencil masks).
  final double? monoPpi;

  /// Downsample only when the effective resolution exceeds the target
  /// times this factor.
  final double downsampleThreshold;

  /// Quality (1–100) for every JPEG written.
  final int jpegQuality;

  /// Whether a losslessly stored continuous-tone image may become a JPEG.
  /// A JPEG that is not downsampled is never re-encoded either way.
  final bool allowLossy;

  /// Whether CMYK output becomes RGB (through the engine's Adobe-aware
  /// CMYK path). Without it CMYK images are kept CMYK; when one must be
  /// rewritten it is stored losslessly, since no CMYK JPEG encoder exists.
  final bool convertCmykToRgb;

  /// Images narrower or shorter than this many pixels are left alone.
  final int minPixels;

  /// A re-encode is written only when it saves at least this fraction of
  /// the image's stored bytes (soft mask included); below it the image is
  /// kept and its row says `belowMinSavings`. Guards the trade of quality
  /// for a few percent. 0 accepts any reduction, which is what
  /// [lossless] uses, since it trades no quality.
  final double minSavings;

  /// Chroma subsampling for every JPEG written; gray images are unaffected.
  final PdfChromaSubsampling chromaSubsampling;

  /// Whether an image already stored as JPEG is re-encoded at
  /// [jpegQuality] even when it is not downsampled. Off by default: every
  /// JPEG generation loses detail, so a stored JPEG is left alone unless
  /// its pixels change. Turn it on for a photo PDF whose images are at
  /// the right resolution but stored at a higher quality than you need;
  /// [minSavings] still applies, so a JPEG already at or below that
  /// quality is kept and a second run changes nothing.
  final bool recompressJpeg;

  @override
  String toString() =>
      'PdfImagePolicy(color: $colorPpi, gray: $grayPpi, mono: $monoPpi, '
      'q$jpegQuality ${chromaSubsampling.name}, lossy: $allowLossy, '
      'recompressJpeg: $recompressJpeg, cmyk→rgb: $convertCmykToRgb, '
      'minSavings: $minSavings)';
}

/// How the colour channels of a written JPEG are subsampled.
///
/// [auto] follows quality: 4:2:0 below 90, 4:4:4 from 90. The `print`
/// preset pins [full], because printed text and hard colour edges show
/// chroma smearing that a screen hides.
enum PdfChromaSubsampling {
  /// 4:4:4 when `jpegQuality` is 90 or more, 4:2:0 below.
  auto,

  /// 4:4:4 — every colour sample kept; text and hard colour edges stay crisp.
  full,

  /// 4:2:0 — colour at half resolution both ways; smallest, fine for photos.
  half,
}
