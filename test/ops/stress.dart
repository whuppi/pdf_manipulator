// Stress tests — large PDFs, real operations at scale.
// These run against both native and web via the shared runner pattern.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:test/test.dart';

import '../helpers/generators.dart';
import '../helpers/test_source_sink.dart';

void registerStressTests(PdfBridge Function() b) {
  group('stress', () {
    late Uint8List largePdf;

    setUpAll(() async {
      largePdf = await buildLargePdf(b, pageCount: 1000);
    });

    // ── Basic operations on 200-page PDF ──

    test('open 1000-page PDF', () async {
      final doc = await b().open(src(largePdf));
      expect(doc.pageCount, 1000);
    });

    test('extract text from 1000-page PDF', () async {
      final text = await b().extract(src(largePdf), pages: const PdfPages.all());
      expect(text.length, greaterThan(1000));
      expect(text, contains('Lorem ipsum'));
    });

    test('extract single page from middle', () async {
      final text = await b().extract(src(largePdf), pages: const PdfPages.single(100));
      expect(text.length, greaterThan(0));
    });

    // ── Split by page count ──

    test('split 1000-page PDF every 100 pages', () async {
      final sinks = <TestSink>[];
      await b().split(src(largePdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, every: 100);
      expect(sinks.length, 10);
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await b().open(src(bytes));
        expect(doc.pageCount, 100);
      }
    });

    // ── splitBySize — tested on 100-page subset for speed ──
    // extractPages is expensive per call; splitBySize probes via extractPages.
    // 100 pages keeps each test under 30 seconds while still testing the algorithm.

    test('splitBySize with generous limit → fewer chunks', () async {
      final smallPdf = await buildLargePdf(b, pageCount: 100);
      final sinks = <TestSink>[];
      final chunkSizes = await b().splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length ~/ 2);
      expect(chunkSizes.length, greaterThanOrEqualTo(2));
      expect(sinks.length, chunkSizes.length);
      var totalPages = 0;
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await b().open(src(bytes));
        totalPages += doc.pageCount;
      }
      expect(totalPages, 100);
    });

    test('splitBySize with tight limit → many chunks', () async {
      final smallPdf = await buildLargePdf(b, pageCount: 100);
      final sinks = <TestSink>[];
      final chunkSizes = await b().splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length ~/ 10);
      expect(chunkSizes.length, greaterThanOrEqualTo(5));
      expect(sinks.length, chunkSizes.length);
      for (final s in sinks) {
        expect(s.takeBytes().length, greaterThan(0));
      }
    });

    test('splitBySize with limit smaller than single page', () async {
      final smallPdf = await buildLargePdf(b, pageCount: 100);
      final sinks = <TestSink>[];
      final chunkSizes = await b().splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: 500);
      expect(chunkSizes.length, greaterThanOrEqualTo(50));
      expect(sinks.length, chunkSizes.length);
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await b().open(src(bytes));
        expect(doc.pageCount, greaterThanOrEqualTo(1));
      }
    });

    test('splitBySize with limit equal to full PDF → 1 chunk', () async {
      final smallPdf = await buildLargePdf(b, pageCount: 100);
      final sinks = <TestSink>[];
      final chunkSizes = await b().splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length * 2);
      expect(chunkSizes.length, 1);
      expect(sinks.length, 1);
      final doc = await b().open(src(sinks.first.takeBytes()));
      expect(doc.pageCount, 100);
    });

    test('splitBySize returns chunk byte sizes', () async {
      final smallPdf = await buildLargePdf(b, pageCount: 20);
      final sinks = <TestSink>[];
      final chunkSizes = await b().splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length ~/ 3);
      expect(chunkSizes.length, greaterThanOrEqualTo(2));
      for (var i = 0; i < chunkSizes.length; i++) {
        expect(chunkSizes[i], greaterThan(0));
        expect(chunkSizes[i], sinks[i].takeBytes().length);
      }
    });

    // ── Structural operations at scale ──

    test('extract first 10 pages from 1000-page PDF', () async {
      final sink = TestSink();
      await b().extractPages(src(largePdf), sink,
          pages: List.generate(10, (i) => i));
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 10);
    });

    test('delete 990 pages from 1000-page PDF', () async {
      final sink = TestSink();
      await b().deletePages(src(largePdf), sink,
          pages: List.generate(990, (i) => i));
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 10);
    });

    test('merge two 1000-page PDFs → 2000 pages', () async {
      final sink = TestSink();
      await b().merge([src(largePdf), src(largePdf)], sink);
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 2000);
    });

    // ── Content operations at scale ──

    test('compress 1000-page PDF', () async {
      final sink = TestSink();
      await b().compress(src(largePdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('watermark 1000 pages', () async {
      final sink = TestSink();
      await b().watermark(src(largePdf), sink, text: 'CONFIDENTIAL');
      expect(sink.takeBytes().length, greaterThan(largePdf.length));
    });

    test('encrypt then decrypt 1000-page PDF', () async {
      final encSink = TestSink();
      await b().encrypt(src(largePdf), encSink,
          encryption: const PdfEncryptionConfig(
              ownerPassword: 'owner', userPassword: 'user'));
      final decSink = TestSink();
      await b().decrypt(src(encSink.takeBytes()), decSink, password: 'owner');
      final doc = await b().open(src(decSink.takeBytes()));
      expect(doc.pageCount, 1000);
    });

    test('search across 1000-page PDF', () async {
      final results = await b().search(src(largePdf),
          query: 'Lorem', pages: const PdfPages.all());
      expect(results.length, greaterThanOrEqualTo(1000));
    });

    // ── Rendering at scale ──

    test('render first page of 1000-page PDF', () async {
      int count = 0;
      await for (final page in b().render(src(largePdf),
          pages: const PdfPages.single(0))) {
        expect(page.width, greaterThan(0));
        count++;
      }
      expect(count, 1);
    });

    test('render pages 100-109', () async {
      int count = 0;
      await for (final _ in b().render(src(largePdf),
          pages: PdfPages.list(List.generate(10, (i) => 100 + i)))) {
        count++;
      }
      expect(count, 10);
    });

    // ── Editor at scale ──

    test('editor: watermark first 10 pages of 1000-page PDF', () async {
      final editor = await b().openEditor(src(largePdf));
      for (var i = 0; i < 10; i++) {
        await editor.addWatermark(i, 'EDITED');
      }
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 1000);
    });

    // ── Varied-size PDF for splitBySize edge cases ──

    test('splitBySize on varied-size pages', () async {
      final variedPdf = await buildVariedSizePdf(b, pageCount: 50);
      final sinks = <TestSink>[];
      final chunkSizes = await b().splitBySize(src(variedPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: variedPdf.length ~/ 5);
      expect(chunkSizes.length, greaterThan(1));
      var totalPages = 0;
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await b().open(src(bytes));
        totalPages += doc.pageCount;
      }
      expect(totalPages, 50);
    });

    // ── Image PDF ──

    test('build and open image PDF', () async {
      final imagePdf = await buildImagePdf(b, pageCount: 10);
      final doc = await b().open(src(imagePdf));
      expect(doc.pageCount, 10);
    });

    // ── Conversion at scale ──

    test('convertTo DOCX from 1000-page PDF', () async {
      final sink = TestSink();
      await b().convertTo(src(largePdf), sink, format: PdfDocumentFormat.docx);
      expect(sink.takeBytes().length, greaterThan(0));
    });

    // ── Forms ──

    test('flattenForms on form PDF', () async {
      final formPdf = await buildFormPdf(b);
      final sink = TestSink();
      await b().flattenForms(src(formPdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    });
  });
}
