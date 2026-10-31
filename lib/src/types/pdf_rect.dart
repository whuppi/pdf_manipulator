import 'package:meta/meta.dart';

/// A rectangle in PDF coordinate space (points, 1pt = 1/72 inch), y up.
@immutable
class PdfRect {
  /// Creates a rectangle from its lower-left corner and its size.
  const PdfRect({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
  });

  /// Creates a rectangle from its four edges. [left] and [bottom] are the
  /// lower-left corner, [right] and [top] the upper-right one.
  const PdfRect.fromEdges({
    required double left,
    required double bottom,
    required double right,
    required double top,
  }) : x = left,
       y = bottom,
       width = right - left,
       height = top - bottom;

  /// Left edge x-coordinate.
  final double x;

  /// Bottom edge y-coordinate.
  final double y;

  /// Rectangle width.
  final double width;

  /// Rectangle height.
  final double height;

  /// Left edge x-coordinate; the same as [x].
  double get left => x;

  /// Bottom edge y-coordinate; the same as [y].
  double get bottom => y;

  /// Right edge x-coordinate.
  double get right => x + width;

  /// Top edge y-coordinate: [bottom] plus [height].
  double get top => y + height;

  @override
  bool operator ==(Object other) =>
      other is PdfRect &&
      other.x == x &&
      other.y == y &&
      other.width == width &&
      other.height == height;

  @override
  int get hashCode => Object.hash(x, y, width, height);

  @override
  String toString() => 'PdfRect($x, $y, ${width}x$height)';
}
