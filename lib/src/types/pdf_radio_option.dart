import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// One button of a radio group: the export [value] it selects and where it
/// sits.
@immutable
final class PdfRadioOption {
  /// Creates the option [value] drawn in [bounds].
  const PdfRadioOption(this.value, this.bounds);

  /// The export value the group takes when this button is selected.
  final String value;

  /// Where the button sits on the page.
  final PdfRect bounds;

  @override
  bool operator ==(Object other) =>
      other is PdfRadioOption && other.value == value && other.bounds == bounds;

  @override
  int get hashCode => Object.hash(value, bounds);
}
