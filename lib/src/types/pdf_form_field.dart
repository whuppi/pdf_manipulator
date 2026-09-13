// Form field types — reads live on PdfDoc (PdfDoc.formFields), mutations
// on PdfEditor, export on PdfDoc.exportFormData.

import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// The kind of an AcroForm field, refined from the raw `/FT` and `/Ff`
/// bits so a caller never reads a flag directly.
enum PdfFormFieldType {
  /// `/FT /Tx` — single or multi-line text.
  text,

  /// `/FT /Btn` without the Radio or Pushbutton flag.
  checkbox,

  /// `/FT /Btn` with the Radio flag (`PdfFormFieldFlag.radio`).
  radioGroup,

  /// `/FT /Ch` with the Combo flag (`PdfFormFieldFlag.combo`).
  comboBox,

  /// `/FT /Ch` without the Combo flag.
  listBox,

  /// `/FT /Btn` with the Pushbutton flag (`PdfFormFieldFlag.pushButton`).
  button,

  /// `/FT /Sig`.
  signature,

  /// A field type the engine could not classify.
  unknown,
}

/// A field's value, typed by the field kind: [PdfTextValue] for text
/// fields, [PdfCheckedValue] for a checkbox or radio button,
/// [PdfChoiceValue] for a single-select choice field, [PdfMultiChoiceValue]
/// for a multi-select list box, [PdfNoValue] when the field has no value.
sealed class PdfFormFieldValue {
  const PdfFormFieldValue();
}

/// The text content of a text or single-select choice field.
class PdfTextValue extends PdfFormFieldValue {
  /// Creates a text value.
  const PdfTextValue(this.text);

  /// The field's text.
  final String text;
}

/// Whether a checkbox or radio button is on.
class PdfCheckedValue extends PdfFormFieldValue {
  /// Creates a checked value.
  const PdfCheckedValue(this.checked);

  /// Whether the widget is in its on-state.
  final bool checked;
}

/// The selected option of a single-select choice field.
class PdfChoiceValue extends PdfFormFieldValue {
  /// Creates a choice value.
  const PdfChoiceValue(this.choice);

  /// The selected option name.
  final String choice;
}

/// The selected options of a multi-select list box.
class PdfMultiChoiceValue extends PdfFormFieldValue {
  /// Creates a multi-choice value.
  const PdfMultiChoiceValue(this.choices);

  /// The selected option names.
  final List<String> choices;
}

/// A field with no value set.
class PdfNoValue extends PdfFormFieldValue {
  /// Creates a no-value marker.
  const PdfNoValue();
}

/// Text alignment of a text field's content (`/Q`).
enum PdfTextAlignment {
  /// `/Q 0`.
  left,

  /// `/Q 1`.
  center,

  /// `/Q 2`.
  right,
}

/// An AcroForm field as read from a document by `PdfDoc.formFields`.
@immutable
class PdfFormField {
  /// Creates a form field snapshot.
  const PdfFormField({
    required this.name,
    required this.type,
    required this.value,
    this.tooltip,
    this.bounds,
    this.maxLength,
    this.alignment,
    this.readOnly = false,
    this.required = false,
  });

  /// Fully qualified (dotted) field name — what every mutator on
  /// `PdfEditor` takes.
  final String name;

  /// The field's kind.
  final PdfFormFieldType type;

  /// The field's current value.
  final PdfFormFieldValue value;

  /// `/TU`, the tooltip shown when a viewer hovers the field.
  final String? tooltip;

  /// `/Rect` of the field's first widget, in points.
  final PdfRect? bounds;

  /// `/MaxLen`, the maximum number of characters a text field accepts.
  final int? maxLength;

  /// `/Q`, the text field's content alignment.
  final PdfTextAlignment? alignment;

  /// `/Ff` bit 1 — the field cannot be edited by the user.
  final bool readOnly;

  /// `/Ff` bit 2 — the field must have a value on submit.
  final bool required;
}

/// Format for `PdfDoc.exportFormData`.
enum PdfFormDataFormat {
  /// Binary Forms Data Format (ISO 32000-1 §12.7.7).
  fdf,

  /// XML Forms Data Format.
  xfdf,
}

/// A form field flag bit (`/Ff`), positioned per ISO 32000-1:2008 Table
/// 221 (common), 226 (button), 228 (choice) and 230 (text). Pass a set of
/// these to `PdfEditor.setFormFieldFlags`.
enum PdfFormFieldFlag {
  /// Table 221 bit 1 — the field cannot be edited by the user.
  readOnly(1),

  /// Table 221 bit 2 — the field must have a value on submit.
  required(1 << 1),

  /// Table 221 bit 3 — excluded from form export.
  noExport(1 << 2),

  /// Table 230 bit 13 — a text field accepts multiple lines.
  multiline(1 << 12),

  /// Table 230 bit 14 — a text field masks its input.
  password(1 << 13),

  /// Table 226 bit 16 — a button field is a radio group.
  radio(1 << 15),

  /// Table 226 bit 17 — a button field is a pushbutton.
  pushButton(1 << 16),

  /// Table 228 bit 18 — a choice field is a combo box.
  combo(1 << 17),

  /// Table 228 bit 19 — a combo box allows a custom typed value.
  edit(1 << 18),

  /// Table 228 bit 20 — a choice field's options are sorted.
  sort(1 << 19),

  /// Table 228 bit 22 — a list box allows multiple selections.
  multiSelect(1 << 21),

  /// Table 230 bit 23 — a text field's input is not spell-checked.
  doNotSpellCheck(1 << 22),

  /// Table 230 bit 24 — a text field does not scroll past its width.
  doNotScroll(1 << 23),

  /// Table 230 bit 25 — a text field splits input into equal-width combs.
  comb(1 << 24),

  /// Table 230 bit 26 — a text field's value is rich text.
  richText(1 << 25);

  /// Creates a flag with its ISO field-flag bit value.
  const PdfFormFieldFlag(this.bit);

  /// The raw `/Ff` bit value.
  final int bit;
}
