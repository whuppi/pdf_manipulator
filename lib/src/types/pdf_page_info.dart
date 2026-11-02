import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// Per-page metadata.
@immutable
final class PdfPageInfo {
  /// Creates page info.
  const PdfPageInfo({
    required this.index,
    required this.mediaBox,
    this.cropBox,
    this.rotation = PdfRotation.r0,
    this.label,
  });

  /// Zero-based page index.
  final int index;

  /// The page's media box in points: its physical size.
  final PdfRect mediaBox;

  /// The page's crop box in points, or `null` when the page has none (it
  /// then shows its whole media box).
  final PdfRect? cropBox;

  /// Page width in points: the media box's.
  double get width => mediaBox.width;

  /// Page height in points: the media box's.
  double get height => mediaBox.height;

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
      other.mediaBox == mediaBox &&
      other.cropBox == cropBox &&
      other.rotation == rotation &&
      other.label == label;

  @override
  int get hashCode => Object.hash(index, mediaBox, cropBox, rotation, label);
}
