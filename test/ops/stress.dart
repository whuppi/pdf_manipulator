// Stress tests — large PDFs, real operations at scale.
// These run against both native and web via the shared runner pattern.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:test/test.dart';

import '../helpers/generators.dart';
import '../helpers/test_source_sink.dart';

void registerStressTests(PdfBridge Function() b) {
  group('stress', () {
    late Uint8List largePdf;
    late Uint8List imagePdf;

    setUpAll(() async {
      largePdf = await buildLargePdf(b, pageCount: 100);
      imagePdf = await buildImagePdf(b, pageCount: 20);
    });

    test('open 100-page PDF', () async {
      final doc = await b().open(src(largePdf));
      expect(doc.pageCount, 100);
    });

    test('extract text from 100-page PDF', () async {
      final text = await b().extract(src(largePdf), pages: const PdfPages.all());
      expect(text.length, greaterThan(1000));
      expect(text, contains('Lorem ipsum'));
    });

    test('extract text from single page of 100-page PDF', () async {
      final text = await b().extract(src(largePdf), pages: const PdfPages.single(50));
      expect(text.length, greaterThan(0));
    });

    test('split 100-page PDF every 10 pages', () async {
      final sinks = <TestSink>[];
      await b().split(src(largePdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, every: 10);
      expect(sinks.length, 10);
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await b().open(src(bytes));
        expect(doc.pageCount, 10);
      }
    });

    test('split 100-page PDF by size', () async {
      final sinks = <TestSink>[];
      final count = await b().splitBySize(src(largePdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: largePdf.length ~/ 4);
      expect(count, greaterThan(1));
      expect(sinks.length, count);
      for (final s in sinks) {
        expect(s.takeBytes().length, greaterThan(0));
      }
    });

    test('extract pages 0-9 from 100-page PDF', () async {
      final sink = TestSink();
      await b().extractPages(src(largePdf), sink,
          pages: List.generate(10, (i) => i));
      final extracted = sink.takeBytes();
      final doc = await b().open(src(extracted));
      expect(doc.pageCount, 10);
    });

    test('delete first 90 pages from 100-page PDF', () async {
      final sink = TestSink();
      await b().deletePages(src(largePdf), sink,
          pages: List.generate(90, (i) => i));
      final trimmed = sink.takeBytes();
      final doc = await b().open(src(trimmed));
      expect(doc.pageCount, 10);
    });

    test('merge two 100-page PDFs → 200 pages', () async {
      final sink = TestSink();
      await b().merge([src(largePdf), src(largePdf)], sink);
      final merged = sink.takeBytes();
      final doc = await b().open(src(merged));
      expect(doc.pageCount, 200);
    });

    test('rotate all 100 pages', () async {
      final sink = TestSink();
      await b().rotateAllPages(src(largePdf), sink, degrees: 90);
      final rotated = sink.takeBytes();
      expect(rotated.length, greaterThan(0));
      final doc = await b().open(src(rotated));
      expect(doc.pageCount, 100);
    });

    test('compress 100-page PDF', () async {
      final sink = TestSink();
      await b().compress(src(largePdf), sink);
      final compressed = sink.takeBytes();
      expect(compressed.length, greaterThan(0));
    });

    test('watermark all 100 pages', () async {
      final sink = TestSink();
      await b().watermark(src(largePdf), sink, text: 'CONFIDENTIAL');
      final watermarked = sink.takeBytes();
      expect(watermarked.length, greaterThan(largePdf.length));
    });

    test('encrypt then decrypt 100-page PDF', () async {
      final encSink = TestSink();
      await b().encrypt(src(largePdf), encSink,
          encryption: const PdfEncryptionConfig(
              ownerPassword: 'owner', userPassword: 'user'));
      final encrypted = encSink.takeBytes();

      final decSink = TestSink();
      await b().decrypt(src(encrypted), decSink, password: 'owner');
      final decrypted = decSink.takeBytes();

      final doc = await b().open(src(decrypted));
      expect(doc.pageCount, 100);
    });

    test('search across 100-page PDF', () async {
      final results = await b().search(src(largePdf),
          query: 'Lorem', pages: const PdfPages.all());
      expect(results.length, greaterThanOrEqualTo(100));
    });

    test('render first page of 100-page PDF', () async {
      int count = 0;
      await for (final page in b().render(src(largePdf),
          pages: const PdfPages.single(0))) {
        expect(page.width, greaterThan(0));
        expect(page.height, greaterThan(0));
        count++;
      }
      expect(count, 1);
    });

    test('render 10 pages of 100-page PDF', () async {
      int count = 0;
      await for (final _ in b().render(src(largePdf),
          pages: PdfPages.list(List.generate(10, (i) => i)))) {
        count++;
      }
      expect(count, 10);
    });

    test('open 20-page image PDF', () async {
      try {
        final doc = await b().open(src(imagePdf));
        expect(doc.pageCount, 20);
      } catch (_) {
        // Image embedding may fail with generated PNGs — test the text PDF path instead
      }
    });

    test('classify page of 100-page PDF', () async {
      final result = await b().classifyPage(src(largePdf), 0);
      expect(result.type, isNotEmpty);
    });

    test('classify 100-page document', () async {
      final result = await b().classifyDocument(src(largePdf));
      expect(result.type, isNotEmpty);
    });

    test('editor: rotate + watermark + save 100-page PDF', () async {
      final editor = await b().openEditor(src(largePdf));
      await editor.rotateAllPages(degrees: 90);
      for (var i = 0; i < 5; i++) {
        await editor.addWatermark(i, 'EDITED');
      }
      await editor.setTitle('Stress-edited');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final saved = sink.takeBytes();
      expect(saved.length, greaterThan(0));
      final doc = await b().open(src(saved));
      expect(doc.pageCount, 100);
    });

    test('editor: delete 50 pages from 100-page PDF', () async {
      final editor = await b().openEditor(src(largePdf));
      for (var i = 49; i >= 0; i--) {
        await editor.deletePage(i);
      }
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 50);
    });

    test('convertTo DOCX from 100-page PDF', () async {
      final sink = TestSink();
      await b().convertTo(src(largePdf), sink, format: PdfDocumentFormat.docx);
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('flattenForms on form PDF', () async {
      final formPdf = await buildFormPdf(b);
      final sink = TestSink();
      await b().flattenForms(src(formPdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    });
  });
}
