import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// A text search result with position on the page.
@immutable
class SearchResult {
  final String text;
  final int page;
  final PdfRect rect;
  const SearchResult({
    required this.text,
    required this.page,
    required this.rect,
  });
}
