// The style of one text run.

import 'package:pdf_manipulator/src/types/pdf_params.dart';

/// The style of one text run: its font, size, weight, slant and colour.
///
/// Every field is optional: a null [font] or [size] keeps the page's
/// current setting, and a null [color] keeps the text in the colour the
/// page's font draws it in.
final class PdfTextStyle {
  /// Creates a text style.
  const PdfTextStyle({
    this.font,
    this.size,
    this.bold = false,
    this.italic = false,
    this.color,
  });

  /// The font name, or null for the page's current font.
  final String? font;

  /// The font size in points, or null for the page's current size.
  final double? size;

  /// Draw the run in the bold face of its font.
  final bool bold;

  /// Draw the run in the italic face of its font.
  final bool italic;

  /// The colour to draw the run in, or null for the font's own colour.
  final PdfColor? color;

  @override
  bool operator ==(Object other) =>
      other is PdfTextStyle &&
      other.font == font &&
      other.size == size &&
      other.bold == bold &&
      other.italic == italic &&
      other.color == color;

  @override
  int get hashCode => Object.hash(font, size, bold, italic, color);

  @override
  String toString() =>
      'PdfTextStyle(font: $font, size: $size, bold: $bold, italic: $italic, '
      'color: $color)';
}
