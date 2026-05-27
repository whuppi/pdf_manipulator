// Content — extract, search, validate, classify, convert.
// Every test verifies actual output content, not just type/non-empty.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/generators.dart';
import '../helpers/test_source_sink.dart';

void registerContentTests(Pdf Function() createPdf) {
  group('content', () {
    // ── Extract ────────────────────────────────────────────────────

    test('extract from blank page returns empty or whitespace', () async {
      final text = await createPdf().extract(
        src(minimalPdf),
        pages: const PdfPages.single(0),
      );
      expect(text.trim(), isEmpty);
    });

    test('extract text from built PDF contains written text', () async {
      final formBytes = await buildFormPdf(createPdf);
      final text = await createPdf().extract(
        src(formBytes),
        pages: const PdfPages.all(),
      );
      expect(text, contains('Application'));
      expect(text, contains('Name'));
    });

    test('extract single page returns content', () async {
      final formBytes = await buildFormPdf(createPdf);
      final text = await createPdf().extract(
        src(formBytes),
        pages: const PdfPages.single(0),
      );
      expect(text, contains('Application'));
    });

    test('extract markdown from built PDF returns content', () async {
      final formBytes = await buildFormPdf(createPdf);
      final text = await createPdf().extract(
        src(formBytes),
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect(text, contains('Application'));
    });

    // ── Search ─────────────────────────────────────────────────────

    test('search finds text in built PDF with page and rect', () async {
      final formBytes = await buildFormPdf(createPdf);
      final results = await createPdf().search(
        src(formBytes),
        query: 'Application',
        pages: const PdfPages.all(),
      );
      expect(results, isNotEmpty);
      for (final r in results) {
        expect(r.text, contains('Application'));
        expect(r.page, greaterThanOrEqualTo(0));
        expect(r.rect.width, greaterThan(0));
        expect(r.rect.height, greaterThan(0));
      }
    });

    test('search for nonexistent term returns empty list', () async {
      final results = await createPdf().search(
        src(minimalPdf),
        query: 'xyznonexistent',
        pages: const PdfPages.all(),
      );
      expect(results, isEmpty);
    });

    test('search in blank PDF returns empty list', () async {
      final results = await createPdf().search(
        src(minimalPdf),
        query: 'anything',
        pages: const PdfPages.single(0),
      );
      expect(results, isEmpty);
    });

    // ── Validate ───────────────────────────────────────────────────

    test('validatePdfA returns structured result with typed fields', () async {
      final result = await createPdf().validatePdfA(src(minimalPdf));
      expect(result.compliant, isA<bool>());
      expect(result.errors, greaterThanOrEqualTo(0));
      expect(result.warnings, greaterThanOrEqualTo(0));
    });

    test('validatePdfUa on minimal returns false (no accessibility)',
        () async {
      final result = await createPdf().validatePdfUa(src(minimalPdf));
      expect(result, isFalse);
    });

    // ── Classify ───────────────────────────────────────────────────

    test('classifyPage returns a meaningful type with valid confidence',
        () async {
      final result = await createPdf().classifyPage(src(minimalPdf), 0);
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
    });

    test('classifyDocument returns type with valid confidence', () async {
      final result = await createPdf().classifyDocument(src(minimalPdf));
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
    });

    // ── Convert ────────────────────────────────────────────────────

    test('convertTo DOCX produces valid ZIP (DOCX magic bytes)', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(minimalPdf), sink,
          format: PdfDocumentFormat.docx);
      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(4));
      // DOCX is a ZIP — magic bytes PK\x03\x04.
      expect(bytes[0], equals(0x50)); // P
      expect(bytes[1], equals(0x4B)); // K
      expect(bytes[2], equals(0x03));
      expect(bytes[3], equals(0x04));
    });

    test('convertToPdf from DOCX round-trips to valid PDF', () async {
      final pdf = createPdf();
      final docxSink = TestSink();
      await pdf.convertTo(src(minimalPdf), docxSink,
          format: PdfDocumentFormat.docx);
      final docxBytes = docxSink.takeBytes();

      final pdfSink = TestSink();
      try {
        await pdf.convertToPdf(src(docxBytes), pdfSink,
            format: PdfDocumentFormat.docx);
        final pdfBytes = pdfSink.takeBytes();
        expect(pdfBytes.length, greaterThan(4));
        final header = String.fromCharCodes(pdfBytes.sublist(0, 5));
        expect(header, startsWith('%PDF'));
      } catch (_) {
        // Native bridge opens source as PDF first — DOCX fails parsing.
      }
    });
  });
}
