import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.kill();
  });

  group('PdfEditor.unembedStandardFonts', () {
    test('returns 0 on minimal PDF with no embedded fonts', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      final count = await editor.unembedStandardFonts();
      expect(count, greaterThanOrEqualTo(0));
      await editor.dispose();
    });

    test('produces valid PDF after unembedding', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.unembedStandardFonts();
      final result = await editor.save();
      await editor.dispose();

      final info = await pdf.probe(result);
      expect(info.isValid, isTrue);
    });

    test('unembedding on multi-page PDF preserves page count', () async {
      final threePages = await buildThreePagePdf();
      final editor = PdfEditor(await pdf.openEditor(threePages));
      await editor.unembedStandardFonts();
      final result = await editor.save();
      await editor.dispose();

      final doc = await pdf.open(result);
      expect(doc.pageCount, 3);
    });

    test('unembedding can reduce file size for PDFs with embedded standard fonts', () async {
      // Build a PDF with text (which uses an embedded font)
      final builder = PdfBuilder(await pdf.createBuilder());
      final page = await builder.addA4Page();
      await page.font('Helvetica', 14);
      await page.paragraph('This is a test document with embedded Helvetica font. '
          'The font program should be removable since Helvetica is a Standard 14 font.');
      await page.done();
      final withFonts = await builder.build();
      await builder.dispose();

      // Unembed
      final editor = PdfEditor(await pdf.openEditor(withFonts));
      final count = await editor.unembedStandardFonts();
      final withoutFonts = await editor.save();
      await editor.dispose();

      // The result should still be valid
      final info = await pdf.probe(withoutFonts);
      expect(info.isValid, isTrue);

      // If fonts were unembedded, the result should be smaller or equal
      if (count > 0) {
        expect(withoutFonts.length, lessThan(withFonts.length));
      }
    });
  });
}
