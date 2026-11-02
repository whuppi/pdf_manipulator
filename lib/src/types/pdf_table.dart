// Table types for the builder's table ops.

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_wire.dart';

/// A table column with a header, a width and cell alignment.
///
/// [width] is in points; a table that sizes its columns itself measures
/// them, and a table that does not needs a width on every column.
final class PdfTableColumn {
  /// Creates a column; [header] is its title text.
  const PdfTableColumn(
    this.header, {
    this.width,
    this.align = PdfAlignment.left,
  });

  /// Header text.
  final String header;

  /// Column width in points.
  final double? width;

  /// Text alignment inside the column.
  final PdfAlignment align;

  @override
  bool operator ==(Object other) =>
      other is PdfTableColumn &&
      other.header == header &&
      other.width == width &&
      other.align == align;

  @override
  int get hashCode => Object.hash(header, width, align);
}

/// One cell of a table row.
final class PdfTableCell {
  /// Creates a cell; [text] is its content.
  const PdfTableCell(this.text, {this.rowSpan = 1});

  /// Cell text.
  final String text;

  /// Rows this cell spans downward (1 = this row only). Each row still
  /// lists one cell per column; a cell a span from above covers is a
  /// placeholder and is written empty.
  final int rowSpan;

  @override
  bool operator ==(Object other) =>
      other is PdfTableCell && other.text == text && other.rowSpan == rowSpan;

  @override
  int get hashCode => Object.hash(text, rowSpan);
}

/// How a table sizes its columns.
enum PdfTableSizing implements PdfWire {
  /// No measuring — every column needs an explicit width.
  fixed('fixed'),

  /// Sample the first rows and size columns from their content, clamped
  /// to the min/max widths. A column without a width takes the engine's
  /// default before it is measured.
  sample('sample'),

  /// Measure every row before the table is written. The engine measures
  /// one sample, so the table sends every row as the sample.
  autoAll('auto_all');

  const PdfTableSizing(this.wire);

  @override
  final String wire;
}

/// How a table is laid out: its header row, its column sizing and the
/// spans its cells may carry.
final class PdfTableOptions {
  /// Creates the options of a table.
  const PdfTableOptions({
    this.header = true,
    this.repeatHeader = true,
    this.sizing = PdfTableSizing.fixed,
    this.sampleRows = 50,
    this.minColumnWidth = 20,
    this.maxColumnWidth = 400,
    this.maxRowSpan = 1,
  });

  /// Draw the header row.
  final bool header;

  /// Draw the header row again on every page the table runs over.
  final bool repeatHeader;

  /// How the columns are sized: [PdfTableSizing.fixed] needs a width on
  /// every column, [PdfTableSizing.sample] and [PdfTableSizing.autoAll]
  /// measure the columns themselves.
  final PdfTableSizing sizing;

  /// How many rows are measured before the columns are sized, in
  /// [PdfTableSizing.sample] mode.
  final int sampleRows;

  /// The narrowest a measured column may become, in points.
  final double minColumnWidth;

  /// The widest a measured column may become, in points.
  final double maxColumnWidth;

  /// How many rows a cell may span.
  final int maxRowSpan;

  @override
  bool operator ==(Object other) =>
      other is PdfTableOptions &&
      other.header == header &&
      other.repeatHeader == repeatHeader &&
      other.sizing == sizing &&
      other.sampleRows == sampleRows &&
      other.minColumnWidth == minColumnWidth &&
      other.maxColumnWidth == maxColumnWidth &&
      other.maxRowSpan == maxRowSpan;

  @override
  int get hashCode => Object.hash(
    header,
    repeatHeader,
    sizing,
    sampleRows,
    minColumnWidth,
    maxColumnWidth,
    maxRowSpan,
  );

  @override
  String toString() =>
      'PdfTableOptions(header: $header, repeatHeader: $repeatHeader, '
      'sizing: $sizing, sampleRows: $sampleRows, '
      'minColumnWidth: $minColumnWidth, maxColumnWidth: $maxColumnWidth, '
      'maxRowSpan: $maxRowSpan)';
}
