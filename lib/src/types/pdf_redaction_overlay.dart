// What a redaction draws where it removed content.

import 'package:pdf_manipulator/src/types/pdf_params.dart';

/// What a redaction draws over the region it removed: a box of one colour, or
/// nothing at all.
sealed class PdfRedactionOverlay {
  const PdfRedactionOverlay();

  /// An opaque box of [color] over the region.
  const factory PdfRedactionOverlay.fill(PdfColor color) = PdfRedactionFill;

  /// No box: the content under the region is removed and the page shows what
  /// is behind it. This is an erase.
  const factory PdfRedactionOverlay.none() = PdfRedactionNone;
}

/// An opaque box over the region.
final class PdfRedactionFill extends PdfRedactionOverlay {
  /// Creates a box overlay of [color].
  const PdfRedactionFill(this.color);

  /// The box's color.
  final PdfColor color;

  @override
  bool operator ==(Object other) =>
      other is PdfRedactionFill && other.color == color;

  @override
  int get hashCode => color.hashCode;
}

/// No box over the region.
final class PdfRedactionNone extends PdfRedactionOverlay {
  /// Creates the no-box overlay.
  const PdfRedactionNone();

  @override
  bool operator ==(Object other) => other is PdfRedactionNone;

  @override
  int get hashCode => (PdfRedactionNone).hashCode;
}
