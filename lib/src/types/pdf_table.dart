// Table types for the builder's table ops.

/// Horizontal alignment of a table column's text.
enum PdfCellAlign {
  /// Align to the left edge.
  left,

  /// Center horizontally.
  center,

  /// Align to the right edge.
  right,
}

/// A table column with a header, optional width and cell alignment.
///
/// [width] in points; `null` lets the engine size the column to fit its
/// content. `streamingTable` requires an explicit width on every column.
class PdfTableColumn {
  /// Creates a column; [header] is its title text.
  const PdfTableColumn(
    this.header, {
    this.width,
    this.align = PdfCellAlign.left,
  });

  /// Header text.
  final String header;

  /// Column width in points, or `null` for auto sizing.
  final double? width;

  /// Text alignment inside the column.
  final PdfCellAlign align;
}

/// One cell of a streaming table row.
class PdfTableCell {
  /// Creates a cell; [text] is its content.
  const PdfTableCell(this.text, {this.rowSpan = 1});

  /// Cell text.
  final String text;

  /// Rows this cell spans downward (1 = this row only). Each row still
  /// lists one cell per column; a cell a span from above covers is a
  /// placeholder and is written empty.
  final int rowSpan;
}

/// How a streaming table sizes its columns.
enum PdfTableSizing {
  /// No measuring — columns use their explicit widths.
  fixed('fixed'),

  /// Sample the first rows and size columns from their content, clamped
  /// to the min/max widths.
  sample('sample'),

  /// Measure every row before streaming (buffered table only).
  autoAll('auto_all');

  const PdfTableSizing(this.wire);

  /// The wire-mode string the engine's table config expects.
  final String wire;
}
