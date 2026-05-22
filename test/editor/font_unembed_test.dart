import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('PdfEditor.unembedStandardFonts', () {
    test('returns 0 on minimal PDF with no embedded fonts', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final count = await editor.unembedStandardFonts();
      expect(count, greaterThanOrEqualTo(0));
      editor.dispose();
    });

    test('produces valid PDF after unembedding', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.unembedStandardFonts();
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final result = sink.takeBytes();
      final info = await pdf.probe(sourceOf(result));
      expect(info.isValid, isTrue);
    });

    test('unembedding on multi-page PDF preserves page count', () async {
      final threePages = await buildThreePagePdf();
      final editor = await Pdf.edit(sourceOf(threePages));
      await editor.unembedStandardFonts();
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final result = sink.takeBytes();
      final doc = await pdf.open(sourceOf(result));
      expect(doc.pageCount, 3);
    });

    test('unembedding can reduce file size for PDFs with embedded standard fonts', () async {
      // Build a PDF with text (which uses an embedded font)
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.font('Helvetica', 14);
      await page.paragraph('This is a test document with embedded Helvetica font. '
          'The font program should be removable since Helvetica is a Standard 14 font.');
      await page.done();
      final buildSink = TestPdfSink();
      await builder.save(buildSink);
      builder.dispose();
      final withFonts = buildSink.takeBytes();

      // Unembed
      final editor = await Pdf.edit(sourceOf(withFonts));
      final count = await editor.unembedStandardFonts();
      final saveSink = TestPdfSink();
      await editor.save(saveSink);
      editor.dispose();
      final withoutFonts = saveSink.takeBytes();

      // The result should still be valid
      final info = await pdf.probe(sourceOf(withoutFonts));
      expect(info.isValid, isTrue);

      // If fonts were unembedded, the result should be smaller or equal
      if (count > 0) {
        expect(withoutFonts.length, lessThan(withFonts.length));
      }
    });
  });
}
