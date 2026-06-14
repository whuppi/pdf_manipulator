// Sealed type for page scope — used by every page-targeted operation.
//
// The compiler enforces exhaustive handling. No null. No empty list.
// No guessing. The call site reads what it does.

/// Which pages an operation targets.
sealed class PdfPages {
  const PdfPages();

  /// Every page in the document.
  const factory PdfPages.all() = PdfPagesAll;

  /// A single page by index (0-based).
  const factory PdfPages.single(int index) = PdfPagesSingle;

  /// A list of specific pages by index (0-based).
  const factory PdfPages.list(List<int> indices) = PdfPagesList;

  /// A range of pages (inclusive start, exclusive end).
  const factory PdfPages.range(int start, int end) = PdfPagesRange;
}

/// Every page.
class PdfPagesAll extends PdfPages {
  /// Creates an all-pages scope.
  const PdfPagesAll();
}

/// One page.
class PdfPagesSingle extends PdfPages {
  /// Creates a single-page scope.
  const PdfPagesSingle(this.index);

  /// Zero-based page index.
  final int index;
}

/// Specific pages.
class PdfPagesList extends PdfPages {
  /// Creates a page-list scope.
  const PdfPagesList(this.indices);

  /// Zero-based page indices.
  final List<int> indices;
}

/// Contiguous range [start, end).
class PdfPagesRange extends PdfPages {
  /// Creates a page-range scope.
  const PdfPagesRange(this.start, this.end);

  /// Inclusive start index.
  final int start;

  /// Exclusive end index.
  final int end;
}
