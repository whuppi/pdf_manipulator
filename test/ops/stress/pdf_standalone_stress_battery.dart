// CHARTER — scale only: standalone ops must handle a 1000-page
// foreign document within their time budgets. Correctness lives in
// the core standalone battery.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/perf.dart';

void registerStandaloneStressTests(Pdf Function() createPdf) {
  group('stress standalone', tags: 'stress', () {
    final largePdf = fThousandPage;

    // Observed op/calibration: 5.48, 5.17, 5.17, 4.9.
    stressTest(
      'extractPages first 10 from 1000-page',
      createPdf,
      ratio: 17,
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.extractPages(
          src(largePdf),
          sink,
          pages: List.generate(10, (i) => i),
        );
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(doc.pageCount, 10);
        await doc.dispose();
      },
    );
  });
}
