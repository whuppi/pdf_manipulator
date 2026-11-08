// The stroke of a line or an outline.

import 'package:pdf_manipulator/src/types/equality.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';

/// The stroke of a line or an outline: its width, colour and dash pattern.
///
/// A null [dash] draws a solid line; a list of dash lengths (in points)
/// draws a dashed one, starting at [dashPhase].
final class PdfStroke {
  /// Creates a stroke.
  const PdfStroke({
    this.width = 1,
    this.color = PdfColor.black,
    this.dash,
    this.dashPhase = 0,
  });

  /// The line width in points.
  final double width;

  /// The colour to draw the line in.
  final PdfColor color;

  /// The dash lengths in points, or null for a solid line.
  final List<double>? dash;

  /// Where the dash pattern starts, in points.
  final double dashPhase;

  @override
  bool operator ==(Object other) {
    if (other is! PdfStroke) return false;
    final dash = this.dash;
    final otherDash = other.dash;
    return other.width == width &&
        other.color == color &&
        other.dashPhase == dashPhase &&
        (dash == null
            ? otherDash == null
            : otherDash != null && listsEqual(dash, otherDash));
  }

  @override
  int get hashCode => Object.hash(
    width,
    color,
    dash == null ? null : Object.hashAll(dash!),
    dashPhase,
  );

  @override
  String toString() =>
      'PdfStroke(width: $width, color: $color, dash: $dash, '
      'dashPhase: $dashPhase)';
}
