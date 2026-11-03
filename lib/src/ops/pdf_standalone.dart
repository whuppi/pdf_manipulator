// Standalone ops — source in, sink out, no persistent handle exposed to callers.
//
// These never belong on PdfEditor (editor = mutations only).
// Even if the Rust engine implements them on DocumentEditor internally,
// the Dart API exposes them here as one-shot ops on Pdf.

part of 'pdf.dart';

/// One-shot operations — source in, sink out, no persistent handle.
extension PdfStandalone on Pdf {
  /// Digitally signs a PDF with the given [credentials]. An encrypted
  /// [source] throws [PdfUnsupported]: the appended signature would have to
  /// be encrypted with the source's keys.
  PdfTask<void> sign(
    DataSource source,
    DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  }) {
    KeepRecord.op('signatures');
    return _live.sign(
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
    return _live.convertTo(source, output, format: format, password: password);
  }

  /// Converts a document of the given [format] into a PDF.
  PdfTask<void> convertToPdf(
    DataSource document,
    DataSink output, {
    required PdfDocumentFormat format,
  }) {
    KeepRecord.op('office');
    return _live.convertToPdf(document, output, format: format);
  }

  /// Converts the XFA form in [source] into a plain AcroForm document
  /// written to [output]. A document without XFA throws [PdfEngineError].
  PdfTask<void> convertXfaToAcroForm(
    DataSource source,
    DataSink output, {
    String? password,
  }) => _live.convertXfaToAcroForm(source, output, password: password);
}
