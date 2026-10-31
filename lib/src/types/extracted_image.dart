import 'dart:typed_data';

import 'package:meta/meta.dart';

/// An image extracted from a PDF page, with its bytes. Not exported: the
/// stream `PdfDoc.extractImages` yields it until extraction writes to a sink
/// and yields `PdfImage` descriptors.
@immutable
class ExtractedImage {
  /// Creates an extracted image.
  const ExtractedImage({
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
      'ExtractedImage(${width}x$height, $format, $colorSpace, ${data.length} bytes)';
}
