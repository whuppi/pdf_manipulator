// Lifecycle — dispose, double dispose, ops after dispose, sequential reuse.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerLifecycleTests(Pdf Function() createPdf) {
  group('lifecycle', () {
    test('dispose completes without error', () async {
      final pdf = createPdf();
      await pdf.open(src(minimalPdf));
      await pdf.dispose();
    });

    test('double dispose is safe', () async {
      final pdf = createPdf();
      await pdf.dispose();
      await pdf.dispose();
    });

    test('operations after dispose throw', () async {
      final pdf = createPdf();
      await pdf.open(src(minimalPdf));
      await pdf.dispose();
      expect(
        () => pdf.open(src(minimalPdf)),
        throwsStateError,
      );
    });

    test('dispose returns quickly', () async {
      final pdf = createPdf();
      await pdf.open(src(minimalPdf));
      final sw = Stopwatch()..start();
      await pdf.dispose();
      sw.stop();
      expect(sw.elapsedMilliseconds, lessThan(2000));
    });

    test('multiple sequential operations on same instance', () async {
      final pdf = createPdf();
      final doc1 = await pdf.open(src(minimalPdf));
      expect(doc1.pageCount, 1);

      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], sink);
      final doc2 = await pdf.open(src(sink.takeBytes()));
      expect(doc2.pageCount, 2);

      await pdf.dispose();
    });
  });
}
