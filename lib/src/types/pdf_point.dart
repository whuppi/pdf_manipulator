import 'package:meta/meta.dart';

/// A point in PDF coordinate space (points, 1pt = 1/72 inch), y up.
@immutable
final class PdfPoint {
  /// Creates the point at [x], [y].
  const PdfPoint(this.x, this.y);

  /// Distance from the left edge of the page.
  final double x;

  /// Distance from the bottom edge of the page.
  final double y;

  @override
  bool operator ==(Object other) =>
      other is PdfPoint && other.x == x && other.y == y;

  @override
  int get hashCode => Object.hash(x, y);

  @override
  String toString() => 'PdfPoint($x, $y)';
}
