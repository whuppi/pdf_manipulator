// CHARTER — scale only: the instance survives rapid open/dispose
// churn, parallel load across instances, and dispose-under-load.
// Dispose SEMANTICS live in the core instance battery.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/perf.dart';

void registerInstanceStressTests(Pdf Function() createPdf) {
  group('instance stress', tags: 'stress', () {
    // Observed op/calibration: 3.68, 4.01, 3.88.
    stressTest(
      '100x rapid open/dispose cycles',
      createPdf,
      ratio: 13,
      ownsPdf: true,
      () async {
        final pdf = createPdf();
        for (var i = 0; i < 100; i++) {
          final doc = await pdf.open(src(minimalPdf));
          expect(doc.pageCount, 1);
          await doc.dispose();
        }
        await pdf.dispose();
      },
    );

    // Observed op/calibration: 3.66, 4.31, 3.97.
    stressTest(
      '100x rapid editor open/dispose cycles',
      createPdf,
      ratio: 13,
      ownsPdf: true,
      () async {
        final pdf = createPdf();
        for (var i = 0; i < 100; i++) {
          final ed = await pdf.edit(src(minimalPdf));
          expect(await ed.pageCount, 1);
          await ed.dispose();
        }
        await pdf.dispose();
      },
    );

    // Observed op/calibration: 1.51, 1.52, 1.64.
    stressTest(
      '50x parallel open across 5 instances',
      createPdf,
      ratio: 5,
      ownsPdf: true,
      () async {
        final instances = List.generate(5, (_) => createPdf());
        for (var round = 0; round < 10; round++) {
          final docs = await Future.wait(
            instances.map((pdf) => pdf.open(src(minimalPdf))),
          );
          for (final doc in docs) {
            expect(doc.pageCount, 1);
            await doc.dispose();
          }
        }
        for (final pdf in instances) {
          await pdf.dispose();
        }
      },
    );

    // Observed op/calibration: 3.84, 3.94, 3.9.
    stressTest(
      'interleaved doc+editor+sugar on same instance',
      createPdf,
      ratio: 12,
      ownsPdf: true,
      () async {
        final pdf = createPdf();
        for (var i = 0; i < 20; i++) {
          final doc = await pdf.open(src(minimalPdf));
          expect(doc.pageCount, 1);
          await doc.dispose();

          final ed = await pdf.edit(src(minimalPdf));
          await ed.setTitle('round $i');
          final sink = TestSink();
          await ed.save(sink);
          await ed.dispose();

          final sink2 = TestSink();
          await pdf.merge([src(minimalPdf), src(minimalPdf)], sink2);
        }
        await pdf.dispose();
      },
    );

    // Observed op/calibration: 0.36, 0.35, 0.35.
    stressTest(
      'dispose under load — open 10 docs then kill',
      createPdf,
      ratio: 2,
      ownsPdf: true,
      () async {
        final pdf = createPdf();
        final docs = <PdfDoc>[];
        for (var i = 0; i < 10; i++) {
          docs.add(await pdf.open(src(minimalPdf)));
        }
        await pdf.dispose();
      },
    );
  });
}
