import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Per-page metadata.
@immutable
final class PdfPageInfo {
  /// Creates page info.
  const PdfPageInfo({
    required this.index,
    required this.width,
    required this.height,
    this.rotation = PdfRotation.r0,
    this.label,
  });

  /// Zero-based page index.
  final int index;

  /// Page width in points.
  final double width;

  /// Page height in points.
  final double height;

  /// Page rotation.
  final PdfRotation rotation;

  /// Optional page label from the PDF's page-label dictionary.
  final String? label;

  bool get _quarterTurn =>
      rotation == PdfRotation.r90 || rotation == PdfRotation.r270;

  /// Width after applying rotation.
  double get effectiveWidth => _quarterTurn ? height : width;

  /// Height after applying rotation.
  double get effectiveHeight => _quarterTurn ? width : height;

  @override
  bool operator ==(Object other) =>
      other is PdfPageInfo &&
      other.index == index &&
      other.width == width &&
      other.height == height &&
      other.rotation == rotation &&
      other.label == label;

  @override
  int get hashCode => Object.hash(index, width, height, rotation, label);
}
