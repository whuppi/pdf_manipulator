// How one image page is sized and filled.

import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_page_size.dart';

/// The page one image gets: its size, how the image fills it and the
/// margin around it.
@immutable
final class PdfImagePage {
  /// Creates the description of an image page.
  const PdfImagePage({
    this.size = const PdfImagePageSize.ofImage(),
    this.fit = PdfImageFit.contain,
    this.margin = 0,
  });

  /// The page's size.
  final PdfImagePageSize size;

  /// How the image fills the page.
  final PdfImageFit fit;

  /// The margin around the image, in points.
  final double margin;

  @override
  bool operator ==(Object other) =>
      other is PdfImagePage &&
      other.size == size &&
      other.fit == fit &&
      other.margin == margin;

  @override
  int get hashCode => Object.hash(size, fit, margin);

  @override
  String toString() => 'PdfImagePage(size: $size, fit: $fit, margin: $margin)';
}

/// The size of an image page.
sealed class PdfImagePageSize {
  const PdfImagePageSize();

  /// The image's own size, one point per pixel. The default.
  const factory PdfImagePageSize.ofImage() = PdfImagePageSizeOfImage;

  /// A page of [size], however large the image.
  const factory PdfImagePageSize.fixed(PdfPageSize size) =
      PdfImagePageSizeFixed;
}

/// [PdfImagePageSize.ofImage].
final class PdfImagePageSizeOfImage extends PdfImagePageSize {
  /// Creates the image's own size.
  const PdfImagePageSizeOfImage();

  @override
  bool operator ==(Object other) => other is PdfImagePageSizeOfImage;

  @override
  int get hashCode => (PdfImagePageSizeOfImage).hashCode;
}

/// [PdfImagePageSize.fixed].
final class PdfImagePageSizeFixed extends PdfImagePageSize {
  /// Creates a fixed page size.
  const PdfImagePageSizeFixed(this.size);

  /// The page's size.
  final PdfPageSize size;

  @override
  bool operator ==(Object other) =>
      other is PdfImagePageSizeFixed && other.size == size;

  @override
  int get hashCode => Object.hash(PdfImagePageSizeFixed, size);
}
