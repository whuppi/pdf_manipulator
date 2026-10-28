// CHARTER — scale only: one-shot sugar ops must conserve every page
// across 100/1000-page splits, merges, and deletes within their time
// budgets. Content correctness lives in the core sugar battery.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/perf.dart';

void registerSugarStressTests(Pdf Function() createPdf) {
  group('stress sugar', tags: 'stress', () {
    final largePdf = fThousandPage;

    // Observed op/calibration: 79.72, 76.7, 75.67, 64.74.
    stressTest('split every 100 pages', createPdf, ratio: 240, () async {
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
    });

    // Observed op/calibration: 33.2, 29.9, 33.64, 34.3.
    stressTest('splitBySize generous limit', createPdf, ratio: 103, () async {
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
    });

    // Observed op/calibration: 194.71, 210.7, 193.14, 196.49.
    stressTest('splitBySize tight limit', createPdf, ratio: 633, () async {
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
    });

    // Observed op/calibration: 114.16, 123.5, 119.84, 125.82.
    stressTest(
      'splitBySize smaller than single page',
      createPdf,
      ratio: 378,
      () async {
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
      },
    );

    // Observed op/calibration: 21.71, 23.43, 22.63, 25.18.
    stressTest(
      'splitBySize equal to full PDF → 1 chunk',
      createPdf,
      ratio: 76,
      () async {
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
      },
    );

    // Observed op/calibration: 38.35, 37.65, 39.52, 35.28.
    stressTest(
      'splitBySize returns chunk byte sizes',
      createPdf,
      ratio: 119,
      () async {
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
      },
    );

    // Observed op/calibration: 9.57, 9.48, 9.44, 9.48.
    stressTest('delete 990 pages', createPdf, ratio: 29, () async {
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
    });

    // Observed op/calibration: 134.14, 128.12, 107.91, 125.97.
    stressTest(
      'merge two 1000-page PDFs → 2000 pages',
      createPdf,
      ratio: 403,
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.merge([src(largePdf), src(largePdf)], sink);
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(doc.pageCount, 2000);
        await doc.dispose();
      },
    );

    // Observed op/calibration: 2.16, 2.11, 2.37, 1.87.
    stressTest(
      'splitBySize on varied-size pages',
      createPdf,
      ratio: 8,
      () async {
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
      },
    );
  });
}
