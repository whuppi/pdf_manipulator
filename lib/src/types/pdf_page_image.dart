import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_matrix.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// An image XObject as placed on a page.
///
/// Returned by `PdfEditor.pageImages`. [name] is the resource name the
/// page's content stream draws the image by — the name `resizeImage`
/// takes. It is unique within a page, not across pages.
@immutable
class PdfPageImage {
  /// Creates a page-image descriptor.
  const PdfPageImage({
    required this.name,
    required this.bounds,
    required this.transform,
  });

  /// XObject resource name within the page (for example `Im1`).
  final String name;

  /// Placement rectangle in page units (points): the lower-left corner
  /// and the drawn size. For a rotated or skewed placement this is the
  /// transform's translation and scale, not the visual envelope — read
  /// [transform] for the exact geometry.
  final PdfRect bounds;

  /// The full transform the image is drawn with.
  final PdfMatrix transform;

  @override
  String toString() =>
      'PdfPageImage($name, ${bounds.width}x${bounds.height} at '
      '(${bounds.x}, ${bounds.y}))';
}
