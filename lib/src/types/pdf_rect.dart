import 'package:meta/meta.dart';

/// A rectangle in PDF coordinate space (points, 1pt = 1/72 inch).
@immutable
class PdfRect {
  /// Creates a rectangle.
  const PdfRect({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
  });

  /// Left edge x-coordinate.
  final double x;

  /// Bottom edge y-coordinate.
  final double y;

  /// Rectangle width.
  final double width;

  /// Rectangle height.
  final double height;

  /// Right edge x-coordinate.
  double get right => x + width;

  /// Bottom-plus-height y-coordinate.
  double get bottom => y + height;

  @override
  String toString() => 'PdfRect($x, $y, ${width}x$height)';
}
