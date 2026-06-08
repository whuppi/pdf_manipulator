// Sugar stress — split, merge, delete, extract on 1000-page PDF.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';

void registerSugarStressTests(Pdf Function() createPdf) {
  group('stress sugar', tags: 'stress', () {
    late Uint8List largePdf;

    test('build 1000-page PDF', () async {
      largePdf = await buildLargePdf(createPdf, pageCount: 1000);
      expect(largePdf.length, greaterThan(0));
    }, timeout: Timeout(Duration(seconds: 20)));

    test('split every 100 pages', () async {
      final pdf = createPdf();
      final sinks = <TestSink>[];
      await pdf.split(src(largePdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, every: 100);
      expect(sinks.length, 10);
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await pdf.open(src(bytes));
        expect(doc.pageCount, 100);
      }
    }, timeout: Timeout(Duration(seconds: 20)));

    test('splitBySize generous limit', () async {
      final pdf = createPdf();
      final smallPdf = await buildLargePdf(createPdf, pageCount: 100);
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length ~/ 2);
      expect(chunkSizes.length, greaterThanOrEqualTo(2));
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        totalPages += doc.pageCount;
      }
      expect(totalPages, 100);
    }, timeout: Timeout(Duration(seconds: 20)));

    test('splitBySize tight limit', () async {
      final pdf = createPdf();
      final smallPdf = await buildLargePdf(createPdf, pageCount: 100);
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length ~/ 10);
      expect(chunkSizes.length, greaterThanOrEqualTo(5));
      for (final s in sinks) {
        expect(s.takeBytes().length, greaterThan(0));
      }
    }, timeout: Timeout(Duration(seconds: 20)));

    test('splitBySize smaller than single page', () async {
      final pdf = createPdf();
      final smallPdf = await buildLargePdf(createPdf, pageCount: 100);
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: 500);
      expect(chunkSizes.length, greaterThanOrEqualTo(50));
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await pdf.open(src(bytes));
        expect(doc.pageCount, greaterThanOrEqualTo(1));
      }
    }, timeout: Timeout(Duration(seconds: 20)));

    test('splitBySize equal to full PDF → 1 chunk', () async {
      final pdf = createPdf();
      final smallPdf = await buildLargePdf(createPdf, pageCount: 100);
      final sinks = <TestSink>[];
      await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length * 2);
      expect(sinks.length, 1);
      final doc = await pdf.open(src(sinks.first.takeBytes()));
      expect(doc.pageCount, 100);
    }, timeout: Timeout(Duration(seconds: 20)));

    test('splitBySize returns chunk byte sizes', () async {
      final pdf = createPdf();
      final smallPdf = await buildLargePdf(createPdf, pageCount: 20);
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length ~/ 3);
      expect(chunkSizes.length, greaterThanOrEqualTo(2));
      for (var i = 0; i < chunkSizes.length; i++) {
        expect(chunkSizes[i], greaterThan(0));
        expect(chunkSizes[i], sinks[i].takeBytes().length);
      }
    }, timeout: Timeout(Duration(seconds: 20)));

    test('extract first 10 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.extractPages(src(largePdf), sink,
          pages: List.generate(10, (i) => i));
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 10);
    }, timeout: Timeout(Duration(seconds: 20)));

    test('delete 990 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.deletePages(src(largePdf), sink,
          pages: List.generate(990, (i) => i));
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 10);
    }, timeout: Timeout(Duration(seconds: 20)));

    test('merge two 1000-page PDFs → 2000 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(largePdf), src(largePdf)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2000);
    }, timeout: Timeout(Duration(seconds: 20)));

    test('splitBySize on varied-size pages', () async {
      final pdf = createPdf();
      final variedPdf = await buildVariedSizePdf(createPdf, pageCount: 50);
      final sinks = <TestSink>[];
      await pdf.splitBySize(src(variedPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: variedPdf.length ~/ 5);
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        totalPages += doc.pageCount;
      }
      expect(totalPages, 50);
    }, timeout: Timeout(Duration(seconds: 20)));
  });
}
