import 'dart:typed_data';

import 'package:meta/meta.dart';

/// An image extracted from a PDF page.
@immutable
class PdfImage {
  /// Creates a PDF image descriptor.
  const PdfImage({
    required this.width,
    required this.height,
    required this.format,
    required this.colorSpace,
    required this.bitsPerComponent,
    required this.data,
  });

  /// Image width in pixels.
  final int width;

  /// Image height in pixels.
  final int height;

  /// Image encoding format (e.g. "jpeg", "png").
  final String format;

  /// Color space (e.g. "DeviceRGB", "DeviceGray").
  final String colorSpace;

  /// Bits per color component.
  final int bitsPerComponent;

  /// Raw image bytes.
  final Uint8List data;

  @override
  String toString() =>
      'PdfImage(${width}x$height, $format, $colorSpace, ${data.length} bytes)';
}

/// A rendered page bitmap.
@immutable
class RenderedPage {
  /// Creates a rendered page descriptor.
  const RenderedPage({
    required this.width,
    required this.height,
    required this.data,
  });

  /// Bitmap width in pixels.
  final int width;

  /// Bitmap height in pixels.
  final int height;

  /// PNG-encoded image of the page; decode it to read pixels.
  final Uint8List data;

  @override
  String toString() => 'RenderedPage(${width}x$height, ${data.length} bytes)';
}
