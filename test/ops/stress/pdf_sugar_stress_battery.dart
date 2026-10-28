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

    // Observed op/calibration: 67.75, 69.58, 56.
    stressTest('split every 100 pages', createPdf, ratio: 209, () async {
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

    // Observed op/calibration: 28.07, 31.35, 25.69.
    stressTest('splitBySize generous limit', createPdf, ratio: 95, () async {
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

    // Observed op/calibration: 162.04, 170.74, 129.98.
    stressTest('splitBySize tight limit', createPdf, ratio: 513, () async {
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

    // Observed op/calibration: 105.74, 110.74, 68.51.
    stressTest(
      'splitBySize smaller than single page',
      createPdf,
      ratio: 333,
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

    // Observed op/calibration: 21.6, 21.63, 19.63.
    stressTest(
      'splitBySize equal to full PDF → 1 chunk',
      createPdf,
      ratio: 65,
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

    // Observed op/calibration: 34.18, 34.86, 32.17.
    stressTest(
      'splitBySize returns chunk byte sizes',
      createPdf,
      ratio: 105,
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

    // Observed op/calibration: 9.32, 3.93, 9.49.
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

    // Observed op/calibration: 107.31, 47.93, 109.03.
    stressTest(
      'merge two 1000-page PDFs → 2000 pages',
      createPdf,
      ratio: 328,
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.merge([src(largePdf), src(largePdf)], sink);
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(doc.pageCount, 2000);
        await doc.dispose();
      },
    );

    // Observed op/calibration: 1.93, 1.95, 1.69.
    stressTest(
      'splitBySize on varied-size pages',
      createPdf,
      ratio: 6,
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
