// Doc stress — read-only queries on 1000-page PDF.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';
import '../../helpers/timeouts.dart';

void registerDocStressTests(Pdf Function() createPdf) {
  group('stress doc', tags: 'stress', () {
    late Uint8List largePdf;

    test('build 1000-page PDF', () async {
      largePdf = await buildLargePdf(createPdf, pageCount: 1000);
      expect(largePdf.length, greaterThan(0));
    }, timeout: t(3));

    test('open 1000-page PDF', () async {
      final doc = await createPdf().open(src(largePdf));
      expect(doc.pageCount, 1000);
    }, timeout: t(3));

    test('extract text from 1000-page PDF', () async {
      final doc = await createPdf().open(src(largePdf));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text.length, greaterThan(1000));
      expect(text, contains('Lorem ipsum'));
      await doc.dispose();
    }, timeout: t(3));

    test('extract single page from middle', () async {
      final doc = await createPdf().open(src(largePdf));
      final text = await doc.extract(pages: const PdfPages.single(100));
      expect(text.length, greaterThan(0));
      await doc.dispose();
    }, timeout: t(3));

    test('search across 1000-page PDF', () async {
      final doc = await createPdf().open(src(largePdf));
      final results = await doc.search(query: 'Lorem', pages: const PdfPages.all());
      expect(results.length, greaterThanOrEqualTo(1000));
      await doc.dispose();
    }, timeout: t(3));

    test('render first page', () async {
      final doc = await createPdf().open(src(largePdf));
      int count = 0;
      await for (final page in doc.render(pages: const PdfPages.single(0))) {
        expect(page.width, greaterThan(0));
        count++;
      }
      expect(count, 1);
      await doc.dispose();
    }, timeout: t(3));

    test('render pages 100-109', () async {
      final doc = await createPdf().open(src(largePdf));
      int count = 0;
      await for (final _ in doc.render(
          pages: PdfPages.list(List.generate(10, (i) => 100 + i)))) {
        count++;
      }
      expect(count, 10);
      await doc.dispose();
    }, timeout: t(3));

    test('convertTo DOCX', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(largePdf), sink, format: PdfDocumentFormat.docx);
      expect(sink.takeBytes().length, greaterThan(0));
    }, timeout: t(3));
  });
}
