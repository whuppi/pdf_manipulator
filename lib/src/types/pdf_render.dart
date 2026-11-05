import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_wire.dart';

/// What the bytes a render writes for a page are.
enum PdfRenderFormat implements PdfWire {
  /// A PNG file.
  png('png'),

  /// Raw rows: straight-alpha RGBA, four bytes a pixel, top to bottom, with
  /// no header. The page's size is in its [PdfRenderedPage].
  rgba('rgba');

  const PdfRenderFormat(this.wire);

  @override
  final String wire;
}

/// How large a rendered page is. Pixels appear only here, in
/// [PdfRenderedPage] and in `PdfImageInfo`.
///
/// ```dart
/// const PdfRenderSize.dpi(150);                       // the default
/// const PdfRenderSize.fit(maxWidth: 800, maxHeight: 600);
/// ```
sealed class PdfRenderSize {
  const PdfRenderSize();

  /// Renders at [dpi] dots per inch: a page 612 points wide is 612 / 72 *
  /// [dpi] pixels wide.
  const factory PdfRenderSize.dpi(int dpi) = PdfRenderDpi;

  /// Renders as large as fits inside [maxWidth] by [maxHeight] pixels,
  /// keeping the page's aspect ratio.
  const factory PdfRenderSize.fit({
    required int maxWidth,
    required int maxHeight,
  }) = PdfRenderFit;
}

/// [PdfRenderSize.dpi].
final class PdfRenderDpi extends PdfRenderSize {
  /// Creates a dots-per-inch size.
  const PdfRenderDpi(this.dpi) : assert(dpi >= 1, 'dpi must be >= 1');

  /// Dots per inch.
  final int dpi;

  @override
  bool operator ==(Object other) => other is PdfRenderDpi && other.dpi == dpi;

  @override
  int get hashCode => Object.hash(PdfRenderDpi, dpi);
}

/// [PdfRenderSize.fit].
final class PdfRenderFit extends PdfRenderSize {
  /// Creates a bounding-box size.
  const PdfRenderFit({required this.maxWidth, required this.maxHeight})
    : assert(maxWidth >= 1 && maxHeight >= 1, 'a size must be >= 1 pixel');

  /// Maximum output width in pixels.
  final int maxWidth;

  /// Maximum output height in pixels.
  final int maxHeight;

  @override
  bool operator ==(Object other) =>
      other is PdfRenderFit &&
      other.maxWidth == maxWidth &&
      other.maxHeight == maxHeight;

  @override
  int get hashCode => Object.hash(PdfRenderFit, maxWidth, maxHeight);
}

/// One page a render wrote to its sink: which page, and the size in pixels
/// of the image written. The render hands it to the sink factory before any
/// pixel is written, and returns it once the page's sink is closed.
@immutable
final class PdfRenderedPage {
  /// Creates a rendered-page descriptor.
  const PdfRenderedPage({
    required this.page,
    required this.width,
    required this.height,
  });

  /// Zero-based index of the page that was rendered.
  final int page;

  /// Image width in pixels.
  final int width;

  /// Image height in pixels.
  final int height;

  @override
  bool operator ==(Object other) =>
      other is PdfRenderedPage &&
      other.page == page &&
      other.width == width &&
      other.height == height;

  @override
  int get hashCode => Object.hash(page, width, height);

  @override
  String toString() => 'PdfRenderedPage(page $page, ${width}x$height)';
}
