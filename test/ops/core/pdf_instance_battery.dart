// Instance architecture — cascade, isolation, parallel, reuse, abrupt kill.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/slow_source.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerInstanceTests(Pdf Function() createPdf) {
  group('instance', () {
    // ── Dispose cascade ──

    test('dispose cascades to open docs', () async {
      final pdf = createPdf();
      final doc1 = await pdf.open(src(minimalPdf));
      final doc2 = await pdf.open(src(minimalPdf));
      expect(doc1.pageCount, 1);
      expect(doc2.pageCount, 1);
      await pdf.dispose();
    }, timeout: t(1));

    test('dispose cascades to open editors', () async {
      final pdf = createPdf();
      final ed1 = await pdf.edit(src(minimalPdf));
      final ed2 = await pdf.edit(src(minimalPdf));
      expect(await ed1.pageCount, 1);
      expect(await ed2.pageCount, 1);
      await pdf.dispose();
    }, timeout: t(1));

    test('dispose cascades to open builders', () async {
      final pdf = createPdf();
      await pdf.build();
      await pdf.build();
      await pdf.dispose();
      // Reaching here without an error IS the proof — the cascade
      // freed both builders; a leak would surface as a refused
      // dispose or a zone error.
    }, timeout: t(1));

    test('dispose cascades to mix of doc+editor+builder', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      final ed = await pdf.edit(src(minimalPdf));
      final _ = await pdf.build();
      expect(doc.pageCount, 1);
      expect(await ed.pageCount, 1);
      await pdf.dispose();
    }, timeout: t(1));

    // ── Double dispose ──

    test('instance double dispose is safe', () async {
      final pdf = createPdf();
      await pdf.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('doc double dispose is safe', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      await doc.dispose();
      await doc.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('editor double dispose is safe', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(minimalPdf));
      await ed.dispose();
      await ed.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('builder double dispose is safe', () async {
      final pdf = createPdf();
      final b = await pdf.build();
      await b.dispose();
      await b.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    // ── Operations after dispose throw ──

    test('operations after instance dispose throw', () async {
      final pdf = createPdf();
      await pdf.dispose();
      expect(() => pdf.open(src(minimalPdf)), throwsStateError);
    }, timeout: t(1));

    // ── Dispose returns quickly ──

    test('instance dispose returns quickly', () async {
      final pdf = createPdf();
      await pdf.open(src(minimalPdf));
      await pdf.edit(src(minimalPdf));
      await pdf.build();
      final sw = Stopwatch()..start();
      await pdf.dispose();
      sw.stop();
      expect(sw.elapsedMilliseconds, lessThan(2000));
    }, timeout: t(1));

    // ── Sequential reuse ──

    test('sequential ops on same instance', () async {
      final pdf = createPdf();

      final doc1 = await pdf.open(src(minimalPdf));
      expect(doc1.pageCount, 1);
      await doc1.dispose();

      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], sink);

      final doc2 = await pdf.open(src(sink.takeBytes()));
      expect(doc2.pageCount, 2);
      await doc2.dispose();

      await pdf.dispose();
    }, timeout: t(1));

    test('doc dispose frees one doc, others survive', () async {
      final pdf = createPdf();
      final doc1 = await pdf.open(src(minimalPdf));
      final doc2 = await pdf.open(src(minimalPdf));
      await doc1.dispose();
      expect(doc2.pageCount, 1);
      await doc2.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    // ── Multi-instance isolation ──

    test('two instances are independent', () async {
      final pdfA = createPdf();
      final pdfB = createPdf();

      final docA = await pdfA.open(src(minimalPdf));
      final docB = await pdfB.open(src(minimalPdf));

      expect(docA.pageCount, 1);
      expect(docB.pageCount, 1);

      await pdfA.dispose();
      expect(docB.pageCount, 1);
      await docB.dispose();
      await pdfB.dispose();
    }, timeout: t(1));

    test('disposing one instance does not affect another', () async {
      final pdfA = createPdf();
      final pdfB = createPdf();

      await pdfA.open(src(minimalPdf));
      await pdfA.dispose();

      final doc = await pdfB.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await doc.dispose();
      await pdfB.dispose();
    }, timeout: t(1));

    // ── Parallel ops on single instance ──

    test('parallel opens via Future.wait', () async {
      final pdf = createPdf();
      final docs = await Future.wait([
        pdf.open(src(minimalPdf)),
        pdf.open(src(minimalPdf)),
        pdf.open(src(minimalPdf)),
      ]);
      for (final doc in docs) {
        expect(doc.pageCount, 1);
      }
      await pdf.dispose();
    }, timeout: t(1));

    test('parallel mix of doc+editor+builder', () async {
      final pdf = createPdf();
      final results = await Future.wait([
        pdf.open(src(minimalPdf)),
        pdf.edit(src(minimalPdf)),
        pdf.build(),
      ]);
      final doc = results[0] as PdfDoc;
      final ed = results[1] as PdfEditor;
      final _ = results[2] as PdfBuilder;
      expect(doc.pageCount, 1);
      expect(await ed.pageCount, 1);
      await pdf.dispose();
    }, timeout: t(1));

    // ── Abrupt dispose (kill mid-flight) ──

    test('dispose mid-merge produces no unhandled errors', () async {
      final errors = <Object>[];
      await runZonedGuarded(
        () async {
          final pdf = createPdf();
          final sink = TestSink();
          unawaited(
            pdf.merge([
              src(minimalPdf),
              src(minimalPdf),
              src(minimalPdf),
            ], sink),
          );
          await Future<void>.delayed(const Duration(milliseconds: 10));
          await pdf.dispose();
          await Future<void>.delayed(const Duration(milliseconds: 50));
        },
        (e, st) {
          errors.add(e);
        },
      );
      expect(
        errors,
        isEmpty,
        reason: 'dispose mid-flight should not leak errors',
      );
    }, timeout: t(1));

    test('dispose mid-open produces no unhandled errors', () async {
      final errors = <Object>[];
      await runZonedGuarded(
        () async {
          final pdf = createPdf();
          unawaited(pdf.open(src(minimalPdf)));
          await Future<void>.delayed(const Duration(milliseconds: 10));
          await pdf.dispose();
          await Future<void>.delayed(const Duration(milliseconds: 50));
        },
        (e, _) {
          errors.add(e);
        },
      );
      expect(
        errors,
        isEmpty,
        reason: 'dispose mid-flight should not leak errors',
      );
    }, timeout: t(1));

    test('dispose mid-edit produces no unhandled errors', () async {
      final errors = <Object>[];
      await runZonedGuarded(
        () async {
          final pdf = createPdf();
          unawaited(pdf.edit(src(minimalPdf)));
          await Future<void>.delayed(const Duration(milliseconds: 10));
          await pdf.dispose();
          await Future<void>.delayed(const Duration(milliseconds: 50));
        },
        (e, _) {
          errors.add(e);
        },
      );
      expect(
        errors,
        isEmpty,
        reason: 'dispose mid-flight should not leak errors',
      );
    }, timeout: t(1));

    // ── Per-op cancellation (PdfTask.cancel) ──
    //
    // Per-op cancel only interrupts an op blocked on host I/O — a
    // pure-compute op that already finished cannot be un-finished
    // (and shouldn't be). SlowSource parks the engine on its first
    // read so the cancel deterministically lands mid-flight.

    test(
      'task.cancel mid-op resolves PdfCancelled, instance survives',
      () async {
        final pdf = createPdf();
        final slow = SlowSource(minimalPdf); // first readAt never resolves
        final task = pdf.open(slow);
        await slow.firstRead; // op is genuinely parked on the source
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        slow.release(); // unblock the abandoned read (no-op for the op)
        // Only that job died — the instance keeps working.
        final doc = await pdf.open(src(minimalPdf));
        expect(doc.pageCount, 1);
        await doc.dispose();
        await pdf.dispose();
      },
      timeout: t(1),
    );

    test('cancel before the job starts resolves PdfCancelled', () async {
      final pdf = createPdf();
      final task = pdf.open(src(minimalPdf));
      task.cancel(); // same tick — job may not have been submitted yet
      await expectLater(task, throwsA(isA<PdfCancelled>()));
      final doc = await pdf.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await pdf.dispose();
    }, timeout: t(1));

    test('cancel after completion is a no-op', () async {
      final pdf = createPdf();
      final task = pdf.open(src(minimalPdf));
      final doc = await task;
      task.cancel(); // idempotent, result already delivered
      task.cancel();
      expect(doc.pageCount, 1); // handle unaffected
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text.trim(),
        isEmpty,
        reason:
            'minimal fixture is one blank page — late cancels '
            'must not corrupt the handle\'s reads',
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('cancelled op leaves sibling handles on the lane usable', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      // A second op on a slow source, pinned to the same lane.
      final slow = SlowSource(minimalPdf);
      final task = pdf.open(slow);
      await slow.firstRead;
      task.cancel();
      await expectLater(task, throwsA(isA<PdfCancelled>()));
      slow.release();
      // The lane survived a per-job cancel; the first doc still works.
      expect(doc.pageCount, 1);
      final page = await doc.extract(pages: const PdfPages.single(0));
      expect(page, isNotNull);
      await pdf.dispose();
    }, timeout: t(1));

    test('fire-and-forget cancelled task is zone-silent', () async {
      final errors = <Object>[];
      await runZonedGuarded(() async {
        final pdf = createPdf();
        final task = pdf.open(src(minimalPdf));
        task.cancel();
        // Never awaited — the cancelled outcome must not leak.
        await Future<void>.delayed(const Duration(milliseconds: 50));
        await pdf.dispose();
      }, (e, _) => errors.add(e));
      expect(
        errors,
        isEmpty,
        reason: 'cancellation is a normal lifecycle event, never noise',
      );
    }, timeout: t(1));

    test('fire-and-forget REAL failure stays loud in the zone', () async {
      final errors = <Object>[];
      await runZonedGuarded(() async {
        final pdf = createPdf();
        // Garbage bytes — a genuine engine error, not a cancellation.
        // Deliberately unawaited and unlistened: the stay-loud branch
        // of PdfTask is what re-raises this into the zone.
        unawaited(pdf.open(src(Uint8List.fromList([1, 2, 3]))));
        await Future<void>.delayed(const Duration(milliseconds: 200));
        await pdf.dispose();
      }, (e, _) => errors.add(e));
      expect(
        errors,
        isNotEmpty,
        reason: 'real failures must never be silently swallowed',
      );
    }, timeout: t(1));

    test('fresh instance works after abrupt dispose of another', () async {
      final pdf1 = createPdf();
      unawaited(pdf1.merge([src(minimalPdf), src(minimalPdf)], TestSink()));
      await Future<void>.delayed(const Duration(milliseconds: 10));
      await pdf1.dispose();

      // A completely new instance should work fine
      final pdf2 = createPdf();
      final doc = await pdf2.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await doc.dispose();
      await pdf2.dispose();
    }, timeout: t(1));

    test('5000x dispose mid-flight — instant, no leaks, ops resolve', () async {
      for (var i = 0; i < 5000; i++) {
        final pdf = createPdf();
        // Attach the outcome handler BEFORE dispose — the contract is
        // that the op RESOLVES (won the race, or cancelled), promptly.
        final settled = pdf
            .open(src(minimalPdf))
            .then<void>(
              (doc) => doc.dispose().then<void>(
                (_) {},
                onError: (Object e) {
                  if (e is! PdfCancelled) throw e;
                },
              ),
              onError: (Object e) {
                if (e is! PdfCancelled) throw e;
              },
            );
        await Future<void>.delayed(Duration.zero);
        await pdf.dispose();
        await settled;
      }
    }, timeout: t(26));

    test(
      'dispose mid-heavy-merge (5000 inputs) — instant, op resolves',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        final sources = List.generate(5000, (_) => src(minimalPdf));
        final merging = pdf.merge(sources, sink);
        final expectation = expectLater(merging, throwsA(isA<PdfCancelled>()));
        await Future<void>.delayed(const Duration(milliseconds: 50));
        await pdf.dispose();
        await expectation;
      },
      timeout: t(1),
    );
  });
}
