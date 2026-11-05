// Page selection and page order — used by every page-targeted operation.
//
// PdfPages is a set: the only way to name several pages. PdfPageOrder is a
// sequence, for the one operation where the order is the point. A page the
// document does not have is the engine's to refuse, with PdfPageOutOfRange.

import 'package:pdf_manipulator/src/types/equality.dart';

/// Which pages an operation targets: a set, never an order.
///
/// ```dart
/// const PdfPages.all();          // every page
/// const PdfPages.single(0);      // the first page
/// const PdfPages.range(0, 4);    // pages 0, 1, 2 and 3
/// const PdfPages.from(2);        // page 2 to the last page
/// PdfPages.list([3, 1]);         // pages 1 and 3
/// ```
///
/// An index is 0-based. A range's end is exclusive, as in
/// `List.sublist(start, end)`.
sealed class PdfPages {
  const PdfPages();

  /// Every page in the document.
  const factory PdfPages.all() = PdfPagesAll;

  /// The page at [index] (0-based, not negative).
  const factory PdfPages.single(int index) = PdfPagesSingle;

  /// The pages from [start] up to, not including, [end]: `0 <= start < end`.
  const factory PdfPages.range(int start, int end) = PdfPagesRange;

  /// The pages from [start] to the last page of the document (0-based, not
  /// negative).
  const factory PdfPages.from(int start) = PdfPagesFrom;

  /// The pages at [indices], kept in ascending order. An empty list, a
  /// negative index and an index given twice throw [ArgumentError].
  factory PdfPages.list(Iterable<int> indices) = PdfPagesList;
}

/// [PdfPages.all].
final class PdfPagesAll extends PdfPages {
  /// Creates an all-pages scope.
  const PdfPagesAll();

  @override
  bool operator ==(Object other) => other is PdfPagesAll;

  @override
  int get hashCode => (PdfPagesAll).hashCode;
}

/// [PdfPages.single].
final class PdfPagesSingle extends PdfPages {
  /// Creates a single-page scope.
  const PdfPagesSingle(this.index) : assert(index >= 0, 'index must be >= 0');

  /// Zero-based page index.
  final int index;

  @override
  bool operator ==(Object other) =>
      other is PdfPagesSingle && other.index == index;

  @override
  int get hashCode => index.hashCode;
}

/// [PdfPages.list].
final class PdfPagesList extends PdfPages {
  /// Creates a page-list scope from [indices], sorted.
  factory PdfPagesList(Iterable<int> indices) =>
      PdfPagesList._(_checkedIndices('PdfPages.list', indices, sort: true));

  const PdfPagesList._(this.indices);

  /// Zero-based page indices, ascending.
  final List<int> indices;

  @override
  bool operator ==(Object other) =>
      other is PdfPagesList && listsEqual(other.indices, indices);

  @override
  int get hashCode => Object.hashAll(indices);
}

/// [PdfPages.range].
final class PdfPagesRange extends PdfPages {
  /// Creates a page-range scope.
  const PdfPagesRange(this.start, this.end)
    : assert(start >= 0 && start < end, 'a range needs 0 <= start < end');

  /// First page of the range (inclusive).
  final int start;

  /// Page after the last of the range (exclusive).
  final int end;

  @override
  bool operator ==(Object other) =>
      other is PdfPagesRange && other.start == start && other.end == end;

  @override
  int get hashCode => Object.hash(start, end);
}

/// [PdfPages.from].
final class PdfPagesFrom extends PdfPages {
  /// Creates a scope of the pages from [start] to the last.
  const PdfPagesFrom(this.start) : assert(start >= 0, 'start must be >= 0');

  /// Zero-based index of the first page of the scope.
  final int start;

  @override
  bool operator ==(Object other) =>
      other is PdfPagesFrom && other.start == start;

  @override
  int get hashCode => Object.hash(PdfPagesFrom, start);
}

/// A sequence of pages in the order given: the pages a document keeps, in
/// the order it keeps them (`PdfEdits.reorderPages`).
final class PdfPageOrder {
  /// Creates the order [indices]. An empty list, a negative index and an
  /// index given twice throw [ArgumentError].
  factory PdfPageOrder(List<int> indices) =>
      PdfPageOrder._(_checkedIndices('PdfPageOrder', indices, sort: false));

  const PdfPageOrder._(this.indices);

  /// Zero-based page indices, in the order given.
  final List<int> indices;

  @override
  bool operator ==(Object other) =>
      other is PdfPageOrder && listsEqual(other.indices, indices);

  @override
  int get hashCode => Object.hashAll(indices);

  @override
  String toString() => 'PdfPageOrder($indices)';
}

List<int> _checkedIndices(
  String name,
  Iterable<int> indices, {
  required bool sort,
}) {
  final checked = List<int>.of(indices);
  if (checked.isEmpty) {
    throw ArgumentError.value(indices, 'indices', '$name needs a page');
  }
  for (final index in checked) {
    if (index < 0) {
      throw ArgumentError.value(index, 'indices', '$name: a page is negative');
    }
  }
  if (checked.toSet().length != checked.length) {
    throw ArgumentError.value(
      indices,
      'indices',
      '$name: a page is listed twice',
    );
  }
  if (sort) checked.sort();
  return List.unmodifiable(checked);
}
