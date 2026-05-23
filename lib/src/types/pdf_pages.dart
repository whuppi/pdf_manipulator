// Sealed type for page scope — used by every page-targeted operation.
//
// The compiler enforces exhaustive handling. No null. No empty list.
// No guessing. The call site reads what it does.

/// Which pages an operation targets.
sealed class PdfPages {
  const PdfPages();

  /// Every page in the document.
  const factory PdfPages.all() = PdfAllPages;

  /// A single page by index (0-based).
  const factory PdfPages.single(int index) = PdfSinglePage;

  /// A list of specific pages by index (0-based).
  const factory PdfPages.list(List<int> indices) = PdfPageList;

  /// A range of pages (inclusive start, exclusive end).
  const factory PdfPages.range(int start, int end) = PdfPageRange;
}

/// Every page.
class PdfAllPages extends PdfPages {
  const PdfAllPages();
}

/// One page.
class PdfSinglePage extends PdfPages {
  const PdfSinglePage(this.index);
  final int index;
}

/// Specific pages.
class PdfPageList extends PdfPages {
  const PdfPageList(this.indices);
  final List<int> indices;
}

/// Contiguous range [start, end).
class PdfPageRange extends PdfPages {
  const PdfPageRange(this.start, this.end);
  final int start;
  final int end;
}
