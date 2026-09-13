import 'package:meta/meta.dart';

/// A PDF affine transform `[a b c d e f]` (ISO 32000-1 §8.3.4).
///
/// Maps user-space coordinates as `x' = a·x + c·y + e`,
/// `y' = b·x + d·y + f`. For an image placed without rotation or skew,
/// `a` and `d` are its drawn width and height and `e`, `f` its lower-left
/// corner.
@immutable
class PdfMatrix {
  /// Creates a transform from its six coefficients.
  const PdfMatrix({
    required this.a,
    required this.b,
    required this.c,
    required this.d,
    required this.e,
    required this.f,
  });

  /// Horizontal scale.
  final double a;

  /// Horizontal skew.
  final double b;

  /// Vertical skew.
  final double c;

  /// Vertical scale.
  final double d;

  /// Horizontal translation.
  final double e;

  /// Vertical translation.
  final double f;

  /// Whether the transform rotates or skews (any non-zero `b` or `c`).
  bool get isAxisAligned => b == 0 && c == 0;

  @override
  String toString() => 'PdfMatrix($a, $b, $c, $d, $e, $f)';
}
