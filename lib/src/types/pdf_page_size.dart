// The size of a page, in points.

/// The size of a page, in points.
///
/// The named sizes match the engine's: A4 is 595 × 842, US Letter 612 ×
/// 792, US Legal 612 × 1008, A3 842 × 1190 and A5 420 × 595.
sealed class PdfPageSize {
  const PdfPageSize();

  /// A4 (210 × 297 mm).
  const factory PdfPageSize.a4() = PdfPageSizeA4;

  /// US Letter (8.5 × 11 in).
  const factory PdfPageSize.letter() = PdfPageSizeLetter;

  /// US Legal (8.5 × 14 in).
  const factory PdfPageSize.legal() = PdfPageSizeLegal;

  /// A3 (297 × 420 mm).
  const factory PdfPageSize.a3() = PdfPageSizeA3;

  /// A5 (148 × 210 mm).
  const factory PdfPageSize.a5() = PdfPageSizeA5;

  /// A page of [width] × [height] points.
  const factory PdfPageSize.custom(double width, double height) =
      PdfPageSizeCustom;

  /// The page's width in points.
  double get width;

  /// The page's height in points.
  double get height;

  /// The same size turned on its side: the width and the height swap.
  PdfPageSize get landscape => PdfPageSize.custom(height, width);

  @override
  bool operator ==(Object other) =>
      other is PdfPageSize && other.width == width && other.height == height;

  @override
  int get hashCode => Object.hash(width, height);

  @override
  String toString() => 'PdfPageSize($width × $height)';
}

/// An A4 page: 595 × 842 points.
final class PdfPageSizeA4 extends PdfPageSize {
  /// Creates an A4 page size.
  const PdfPageSizeA4();

  @override
  double get width => 595;

  @override
  double get height => 842;
}

/// A US Letter page: 612 × 792 points.
final class PdfPageSizeLetter extends PdfPageSize {
  /// Creates a US Letter page size.
  const PdfPageSizeLetter();

  @override
  double get width => 612;

  @override
  double get height => 792;
}

/// A US Legal page: 612 × 1008 points.
final class PdfPageSizeLegal extends PdfPageSize {
  /// Creates a US Legal page size.
  const PdfPageSizeLegal();

  @override
  double get width => 612;

  @override
  double get height => 1008;
}

/// An A3 page: 842 × 1190 points.
final class PdfPageSizeA3 extends PdfPageSize {
  /// Creates an A3 page size.
  const PdfPageSizeA3();

  @override
  double get width => 842;

  @override
  double get height => 1190;
}

/// An A5 page: 420 × 595 points.
final class PdfPageSizeA5 extends PdfPageSize {
  /// Creates an A5 page size.
  const PdfPageSizeA5();

  @override
  double get width => 420;

  @override
  double get height => 595;
}

/// A page of a given width and height, in points.
final class PdfPageSizeCustom extends PdfPageSize {
  /// Creates a custom page size.
  const PdfPageSizeCustom(this.width, this.height);

  @override
  final double width;

  @override
  final double height;
}
