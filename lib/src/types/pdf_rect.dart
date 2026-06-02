import 'package:meta/meta.dart';

/// A rectangle in PDF coordinate space (points, 1pt = 1/72 inch).
@immutable
class PdfRect {
  final double x;
  final double y;
  final double width;
  final double height;

  const PdfRect({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
  });

  double get right => x + width;
  double get bottom => y + height;

  @override
  String toString() => 'PdfRect($x, $y, ${width}x$height)';
}
