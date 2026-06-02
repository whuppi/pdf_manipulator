// PdfDoc — read-only queries on an open document handle.
// Every test is behavioral: proves the op does what it claims, not just
// that it doesn't crash.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/fixtures.dart';
import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';

void registerDocTests(Pdf Function() createPdf) {
  group('doc', () {
    // ── Open + page info ──────────────────────────────────────────

    test('open returns pageCount 1 for minimal PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('open returns version containing dot', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.version, contains('.'));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('A4 dimensions are 595×842', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.pages, hasLength(1));
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[0].height, closeTo(842, 1));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('Letter dimensions are 612×792', () async {
      final doc = await createPdf().open(src(letterPdf));
      expect(doc.pages[0].width, closeTo(612, 1));
      expect(doc.pages[0].height, closeTo(792, 1));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('isEncrypted false for unencrypted PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.isEncrypted, isFalse);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('multi-page PDF has correct page count', () async {
      final bytes = await buildTwoPageTextPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 3)));

    // ── Open with password ────────────────────────────────────────

    test('open encrypted PDF with correct password succeeds', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.encrypt(src(minimalPdf), encSink,
          encryption: const PdfEncryptionConfig(ownerPassword: 'testpw'));
      final encBytes = encSink.takeBytes();

      final doc = await pdf.open(src(encBytes), password: 'testpw');
      expect(doc.isEncrypted, isTrue);
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 3)));

    // ── Extract text ──────────────────────────────────────────────

    test('extract from blank page returns empty', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final text = await doc.extract(pages: const PdfPages.single(0));
      expect(text.trim(), isEmpty);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('extract all pages returns text from both pages', () async {
      final bytes = await buildTwoPageTextPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains('ALPHA'));
      expect(text, contains('BRAVO'));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 3)));

    test('extract single page returns only that page text', () async {
      final bytes = await buildTwoPageTextPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final page0 = await doc.extract(pages: const PdfPages.single(0));
      final page1 = await doc.extract(pages: const PdfPages.single(1));
      expect(page0, contains('ALPHA'));
      expect(page0, isNot(contains('BRAVO')));
      expect(page1, contains('BRAVO'));
      expect(page1, isNot(contains('ALPHA')));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 3)));

    test('extract range returns subset', () async {
      final bytes = await buildTwoPageTextPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final range = await doc.extract(pages: const PdfPages.range(0, 1));
      expect(range, contains('ALPHA'));
      expect(range, isNot(contains('BRAVO')));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 3)));

    test('extract markdown format returns content', () async {
      final bytes = await buildFormPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final md = await doc.extract(
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect(md, contains('Application'));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('extract HTML format returns content', () async {
      final bytes = await buildFormPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final html = await doc.extract(
        pages: const PdfPages.single(0),
        format: PdfExtractionFormat.html,
      );
      expect(html, isNotEmpty);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Search ────────────────────────────────────────────────────

    test('search finds text with page and rect', () async {
      final bytes = await buildFormPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final results = await doc.search(
        query: 'Application',
        pages: const PdfPages.all(),
      );
      expect(results, isNotEmpty);
      for (final r in results) {
        expect(r.text, contains('Application'));
        expect(r.page, greaterThanOrEqualTo(0));
        expect(r.rect.width, greaterThan(0));
      }
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('search for nonexistent term returns empty', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final results = await doc.search(
        query: 'xyznonexistent',
        pages: const PdfPages.all(),
      );
      expect(results, isEmpty);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    // This test catches the search page-filtering bug:
    // if pages param is ignored, both searches return the same count.
    test('search respects page scope', () async {
      final bytes = await buildTwoPageTextPdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final allHits = await doc.search(query: 'ALPHA', pages: const PdfPages.all());
      final page0Hits = await doc.search(query: 'ALPHA', pages: const PdfPages.single(0));
      final page1Hits = await doc.search(query: 'ALPHA', pages: const PdfPages.single(1));

      expect(allHits, isNotEmpty, reason: 'ALPHA should be found in all-page search');
      expect(page0Hits, isNotEmpty, reason: 'ALPHA should be on page 0');
      expect(page1Hits, isEmpty, reason: 'ALPHA should NOT be on page 1');
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 3)));

    // ── Render ────────────────────────────────────────────────────

    test('render single page yields one image with dimensions', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final pages = <RenderedPage>[];
      await for (final page in doc.render(pages: const PdfPages.single(0))) {
        pages.add(page);
      }
      expect(pages, hasLength(1));
      expect(pages[0].width, greaterThan(0));
      expect(pages[0].height, greaterThan(0));
      expect(pages[0].data.length, greaterThan(0));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 5)));

    test('render all pages of 2-page PDF yields 2 results', () async {
      final pdf = createPdf();
      final bytes = await buildTwoPageTextPdf(() => pdf);
      final doc = await pdf.open(src(bytes));
      final pages = <RenderedPage>[];
      await for (final page in doc.render(pages: const PdfPages.all())) {
        pages.add(page);
      }
      expect(pages, hasLength(2));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 5)));

    // ── Extract images ────────────────────────────────────────────

    test('extractImages from blank PDF returns empty', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final images = <PdfImage>[];
      await for (final img in doc.extractImages(pages: const PdfPages.all())) {
        images.add(img);
      }
      expect(images, isEmpty);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    // Builder creates proper XObject images (/Im1 in /Resources/XObject,
    // referenced by Do in content stream). The extractor should find them.
    // If this fails, the content stream parser can't parse builder output.
    test('extractImages finds builder-embedded XObject images', () async {
      final bytes = await buildSingleImagePdf(createPdf);
      final doc = await createPdf().open(src(bytes));
      final images = <PdfImage>[];
      await for (final img in doc.extractImages(pages: const PdfPages.all())) {
        images.add(img);
      }
      expect(images, isNotEmpty, reason: 'Builder XObject image should be extractable');
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 3)));

    // ── Validate ──────────────────────────────────────────────────

    test('validatePdfA on minimal returns non-compliant with errors', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final result = await doc.validatePdfA();
      expect(result.compliant, isFalse);
      expect(result.errors, greaterThan(0));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('validatePdfUa on minimal returns false', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.validatePdfUa(), isFalse);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Classify ──────────────────────────────────────────────────

    test('classifyPage returns type and confidence in range', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final result = await doc.classifyPage(0);
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('classifyDocument returns type and confidence in range', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final result = await doc.classifyDocument();
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Signatures ────────────────────────────────────────────────

    test('getSignatures empty for unsigned PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.getSignatures(), isEmpty);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    test('verifySignatures false for unsigned PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.verifySignatures(), isFalse);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Bookmarks ─────────────────────────────────────────────────

    test('planSplitByBookmarks returns chapter titles', () async {
      final doc = await createPdf().open(src(bookmarkedPdf));
      final splits = await doc.planSplitByBookmarks();
      expect(splits.length, 2);
      expect(splits[0].title, contains('Chapter 1'));
      expect(splits[1].title, contains('Chapter 2'));
      expect(splits[0].startPage, greaterThanOrEqualTo(0));
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Convert ───────────────────────────────────────────────────

    test('convertTo DOCX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(minimalPdf), sink, format: PdfDocumentFormat.docx);
      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(4));
      expect(bytes[0], 0x50); // PK ZIP header
      expect(bytes[1], 0x4B);
    }, timeout: Timeout(Duration(seconds: 3)));

    test('convertTo PPTX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(minimalPdf), sink, format: PdfDocumentFormat.pptx);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: Timeout(Duration(seconds: 3)));

    test('convertTo XLSX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(minimalPdf), sink, format: PdfDocumentFormat.xlsx);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: Timeout(Duration(seconds: 3)));

    test('convertToPdf from DOCX produces valid PDF', () async {
      final pdf = createPdf();
      final docxSink = TestSink();
      await pdf.convertTo(src(minimalPdf), docxSink, format: PdfDocumentFormat.docx);
      final pdfSink = TestSink();
      await pdf.convertToPdf(src(docxSink.takeBytes()), pdfSink, format: PdfDocumentFormat.docx);
      final pdfBytes = pdfSink.takeBytes();
      expect(String.fromCharCodes(pdfBytes.sublist(0, 5)), startsWith('%PDF'));
    }, timeout: Timeout(Duration(seconds: 5)));
  });
}
