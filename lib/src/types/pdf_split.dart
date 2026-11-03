import 'package:meta/meta.dart';

/// How `Pdf.split` cuts a document into parts.
///
/// ```dart
/// const PdfSplitRule.every(10);        // ten pages a part
/// const PdfSplitRule.maxBytes(5000000); // as many pages as fit in 5 MB
/// const PdfSplitRule.bookmarks();      // one part per top-level bookmark
/// ```
sealed class PdfSplitRule {
  const PdfSplitRule();

  /// Cuts a part every [pages] pages; the last part holds what is left.
  const factory PdfSplitRule.every(int pages) = PdfSplitEvery;

  /// Cuts each part as long as it stays within [bytes] bytes, so no part is
  /// larger unless one page alone is.
  const factory PdfSplitRule.maxBytes(int bytes) = PdfSplitMaxBytes;

  /// Cuts a part for each top-level bookmark, from its page to the page
  /// before the next bookmark's. Pages before the first bookmark belong to no
  /// part.
  const factory PdfSplitRule.bookmarks() = PdfSplitBookmarks;
}

/// [PdfSplitRule.every].
final class PdfSplitEvery extends PdfSplitRule {
  /// Creates the every-n-pages rule.
  const PdfSplitEvery(this.pages);

  /// Pages in each part.
  final int pages;

  @override
  bool operator ==(Object other) =>
      other is PdfSplitEvery && other.pages == pages;

  @override
  int get hashCode => Object.hash(PdfSplitEvery, pages);

  @override
  String toString() => 'PdfSplitRule.every($pages)';
}

/// [PdfSplitRule.maxBytes].
final class PdfSplitMaxBytes extends PdfSplitRule {
  /// Creates the size-bounded rule.
  const PdfSplitMaxBytes(this.bytes);

  /// The most bytes a part may hold.
  final int bytes;

  @override
  bool operator ==(Object other) =>
      other is PdfSplitMaxBytes && other.bytes == bytes;

  @override
  int get hashCode => Object.hash(PdfSplitMaxBytes, bytes);

  @override
  String toString() => 'PdfSplitRule.maxBytes($bytes)';
}

/// [PdfSplitRule.bookmarks].
final class PdfSplitBookmarks extends PdfSplitRule {
  /// Creates the bookmark rule.
  const PdfSplitBookmarks();

  @override
  bool operator ==(Object other) => other is PdfSplitBookmarks;

  @override
  int get hashCode => (PdfSplitBookmarks).hashCode;

  @override
  String toString() => 'PdfSplitRule.bookmarks()';
}

/// One part a split wrote: which pages of the source it holds. The split
/// hands it to the sink factory before the part is written, and returns it
/// once the part's sink is closed. It holds no bytes: they went to the sink.
@immutable
final class PdfSplitPart {
  /// Creates a part that holds the pages [start] to [end].
  const PdfSplitPart({
    required this.index,
    required this.start,
    required this.end,
    this.title,
  });

  /// The part's position among the parts, from 0.
  final int index;

  /// Zero-based index of the part's first page in the source.
  final int start;

  /// Zero-based index of the part's last page in the source (inclusive).
  final int end;

  /// The bookmark's title for a part cut by [PdfSplitRule.bookmarks]; null
  /// when the bookmark has none and for every other rule.
  final String? title;

  @override
  bool operator ==(Object other) =>
      other is PdfSplitPart &&
      other.index == index &&
      other.start == start &&
      other.end == end &&
      other.title == title;

  @override
  int get hashCode => Object.hash(index, start, end, title);

  @override
  String toString() => 'PdfSplitPart($index, pages $start-$end)';
}
