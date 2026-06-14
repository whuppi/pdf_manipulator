// CHARTER — scale only: one-shot sugar ops must conserve every page
// across 100/1000-page splits, merges, and deletes within their time
// budgets. Content correctness lives in the core sugar battery.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerSugarStressTests(Pdf Function() createPdf) {
  group('stress sugar', tags: 'stress', () {
    final largePdf = fThousandPage;

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
        await doc.dispose();
      }
    }, timeout: t(1));

    test('splitBySize generous limit', () async {
      final pdf = createPdf();
      final smallPdf = fHundredPage;
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
        await doc.dispose();
      }
      expect(totalPages, fHundredPageTruth.pages);
    }, timeout: t(1));

    test('splitBySize tight limit', () async {
      final pdf = createPdf();
      final smallPdf = fHundredPage;
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length ~/ 10);
      expect(chunkSizes.length, greaterThanOrEqualTo(5));
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        totalPages += doc.pageCount;
        await doc.dispose();
      }
      expect(
        totalPages,
        fHundredPageTruth.pages,
        reason:
            'a split that loses or duplicates pages is data '
            'corruption, however valid each chunk looks',
      );
    }, timeout: t(3));

    test('splitBySize smaller than single page', () async {
      final pdf = createPdf();
      final smallPdf = fHundredPage;
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: 500);
      expect(chunkSizes.length, greaterThanOrEqualTo(50));
      var totalPages = 0;
      for (final s in sinks) {
        final bytes = s.takeBytes();
        expect(bytes.length, greaterThan(0));
        final doc = await pdf.open(src(bytes));
        totalPages += doc.pageCount;
        expect(doc.pageCount, greaterThanOrEqualTo(1));
        await doc.dispose();
      }
      expect(totalPages, fHundredPageTruth.pages);
    }, timeout: t(2));

    test('splitBySize equal to full PDF → 1 chunk', () async {
      final pdf = createPdf();
      final smallPdf = fHundredPage;
      final sinks = <TestSink>[];
      await pdf.splitBySize(src(smallPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: smallPdf.length * 2);
      expect(sinks.length, 1);
      final doc = await pdf.open(src(sinks.first.takeBytes()));
      expect(doc.pageCount, 100);
      await doc.dispose();
    }, timeout: t(1));

    test('splitBySize returns chunk byte sizes', () async {
      final pdf = createPdf();
      final smallPdf = fHundredPage;
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
    }, timeout: t(1));

    test('delete 990 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.deletePages(
        src(largePdf),
        sink,
        pages: List.generate(990, (i) => i),
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 10);
      await doc.dispose();
    }, timeout: t(1));

    test('merge two 1000-page PDFs → 2000 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(largePdf), src(largePdf)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2000);
      await doc.dispose();
    }, timeout: t(2));

    test('splitBySize on varied-size pages', () async {
      final pdf = createPdf();
      final variedPdf = fMultisize;
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
        await doc.dispose();
      }
      expect(totalPages, fMultisizeTruth.pages);
    }, timeout: t(1));
  });
}
