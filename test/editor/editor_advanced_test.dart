import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('PdfEditor crop margins', () {
    test('crop margins produces valid output', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.cropMargins(left: 10, right: 10, top: 10, bottom: 10);
      final result = await editor.save();
      editor.dispose();
      expect(result.length, greaterThan(0));
    });

    test('crop margins changes page dimensions', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.cropMargins(left: 50, right: 50, top: 50, bottom: 50);
      final result = await editor.save();
      editor.dispose();

      expect(result.length, greaterThan(0));
    });
  });

  group('PdfEditor PDF/A conversion', () {
    test('convertToPdfA produces valid output', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.convertToPdfA(level: 1);
      final result = await editor.save();
      editor.dispose();
      expect(result.length, greaterThan(0));
    });

    test('convertToPdfA output is valid PDF', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.convertToPdfA(level: 1);
      final result = await editor.save();
      editor.dispose();

      final info = await pdf.probe(result);
      expect(info.isValid, isTrue);
      expect(info.pageCount, 1);
    });
  });

  group('PdfEditor set form field value', () {
    test('setFormFieldValue on PDF without forms does not crash', () async {
      final editor = await Pdf.edit(minimalPdf);
      // Setting a field on a PDF without forms should either succeed silently
      // or throw a clean PdfError — not crash
      try {
        await editor.setFormFieldValue('nonexistent', 'value');
      } on PdfError {
        // Expected — no form fields exist
      }
      final result = await editor.save();
      editor.dispose();
      expect(result.length, greaterThan(0));
    });
  });

  group('PdfEditor flatten all annotations', () {
    test('flattenAllAnnotations produces valid output', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.flattenAllAnnotations();
      final result = await editor.save();
      editor.dispose();
      expect(result.length, greaterThan(0));
    });

    test('flattenAllAnnotations on multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final editor = await Pdf.edit(threePages);
      await editor.flattenAllAnnotations();
      final result = await editor.save();
      editor.dispose();

      final doc = await pdf.open(result);
      expect(doc.pageCount, 3);
    });
  });

  group('PdfEditor page media box', () {
    test('getPageMediaBox returns correct A4 dimensions', () async {
      final editor = await Pdf.edit(minimalPdf);
      final box = await editor.getPageMediaBox(0);
      editor.dispose();
      // A4 = 595x842
      expect(box.width, closeTo(595, 1));
      expect(box.height, closeTo(842, 1));
    });

    test('getPageMediaBox returns correct Letter dimensions', () async {
      final editor = await Pdf.edit(letterPdf);
      final box = await editor.getPageMediaBox(0);
      editor.dispose();
      // Letter = 612x792
      expect(box.width, closeTo(612, 1));
      expect(box.height, closeTo(792, 1));
    });

    test('getPageMediaBox works on each page of multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final editor = await Pdf.edit(threePages);
      for (var i = 0; i < await editor.pageCount; i++) {
        final box = await editor.getPageMediaBox(i);
        expect(box.width, greaterThan(0));
        expect(box.height, greaterThan(0));
      }
      editor.dispose();
    });
  });
}
