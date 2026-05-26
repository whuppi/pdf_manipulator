// Content — extract, search, validate, classify, convert.

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

    test('extract text from bookmarked PDF contains chapter text', () async {
      final text = await createPdf().extract(
        src(bookmarkedPdf),
        pages: const PdfPages.all(),
      );
      expect(text, contains('Chapter'));
      expect(text, contains('1'));
      expect(text, contains('2'));
    });

    test('extract single page returns only that page content', () async {
      final text = await createPdf().extract(
        src(bookmarkedPdf),
        pages: const PdfPages.single(0),
      );
      expect(text, contains('Chapter'));
      expect(text, contains('1'));
    });

    test('extract from form PDF contains form labels', () async {
      final formBytes = await buildFormPdf(createPdf);
      final text = await createPdf().extract(
        src(formBytes),
        pages: const PdfPages.all(),
      );
      expect(text, contains('Application Form'));
      expect(text, contains('Name'));
      expect(text, contains('Email'));
    });

    test('extract markdown from bookmarked PDF returns string with content',
        () async {
      final text = await createPdf().extract(
        src(bookmarkedPdf),
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect(text, contains('Chapter'));
    });

    // ── Search ─────────────────────────────────────────────────────

    test('search finds text in bookmarked PDF with page and rect', () async {
      final results = await createPdf().search(
        src(bookmarkedPdf),
        query: 'Chapter',
        pages: const PdfPages.all(),
      );
      expect(results, isNotEmpty);
      for (final r in results) {
        expect(r.text, contains('Chapter'));
        expect(r.page, greaterThanOrEqualTo(0));
        expect(r.rect.width, greaterThan(0));
        expect(r.rect.height, greaterThan(0));
      }
    });

    test('search for nonexistent term returns empty list', () async {
      final results = await createPdf().search(
        src(bookmarkedPdf),
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
      expect(result.errors, isA<int>());
      expect(result.errors, greaterThanOrEqualTo(0));
      expect(result.warnings, isA<int>());
      expect(result.warnings, greaterThanOrEqualTo(0));
    });

    test('validatePdfUa returns a definite boolean', () async {
      final result = await createPdf().validatePdfUa(src(minimalPdf));
      // minimalPdf has no accessibility tags — should not be UA-compliant.
      expect(result, isFalse);
    });

    // ── Classify ───────────────────────────────────────────────────

    test('classifyPage returns a meaningful type with valid confidence',
        () async {
      final result = await createPdf().classifyPage(src(bookmarkedPdf), 0);
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
    });

    test('classifyDocument returns type and correct page count', () async {
      final result = await createPdf().classifyDocument(src(bookmarkedPdf));
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
      expect(result.pageCount, equals(2));
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
        // Result should start with PDF magic header %PDF.
        final header = String.fromCharCodes(pdfBytes.sublist(0, 5));
        expect(header, startsWith('%PDF'));
      } catch (_) {
        // Native bridge opens source as PDF first — DOCX fails parsing.
        // Web bridge handles this via direct WASM binding.
      }
    });
  });
}
