// Standalone ops — source in, sink out, no persistent handle exposed to callers.
//
// These never belong on PdfEditor (editor = mutations only).
// Even if the Rust engine implements them on DocumentEditor internally,
// the Dart API exposes them here as one-shot ops on Pdf.

import 'package:pdf_manipulator/src/ops/pdf.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/keep/record_use_shim.dart';

/// One-shot operations — source in, sink out, no persistent handle.
extension PdfStandalone on Pdf {
  /// Digitally signs a PDF with the given [credentials].
  PdfTask<void> sign(
    DataSource source,
    DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  }) {
    KeepRecord.op('signatures');
    return bridge.sign(
      source,
      output,
      credentials: credentials,
      reason: reason,
      location: location,
    );
  }

  /// Converts a PDF to another document [format] (e.g. DOCX, image).
  PdfTask<void> convertTo(
    DataSource source,
    DataSink output, {
    required PdfDocumentFormat format,
    String? password,
  }) {
    KeepRecord.op('office');
    return bridge.convertTo(source, output, format: format, password: password);
  }

  /// Converts a document of the given [format] into a PDF.
  PdfTask<void> convertToPdf(
    DataSource document,
    DataSink output, {
    required PdfDocumentFormat format,
  }) {
    KeepRecord.op('office');
    return bridge.convertToPdf(document, output, format: format);
  }

  /// Extracts specific [pages] from [source] into [output].
  PdfTask<void> extractPages(
    DataSource source,
    DataSink output, {
    required List<int> pages,
    String? password,
  }) => PdfTask.group((hook) async {
    final handle = await hook.guard(
      bridge.openEditor(source, password: password),
    );
    try {
      await hook.guard(handle.extractPages(pages, output));
    } finally {
      await handle.dispose();
    }
  });
}
