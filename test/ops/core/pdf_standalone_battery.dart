// CHARTER — this battery alone proves: the standalone ops (sign,
// extractPages, office conversion) transform source→sink correctly
// with no handle exposed. These ops live here and nowhere else —
// sugar composes extractPages internally but never re-proves it.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerStandaloneTests(Pdf Function() createPdf) {
  group('standalone', () {
    // ── Sign ──

    test('sign with PKCS12 adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(
        src(minimalPdf),
        sink,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final signed = sink.takeBytes();
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);
      expect(signed.length, greaterThan(minimalPdf.length));
      final sigs = await doc.getSignatures();
      expect(sigs, isNotEmpty);
      expect(sigs.first.signerName, isNotNull);
      expect(sigs.first.signerName, isNotEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('sign with PEM adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(
        src(minimalPdf),
        sink,
        credentials: const PdfSigningCredentials.pem(testCertPem, testKeyPem),
      );
      final signed = sink.takeBytes();
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);
      final sigs = await doc.getSignatures();
      expect(sigs, isNotEmpty);
      expect(sigs.first.signerName, isNotNull);
      await doc.dispose();
    }, timeout: t(1));

    test('sign with invalid cert throws', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await expectLater(
        pdf.sign(
          src(minimalPdf),
          sink,
          credentials: PdfSigningCredentials.pkcs12(
            Uint8List.fromList([1, 2, 3, 4]),
            'wrong',
          ),
        ),
        throwsA(isA<PdfEngineError>()),
        reason:
            'a broken certificate is an engine failure with a '
            'message — never an untyped throw',
      );
    }, timeout: t(1));

    // ── Extract pages ──

    test(
      'extractPages with an out-of-range index throws typed error',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await expectLater(
          pdf.extractPages(src(minimalPdf), sink, pages: const [99]),
          throwsA(isA<PdfEngineError>()),
          reason:
              'page 99 of a 1-page PDF does not exist — silently '
              'producing an empty PDF would hide caller bugs',
        );
      },
      timeout: t(1),
    );

    test('extractPages keeps exactly the selected pages and content', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.extractPages(src(fThreePageMarkers), sink, pages: [0, 2]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains(fThreePageMarkersTruth.markers[0]));
      expect(text, contains(fThreePageMarkersTruth.markers[2]));
      expect(
        text,
        isNot(contains(fThreePageMarkersTruth.markers[1])),
        reason:
            'the unselected page must not smuggle its content '
            'into the output',
      );
      await doc.dispose();
    }, timeout: t(1));

    // ── Convert ──

    test('convertTo DOCX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        sink,
        format: PdfDocumentFormat.docx,
      );
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50); // PK header
      expect(bytes[1], 0x4B);
    }, timeout: t(1));

    test('convertTo PPTX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        sink,
        format: PdfDocumentFormat.pptx,
      );
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: t(1));

    test('convertTo XLSX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        sink,
        format: PdfDocumentFormat.xlsx,
      );
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: t(1));

    test('convertToPdf from DOCX produces valid PDF', () async {
      final pdf = createPdf();
      final docxSink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        docxSink,
        format: PdfDocumentFormat.docx,
      );
      final pdfSink = TestSink();
      await pdf.convertToPdf(
        src(docxSink.takeBytes()),
        pdfSink,
        format: PdfDocumentFormat.docx,
      );
      final pdfBytes = pdfSink.takeBytes();
      expect(String.fromCharCodes(pdfBytes.sublist(0, 5)), startsWith('%PDF'));
    }, timeout: t(1));
  });
}
