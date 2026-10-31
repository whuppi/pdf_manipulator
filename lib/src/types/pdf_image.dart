import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_image_report.dart';

/// One image found on a page: where it is and how it is stored, without its
/// bytes. The bytes go to a sink the extraction names.
@immutable
class PdfImage {
  /// Creates an image descriptor.
  const PdfImage({
    required this.page,
    required this.index,
    required this.width,
    required this.height,
    required this.bitsPerComponent,
    required this.encoding,
    required this.color,
  });

  /// Zero-based index of the page the image is drawn on.
  final int page;

  /// Position of the image among the images of [page], in the order found.
  final int index;

  /// Image width in pixels.
  final int width;

  /// Image height in pixels.
  final int height;

  /// Bits per color component.
  final int bitsPerComponent;

  /// How the image's samples are stored.
  final PdfImageEncoding encoding;

  /// What the image's decoded samples mean.
  final PdfImageColor color;

  @override
  bool operator ==(Object other) =>
      other is PdfImage &&
      other.page == page &&
      other.index == index &&
      other.width == width &&
      other.height == height &&
      other.bitsPerComponent == bitsPerComponent &&
      other.encoding == encoding &&
      other.color == color;

  @override
  int get hashCode => Object.hash(
    page,
    index,
    width,
    height,
    bitsPerComponent,
    encoding,
    color,
  );

  @override
  String toString() =>
      'PdfImage(page $page #$index, ${width}x$height, $encoding, $color)';
}
