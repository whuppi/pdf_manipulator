// Standalone ops — source in, sink out, no persistent handle exposed to callers.
//
// These never belong on PdfEditor (editor = mutations only).
// Even if the Rust engine implements them on DocumentEditor internally,
// the Dart API exposes them here as one-shot ops on Pdf.

import 'package:pdf_manipulator/src/ops/pdf.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';

extension PdfStandalone on Pdf {
  Future<void> sign(DataSource source, DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  }) {
    return bridge.sign(source, output,
        credentials: credentials, reason: reason, location: location);
  }

  Future<void> convertTo(DataSource source, DataSink output, {
    required PdfDocumentFormat format,
    String? password,
  }) {
    return bridge.convertTo(source, output, format: format, password: password);
  }

  Future<void> convertToPdf(DataSource document, DataSink output, {
    required PdfDocumentFormat format,
  }) {
    return bridge.convertToPdf(document, output, format: format);
  }

  Future<void> extractPages(
    DataSource source,
    DataSink output, {
    required List<int> pages,
    String? password,
  }) async {
    final handle = await bridge.openEditor(source, password: password);
    try {
      await handle.extractPages(pages, output);
    } finally {
      await handle.dispose();
    }
  }
}
