import 'package:meta/meta.dart';

/// What a document's XFA packet declares. `PdfDoc.xfa` is `null` when the
/// document has no XFA at all, so this type never describes absence.
@immutable
class PdfXfaInfo {
  /// Creates XFA facts from the engine's analysis.
  const PdfXfaInfo({
    required this.fieldCount,
    required this.pageCount,
    required this.fieldTypes,
  });

  /// Number of fields in the XFA template, or `null` when the packet
  /// declares none.
  final int? fieldCount;

  /// Number of pages the XFA template lays out, or `null` when the packet
  /// declares none.
  final int? pageCount;

  /// The distinct field types the template uses, sorted — the analyzer's
  /// own names, such as `Text` and `Checkbox`.
  final List<String> fieldTypes;
}
