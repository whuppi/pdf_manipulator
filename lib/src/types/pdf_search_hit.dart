import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// A text search hit with position on the page.
@immutable
final class PdfSearchHit {
  /// Creates a search hit.
  const PdfSearchHit({
    required this.text,
    required this.page,
    required this.bounds,
  });

  /// The matched text.
  final String text;

  /// Zero-based page index where the match was found.
  final int page;

  /// Bounding rectangle of the match on the page.
  final PdfRect bounds;

  @override
  bool operator ==(Object other) =>
      other is PdfSearchHit &&
      other.text == text &&
      other.page == page &&
      other.bounds == bounds;

  @override
  int get hashCode => Object.hash(text, page, bounds);

  @override
  String toString() => 'PdfSearchHit(page $page, "$text")';
}
