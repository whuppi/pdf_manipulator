// CHARTER — scale only: the instance survives rapid open/dispose
// churn, parallel load across instances, and dispose-under-load.
// Dispose SEMANTICS live in the core instance battery.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerInstanceStressTests(Pdf Function() createPdf) {
  group('instance stress', tags: 'stress', () {
    test('100x rapid open/dispose cycles', () async {
      final pdf = createPdf();
      for (var i = 0; i < 100; i++) {
        final doc = await pdf.open(src(minimalPdf));
        expect(doc.pageCount, 1);
        await doc.dispose();
      }
      await pdf.dispose();
    }, timeout: t(1));

    test('100x rapid editor open/dispose cycles', () async {
      final pdf = createPdf();
      for (var i = 0; i < 100; i++) {
        final ed = await pdf.edit(src(minimalPdf));
        expect(await ed.pageCount, 1);
        await ed.dispose();
      }
      await pdf.dispose();
    }, timeout: t(1));

    test('50x parallel open across 5 instances', () async {
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
    }, timeout: t(1));

    test('interleaved doc+editor+sugar on same instance', () async {
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
    }, timeout: t(1));

    test('dispose under load — open 10 docs then kill', () async {
      final pdf = createPdf();
      final docs = <PdfDoc>[];
      for (var i = 0; i < 10; i++) {
        docs.add(await pdf.open(src(minimalPdf)));
      }
      final sw = Stopwatch()..start();
      await pdf.dispose();
      sw.stop();
      expect(sw.elapsedMilliseconds, lessThan(2000));
    }, timeout: t(1));
  });
}
