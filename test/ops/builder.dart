// Builder — create PDF from scratch: pages, text, metadata, imagesToPdf.
// Every test verifies content was written, not just "bytes exist."

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerBuilderTests(Pdf Function() createPdf) {
  group('builder', () {
    test('create → text → save → verify text in output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage(width: 612, height: 792);
      await page.text('Hello World');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();

      // Re-open to verify valid PDF with 1 page.
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);

      // The text should appear in the output bytes.
      expect(String.fromCharCodes(output), contains('Hello World'),
          reason: 'builder text must appear in output PDF bytes');
    });

    test('create → setMetadata → save → verify metadata in output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      await builder.setTitle('Custom Builder Title');
      await builder.setAuthor('Builder Author');
      final page = await builder.addPage(width: 612, height: 792);
      await page.text('content');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();

      // Re-open to verify valid PDF.
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);

      // Metadata strings should appear in the output bytes.
      final asString = String.fromCharCodes(output);
      expect(asString, contains('Custom Builder Title'),
          reason: 'title must be in output');
      expect(asString, contains('Builder Author'),
          reason: 'author must be in output');
    });

    test('double dispose is safe', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      await builder.dispose();
      await builder.dispose();
    });

    test('imagesToPdf creates valid single-page PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.imagesToPdf([src(minimalPng)], sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      // Image data should increase size beyond a blank page.
      expect(output.length, greaterThan(minimalPdf.length),
          reason: 'image PDF should be larger than blank page');
    });

    test('multiple pages have correct count', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      for (var i = 0; i < 3; i++) {
        final page = await builder.addPage(width: 612, height: 792);
        await page.text('Page ${i + 1}');
        await page.done();
      }
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();

      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
    });

    test('addA4Page creates correct dimensions', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('A4');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();

      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      // A4 = 595.28 × 841.89 points
      expect(doc.pages[0].width, closeTo(595, 2));
      expect(doc.pages[0].height, closeTo(842, 2));
    });

    test('addLetterPage creates correct dimensions', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addLetterPage();
      await page.text('Letter');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();

      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      // Letter = 612 × 792 points
      expect(doc.pages[0].width, closeTo(612, 2));
      expect(doc.pages[0].height, closeTo(792, 2));
    });
  });
}
