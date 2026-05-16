import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.kill();
  });

  group('Pdf.watermark', () {
    test('watermarked PDF is larger than original (annotation added)', () async {
      final result = await pdf.watermark(minimalPdf, text: 'DRAFT');
      expect(result.length, greaterThan(minimalPdf.length));
    });

    test('watermarked PDF preserves page count', () async {
      final result = await pdf.watermark(minimalPdf, text: 'CONFIDENTIAL');
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(1));
    });

    test('watermarked PDF preserves page dimensions', () async {
      final result = await pdf.watermark(minimalPdf, text: 'TEST');
      final doc = await pdf.open(result);
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[0].height, closeTo(842, 1));
    });

    test('watermark with different font sizes both produce valid PDFs', () async {
      final small = await pdf.watermark(minimalPdf, text: 'X', fontSize: 12);
      final large = await pdf.watermark(minimalPdf, text: 'X', fontSize: 96);
      final docSmall = await pdf.open(small);
      final docLarge = await pdf.open(large);
      expect(docSmall.pageCount, equals(1));
      expect(docLarge.pageCount, equals(1));
      // Both should be larger than original (watermark annotation added)
      expect(small.length, greaterThan(minimalPdf.length));
      expect(large.length, greaterThan(minimalPdf.length));
    });

    test('watermark on specific pages only — page 0 only on 2-page PDF', () async {
      final twoPage = await pdf.merge([minimalPdf, minimalPdf]);
      final onlyPage0 = await pdf.watermark(twoPage, text: 'FIRST', pages: [0]);
      final bothPages = await pdf.watermark(twoPage, text: 'FIRST');
      // Watermarking both pages adds more data than just page 0
      expect(bothPages.length, greaterThan(onlyPage0.length));
    });

    test('watermark on multi-page PDF preserves all pages', () async {
      final threePage = await pdf.merge([minimalPdf, minimalPdf, minimalPdf]);
      final result = await pdf.watermark(threePage, text: 'ALL');
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
      // Result should be larger than input (3 annotations added)
      expect(result.length, greaterThan(threePage.length));
    });

    test('different text produces different output', () async {
      final a = await pdf.watermark(minimalPdf, text: 'AAA');
      final b = await pdf.watermark(minimalPdf, text: 'BBBBBBBBB');
      // Longer text = larger appearance stream
      expect(b.length, greaterThan(a.length));
    });
  });

  group('PdfEditor.addWatermark', () {
    test('adds watermark — output is larger', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.addWatermark(0, 'EDITED');
      final saved = await editor.save();
      expect(saved.length, greaterThan(minimalPdf.length));
      await editor.dispose();
    });

    test('watermark marks document as modified', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      expect(await editor.isModified, isFalse);
      await editor.addWatermark(0, 'MOD');
      expect(await editor.isModified, isTrue);
      await editor.dispose();
    });

    test('watermark with custom color produces valid PDF', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.addWatermark(0, 'BLUE', r: 0.0, g: 0.0, b: 1.0);
      final saved = await editor.save();
      await editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(1));
      expect(saved.length, greaterThan(minimalPdf.length));
    });
  });
}
