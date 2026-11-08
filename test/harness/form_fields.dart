// Reads the AcroForm field named [name] out of a document's fields.

import 'package:pdf_manipulator/pdf_manipulator.dart';

/// The field named [name], or null when the document has no field with
/// that fully qualified name.
Future<PdfFormField?> formFieldNamed(PdfReads reads, String name) async {
  for (final field in await reads.formFields()) {
    if (field.name == name) return field;
  }
  return null;
}
