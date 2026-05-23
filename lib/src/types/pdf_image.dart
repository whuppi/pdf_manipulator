import 'dart:typed_data';

import 'package:meta/meta.dart';

@immutable
class PdfImage {
  final int width;
  final int height;
  final String format;
  final String colorSpace;
  final int bitsPerComponent;
  final Uint8List data;

  const PdfImage({
    required this.width,
    required this.height,
    required this.format,
    required this.colorSpace,
    required this.bitsPerComponent,
    required this.data,
  });

  @override
  String toString() =>
      'PdfImage(${width}x$height, $format, $colorSpace, ${data.length} bytes)';
}

@immutable
class RenderedPage {
  final int width;
  final int height;
  final Uint8List data;

  const RenderedPage({
    required this.width,
    required this.height,
    required this.data,
  });

  @override
  String toString() => 'RenderedPage(${width}x$height, ${data.length} bytes)';
}
