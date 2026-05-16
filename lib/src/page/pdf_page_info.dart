import 'package:meta/meta.dart';

/// Per-page metadata.
@immutable
class PdfPageInfo {
  final int index;
  final double width;
  final double height;
  final int rotation;
  final String? label;

  const PdfPageInfo({
    required this.index,
    required this.width,
    required this.height,
    this.rotation = 0,
    this.label,
  });

  double get effectiveWidth =>
      (rotation == 90 || rotation == 270) ? height : width;

  double get effectiveHeight =>
      (rotation == 90 || rotation == 270) ? width : height;
}
