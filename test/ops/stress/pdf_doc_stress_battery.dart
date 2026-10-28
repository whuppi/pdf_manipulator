// CHARTER — scale only: read ops must survive a 1000-page foreign
// document within their time budgets. Content correctness lives in
// the core doc battery (no duplicate claims here).

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/perf.dart';

void registerDocStressTests(Pdf Function() createPdf) {
  group('stress doc', tags: 'stress', () {
    final largePdf = fThousandPage;

    // Observed op/calibration: 3.13, 3.8, 6.14, 5.38.
    stressTest('open 1000-page PDF', createPdf, ratio: 19, () async {
      final doc = await createPdf().open(src(largePdf));
      expect(doc.pageCount, fThousandPageTruth.pages);
      await doc.dispose();
    });

    // Observed op/calibration: 36, 37.69, 34.07, 52.64.
    stressTest(
      'extract text from 1000-page PDF',
      createPdf,
      ratio: 158,
      () async {
        final doc = await createPdf().open(src(largePdf));
        final text = await doc.extract(pages: const PdfPages.all());
        expect(text.length, greaterThan(1000));
        expect(text, contains('Lorem ipsum'));
        await doc.dispose();
      },
    );

    // Observed op/calibration: 25.59, 26.69, 25.32, 44.16.
    stressTest(
      'extract single page from middle',
      createPdf,
      ratio: 133,
      () async {
        final doc = await createPdf().open(src(largePdf));
        final text = await doc.extract(pages: const PdfPages.single(100));
        expect(
          text,
          contains('${fThousandPageTruth.markerPrefix}100'),
          reason:
              'every page carries its own marker — the wrong '
              'page (or garbage) cannot fake this',
        );
        expect(
          text,
          isNot(contains('${fThousandPageTruth.markerPrefix}99\n')),
          reason: 'a neighboring page bleeding in is a paging bug',
        );
        expect(text, contains('Lorem ipsum'));
        await doc.dispose();
      },
    );

    // Observed op/calibration: 46.48, 41.39, 44.39, 47.51.
    stressTest('search across 1000-page PDF', createPdf, ratio: 143, () async {
      final doc = await createPdf().open(src(largePdf));
      final results = await doc.search(
        query: 'Lorem',
        pages: const PdfPages.all(),
      );
      expect(results.length, greaterThanOrEqualTo(1000));
      for (final r in results) {
        expect(
          r.text,
          contains('Lorem'),
          reason: 'a hit that does not contain the query is not a hit',
        );
        expect(r.page, inInclusiveRange(0, 999));
      }
      await doc.dispose();
    });

    // Observed op/calibration: 14.43, 10.03, 8.32, 14.46.
    stressTest('render first page', createPdf, ratio: 44, () async {
      final doc = await createPdf().open(src(largePdf));
      int count = 0;
      await for (final page in doc.render(pages: const PdfPages.single(0))) {
        expect(page.width, greaterThan(0));
        count++;
      }
      expect(count, 1);
      await doc.dispose();
    });

    // Observed op/calibration: 11.74, 11.87, 12.13, 13.03.
    stressTest('render pages 100-109', createPdf, ratio: 40, () async {
      final doc = await createPdf().open(src(largePdf));
      int count = 0;
      await for (final _ in doc.render(
        pages: PdfPages.list(List.generate(10, (i) => 100 + i)),
      )) {
        count++;
      }
      expect(count, 10);
      await doc.dispose();
    });

    // Observed op/calibration: 59.82, 61.87, 60.14, 64.18.
    stressTest('convertTo DOCX', createPdf, ratio: 193, () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(src(largePdf), sink, format: PdfDocumentFormat.docx);
      final docx = sink.takeBytes();
      expect(
        docx.sublist(0, 2),
        [0x50, 0x4B],
        reason:
            'DOCX is a ZIP — wrong magic means the converter '
            'wrote something else',
      );
    });
  });
}
