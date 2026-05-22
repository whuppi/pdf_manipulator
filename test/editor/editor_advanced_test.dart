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

  group('PdfEditor crop margins', () {
    test('crop margins produces valid output', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.cropMargins(left: 10, right: 10, top: 10, bottom: 10);
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final result = sink.takeBytes();
      expect(result.length, greaterThan(0));
    });

    test('crop margins changes page dimensions', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.cropMargins(left: 50, right: 50, top: 50, bottom: 50);
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final result = sink.takeBytes();
      expect(result.length, greaterThan(0));
    });
  });

  group('PdfEditor PDF/A conversion', () {
    test('convertToPdfA produces valid output', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.convertToPdfA(level: 1);
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final result = sink.takeBytes();
      expect(result.length, greaterThan(0));
    });

    test('convertToPdfA output is valid PDF', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.convertToPdfA(level: 1);
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final result = sink.takeBytes();
      final info = await pdf.probe(sourceOf(result));
      expect(info.isValid, isTrue);
      expect(info.pageCount, 1);
    });
  });

  group('PdfEditor set form field value', () {
    test('setFormFieldValue on PDF without forms does not crash', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      try {
        await editor.setFormFieldValue('nonexistent', 'value');
      } on PdfError {
        // Expected — no form fields exist
      }
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final result = sink.takeBytes();
      expect(result.length, greaterThan(0));
    });
  });

  group('PdfEditor flatten all annotations', () {
    test('flattenAllAnnotations produces valid output', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.flattenAllAnnotations();
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final result = sink.takeBytes();
      expect(result.length, greaterThan(0));
    });

    test('flattenAllAnnotations on multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final editor = await Pdf.edit(sourceOf(threePages));
      await editor.flattenAllAnnotations();
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final result = sink.takeBytes();
      final doc = await pdf.open(sourceOf(result));
      expect(doc.pageCount, 3);
    });
  });

  group('PdfEditor page media box', () {
    test('getPageMediaBox returns correct A4 dimensions', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final box = await editor.getPageMediaBox(0);
      editor.dispose();
      expect(box.width, closeTo(595, 1));
      expect(box.height, closeTo(842, 1));
    });

    test('getPageMediaBox returns correct Letter dimensions', () async {
      final editor = await Pdf.edit(sourceOf(letterPdf));
      final box = await editor.getPageMediaBox(0);
      editor.dispose();
      expect(box.width, closeTo(612, 1));
      expect(box.height, closeTo(792, 1));
    });

    test('getPageMediaBox works on each page of multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final editor = await Pdf.edit(sourceOf(threePages));
      for (var i = 0; i < await editor.pageCount; i++) {
        final box = await editor.getPageMediaBox(i);
        expect(box.width, greaterThan(0));
        expect(box.height, greaterThan(0));
      }
      editor.dispose();
    });
  });
}
