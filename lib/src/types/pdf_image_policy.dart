import 'package:meta/meta.dart';

/// How far `PdfEditor.reduceImages` may go.
///
/// Resolution targets are pixels per inch as the image is drawn on the
/// page (a 128-pixel image drawn 32 points wide is 288 ppi). An image is
/// downsampled only when it exceeds its target by more than [threshold],
/// and only to the target — never below what the largest placement needs.
/// The presets follow Ghostscript's `/screen`, `/ebook` and `/printer`
/// resolutions.
@immutable
class PdfImagePolicy {
  /// Creates a policy; every `null` resolution means "never downsample
  /// that class of image".
  const PdfImagePolicy({
    this.colorDpi,
    this.grayDpi,
    this.monoDpi,
    this.threshold = 1.5,
    this.jpegQuality = 75,
    this.allowLossy = true,
    this.convertCmykToRgb = false,
    this.minPixels = 32,
  });

  /// On-screen reading: 72 ppi colour and gray, 300 ppi bilevel,
  /// JPEG quality 60, CMYK converted to RGB.
  static const screen = PdfImagePolicy(
    colorDpi: 72,
    grayDpi: 72,
    monoDpi: 300,
    jpegQuality: 60,
    convertCmykToRgb: true,
  );

  /// E-readers and tablets: 150 ppi colour and gray, 300 ppi bilevel,
  /// JPEG quality 75, CMYK converted to RGB.
  static const ebook = PdfImagePolicy(
    colorDpi: 150,
    grayDpi: 150,
    monoDpi: 300,
    convertCmykToRgb: true,
  );

  /// Desktop printing: 300 ppi colour and gray, 1200 ppi bilevel,
  /// JPEG quality 85, CMYK kept.
  static const print = PdfImagePolicy(
    colorDpi: 300,
    grayDpi: 300,
    monoDpi: 1200,
    jpegQuality: 85,
  );

  /// Shrink without changing a pixel: no downsampling, no lossy codec;
  /// lossless sources are re-packed with predictors, bilevel images
  /// become CCITT Group 4.
  static const lossless = PdfImagePolicy(allowLossy: false);

  /// Target resolution for RGB and CMYK images.
  final double? colorDpi;

  /// Target resolution for gray images.
  final double? grayDpi;

  /// Target resolution for bilevel images (scans, stencil masks).
  final double? monoDpi;

  /// Downsample only when the effective resolution exceeds the target
  /// times this factor.
  final double threshold;

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

  @override
  String toString() =>
      'PdfImagePolicy(color: $colorDpi, gray: $grayDpi, mono: $monoDpi, '
      'q$jpegQuality, lossy: $allowLossy, cmyk→rgb: $convertCmykToRgb)';
}
