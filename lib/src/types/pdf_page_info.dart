import 'package:meta/meta.dart';

/// Per-page metadata.
@immutable
class PdfPageInfo {
  /// Creates page info.
  const PdfPageInfo({
    required this.index,
    required this.width,
    required this.height,
    this.rotation = 0,
    this.label,
  });

  /// Zero-based page index.
  final int index;

  /// Page width in points.
  final double width;

  /// Page height in points.
  final double height;

  /// Page rotation in degrees (0, 90, 180, 270).
  final int rotation;

  /// Optional page label from the PDF's page-label dictionary.
  final String? label;

  /// Width after applying rotation.
  double get effectiveWidth =>
      (rotation == 90 || rotation == 270) ? height : width;

  /// Height after applying rotation.
  double get effectiveHeight =>
      (rotation == 90 || rotation == 270) ? width : height;
}
