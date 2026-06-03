import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// A text search result with position on the page.
@immutable
class SearchResult {
  /// Creates a search result.
  const SearchResult({
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
}
