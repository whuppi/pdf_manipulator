// How one image page is sized and filled.

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_page_size.dart';

/// The page one image gets: its size, how the image fills it and the
/// margin around it.
///
/// A null [size] makes the page the image's own size, one point per pixel.
final class PdfImagePageOptions {
  /// Creates the options of an image page.
  const PdfImagePageOptions({
    this.size,
    this.fit = PdfImageFit.contain,
    this.margin = 0,
  });

  /// The page's size, or null for the image's own size.
  final PdfPageSize? size;

  /// How the image fills the page.
  final PdfImageFit fit;

  /// The margin around the image, in points.
  final double margin;

  @override
  bool operator ==(Object other) =>
      other is PdfImagePageOptions &&
      other.size == size &&
      other.fit == fit &&
      other.margin == margin;

  @override
  int get hashCode => Object.hash(size, fit, margin);

  @override
  String toString() =>
      'PdfImagePageOptions(size: $size, fit: $fit, margin: $margin)';
}
