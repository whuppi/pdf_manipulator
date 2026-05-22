import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

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

  group('Pdf.watermark', () {
    test('watermarked PDF is larger than original (annotation added)', () async {
      final sink = TestPdfSink();
      await pdf.watermark(sourceOf(minimalPdf), sink, text: 'DRAFT');
      final result = sink.takeBytes();
      expect(result.length, greaterThan(minimalPdf.length));
    });

    test('watermarked PDF preserves page count', () async {
      final sink = TestPdfSink();
      await pdf.watermark(sourceOf(minimalPdf), sink, text: 'CONFIDENTIAL');
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(1));
    });

    test('watermarked PDF preserves page dimensions', () async {
      final sink = TestPdfSink();
      await pdf.watermark(sourceOf(minimalPdf), sink, text: 'TEST');
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[0].height, closeTo(842, 1));
    });

    test('watermark with different font sizes both produce valid PDFs', () async {
      final sinkSmall = TestPdfSink();
      await pdf.watermark(sourceOf(minimalPdf), sinkSmall, text: 'X', fontSize: 12);
      final small = sinkSmall.takeBytes();
      final sinkLarge = TestPdfSink();
      await pdf.watermark(sourceOf(minimalPdf), sinkLarge, text: 'X', fontSize: 96);
      final large = sinkLarge.takeBytes();
      final docSmall = await pdf.open(sourceOf(small));
      final docLarge = await pdf.open(sourceOf(large));
      expect(docSmall.pageCount, equals(1));
      expect(docLarge.pageCount, equals(1));
      expect(small.length, greaterThan(minimalPdf.length));
      expect(large.length, greaterThan(minimalPdf.length));
    });

    test('watermark on specific pages only — page 0 only on 2-page PDF', () async {
      final mergeSink = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();
      final sink0 = TestPdfSink();
      await pdf.watermark(sourceOf(twoPage), sink0, text: 'FIRST', pages: [0]);
      final onlyPage0 = sink0.takeBytes();
      final sinkAll = TestPdfSink();
      await pdf.watermark(sourceOf(twoPage), sinkAll, text: 'FIRST');
      final bothPages = sinkAll.takeBytes();
      expect(bothPages.length, greaterThan(onlyPage0.length));
    });

    test('watermark on multi-page PDF preserves all pages', () async {
      final mergeSink = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(minimalPdf), sourceOf(minimalPdf)], mergeSink);
      final threePage = mergeSink.takeBytes();
      final sink = TestPdfSink();
      await pdf.watermark(sourceOf(threePage), sink, text: 'ALL');
      final result = sink.takeBytes();
      final doc = await pdf.open(sourceOf(result));
      expect(doc.pageCount, equals(3));
      expect(result.length, greaterThan(threePage.length));
    });

    test('different text produces different output', () async {
      final sinkA = TestPdfSink();
      await pdf.watermark(sourceOf(minimalPdf), sinkA, text: 'AAA');
      final a = sinkA.takeBytes();
      final sinkB = TestPdfSink();
      await pdf.watermark(sourceOf(minimalPdf), sinkB, text: 'BBBBBBBBB');
      final b = sinkB.takeBytes();
      expect(b.length, greaterThan(a.length));
    });
  });

  group('PdfEditor.addWatermark', () {
    test('adds watermark — output is larger', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.addWatermark(0, 'EDITED');
      final sink = TestPdfSink();
      await editor.save(sink);
      final saved = sink.takeBytes();
      expect(saved.length, greaterThan(minimalPdf.length));
      editor.dispose();
    });

    test('watermark marks document as modified', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      expect(await editor.isModified, isFalse);
      await editor.addWatermark(0, 'MOD');
      expect(await editor.isModified, isTrue);
      editor.dispose();
    });

    test('watermark with custom color produces valid PDF', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.addWatermark(0, 'BLUE', r: 0.0, g: 0.0, b: 1.0);
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(1));
      expect(saved.length, greaterThan(minimalPdf.length));
    });
  });
}
