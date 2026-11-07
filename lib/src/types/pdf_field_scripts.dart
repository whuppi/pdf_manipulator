import 'package:meta/meta.dart';

/// The JavaScript actions of a form field a page builder creates. A null
/// script adds no action.
@immutable
final class PdfFieldScripts {
  /// Creates the scripts of a field.
  const PdfFieldScripts({
    this.keystroke,
    this.format,
    this.validate,
    this.calculate,
  });

  /// Runs on each keystroke in the field.
  final String? keystroke;

  /// Formats the field's value for display.
  final String? format;

  /// Decides whether the field's value is acceptable.
  final String? validate;

  /// Computes the field's value from other fields.
  final String? calculate;

  @override
  bool operator ==(Object other) =>
      other is PdfFieldScripts &&
      other.keystroke == keystroke &&
      other.format == format &&
      other.validate == validate &&
      other.calculate == calculate;

  @override
  int get hashCode => Object.hash(keystroke, format, validate, calculate);
}
