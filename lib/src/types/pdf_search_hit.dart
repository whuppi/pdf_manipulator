import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// A text search hit with position on the page.
@immutable
final class PdfSearchHit {
  /// Creates a search hit.
  const PdfSearchHit({
    required this.text,
    required this.page,
    required this.rect,
  });

  /// The matched text.
  final String text;

  /// Zero-based page index where the match was found.
  final int page;

  /// Bounding rectangle of the match on the page.
  final PdfRect rect;

  @override
  bool operator ==(Object other) =>
      other is PdfSearchHit &&
      other.text == text &&
      other.page == page &&
      other.rect == rect;

  @override
  int get hashCode => Object.hash(text, page, rect);

  @override
  String toString() => 'PdfSearchHit(page $page, "$text")';
}
