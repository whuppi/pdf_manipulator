// Lifecycle — dispose, double dispose, ops after dispose, sequential reuse.

import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerLifecycleTests(PdfBridge Function() createBridge) {
  group('lifecycle', () {
    test('dispose completes without error', () async {
      final b = createBridge();
      await b.open(src(minimalPdf));
      await b.dispose();
    });

    test('double dispose is safe', () async {
      final b = createBridge();
      await b.dispose();
      await b.dispose();
    });

    test('operations after dispose throw', () async {
      final b = createBridge();
      await b.open(src(minimalPdf));
      await b.dispose();
      expect(
        () => b.open(src(minimalPdf)),
        throwsStateError,
      );
    });

    test('dispose returns quickly', () async {
      final b = createBridge();
      await b.open(src(minimalPdf));
      final sw = Stopwatch()..start();
      await b.dispose();
      sw.stop();
      expect(sw.elapsedMilliseconds, lessThan(2000));
    });

    test('multiple sequential operations on same bridge', () async {
      final b = createBridge();
      final doc1 = await b.open(src(minimalPdf));
      expect(doc1.pageCount, 1);

      final sink = TestSink();
      await b.merge([src(minimalPdf), src(minimalPdf)], sink);
      final doc2 = await b.open(src(sink.takeBytes()));
      expect(doc2.pageCount, 2);

      await b.dispose();
    });
  });
}
