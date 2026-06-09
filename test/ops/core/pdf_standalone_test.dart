// PdfStandalone — source in, sink out, no handle exposed.
// Mirrors lib/src/ops/pdf_standalone.dart.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/fixtures.dart';
import '../../helpers/test_source_sink.dart';

void registerStandaloneTests(Pdf Function() createPdf) {
  group('standalone', () {
    // ── Sign ──

    test('sign with PKCS12 adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(src(minimalPdf), sink,
          credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'));
      final signed = sink.takeBytes();
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);
      expect(signed.length, greaterThan(minimalPdf.length));
      final sigs = await doc.getSignatures();
      expect(sigs, isNotEmpty);
      expect(sigs.first.signerName, isNotNull);
      expect(sigs.first.signerName, isNotEmpty);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('sign with PEM adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(src(minimalPdf), sink,
          credentials: const PdfSigningCredentials.pem(testCertPem, testKeyPem));
      final signed = sink.takeBytes();
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);
      final sigs = await doc.getSignatures();
      expect(sigs, isNotEmpty);
      expect(sigs.first.signerName, isNotNull);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('sign with invalid cert throws', () async {
      final pdf = createPdf();
      final sink = TestSink();
      expect(
        () => pdf.sign(src(minimalPdf), sink,
            credentials: PdfSigningCredentials.pkcs12(
                Uint8List.fromList([1, 2, 3, 4]), 'wrong')),
        throwsA(anything),
      );
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Extract pages ──

    test('extractPages keeps selected pages', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf), src(minimalPdf)], mergeSink);
      final threePage = mergeSink.takeBytes();

      final sink = TestSink();
      await pdf.extractPages(src(threePage), sink, pages: [0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Convert ──

    test('convertTo DOCX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(minimalPdf), sink, format: PdfDocumentFormat.docx);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50); // PK header
      expect(bytes[1], 0x4B);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('convertTo PPTX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(minimalPdf), sink, format: PdfDocumentFormat.pptx);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('convertTo XLSX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(minimalPdf), sink, format: PdfDocumentFormat.xlsx);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('convertToPdf from DOCX produces valid PDF', () async {
      final pdf = createPdf();
      final docxSink = TestSink();
      await pdf.convertTo(src(minimalPdf), docxSink, format: PdfDocumentFormat.docx);
      final pdfSink = TestSink();
      await pdf.convertToPdf(src(docxSink.takeBytes()), pdfSink, format: PdfDocumentFormat.docx);
      final pdfBytes = pdfSink.takeBytes();
      expect(String.fromCharCodes(pdfBytes.sublist(0, 5)), startsWith('%PDF'));
    }, timeout: Timeout(Duration(seconds: 1)));
  });
}
