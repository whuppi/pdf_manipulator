// Lifecycle — dispose, double dispose, error recovery, garbage input.
// Cross-cutting behavior that applies to all handle types.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerLifecycleTests(Pdf Function() createPdf) {
  group('lifecycle', () {
    // ── Error handling ──

    test('open garbage bytes throws the typed engine error', () async {
      final pdf = createPdf();
      await expectLater(
        pdf.open(TestSource(Uint8List.fromList([1, 2, 3, 4]))),
        throwsA(isA<PdfEngineError>()),
        reason:
            'engine failures are PdfError — never a bare '
            'StateError, which is reserved for use-after-dispose',
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('open empty bytes throws the typed engine error', () async {
      final pdf = createPdf();
      await expectLater(
        pdf.open(TestSource(Uint8List(0))),
        throwsA(isA<PdfEngineError>()),
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('recover after error — next op works', () async {
      final pdf = createPdf();
      try {
        await pdf.open(TestSource(Uint8List.fromList([0xFF, 0xFE])));
      } catch (_) {}

      final doc = await pdf.open(
        TestSource(
          Uint8List.fromList(
            '%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n'
                    '2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n'
                    '3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]>>endobj\n'
                    'xref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n'
                    '0000000058 00000 n \n0000000115 00000 n \n'
                    'trailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF'
                .codeUnits,
          ),
        ),
      );
      expect(doc.pageCount, 1);
      await doc.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    // ── Instance dispose ──

    test('instance dispose actually disposes — later ops refuse', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await pdf.dispose();
      expect(
        () => pdf.open(src(minimalPdf)),
        throwsStateError,
        reason:
            'dispose must leave the instance provably dead, '
            'not just return without error',
      );
    }, timeout: t(1));

    test('instance double dispose is safe and stays disposed', () async {
      final pdf = createPdf();
      await pdf.dispose();
      await pdf.dispose();
      expect(() => pdf.open(src(minimalPdf)), throwsStateError);
    }, timeout: t(1));

    test('operations after instance dispose throw', () async {
      final pdf = createPdf();
      await pdf.dispose();
      expect(() => pdf.open(src(minimalPdf)), throwsStateError);
    }, timeout: t(1));

    test('instance dispose returns quickly', () async {
      final pdf = createPdf();
      await pdf.open(src(minimalPdf));
      final sw = Stopwatch()..start();
      await pdf.dispose();
      sw.stop();
      expect(sw.elapsedMilliseconds, lessThan(2000));
    }, timeout: t(1));

    // ── Doc handle dispose ──

    test('doc dispose kills the doc, not the instance', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await doc.dispose();
      expect(
        () => doc.extract(pages: const PdfPages.all()),
        throwsStateError,
        reason: 'a disposed doc refuses ops',
      );
      final doc2 = await pdf.open(src(minimalPdf));
      expect(doc2.pageCount, 1, reason: 'the instance survives a doc dispose');
      await doc2.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('doc double dispose is safe and stays disposed', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      await doc.dispose();
      await doc.dispose();
      expect(() => doc.extract(pages: const PdfPages.all()), throwsStateError);
      await pdf.dispose();
    }, timeout: t(1));
  });
}
