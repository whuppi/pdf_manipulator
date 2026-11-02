// CHARTER — this battery alone proves the abort contract of DataSink: a sink
// handed to an operation that ends cancelled or failed is aborted exactly
// once, never written to after, and closed by its owner as before; an
// operation that succeeds never aborts it. A combinator that ends cancelled
// aborts its output sink even when the save into it had finished, and a
// cancelled operation leaves its editor as it was: a save after it writes
// what a save on a fresh editor writes.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// What a sink saw of an operation that ended in an error: one abort after
/// the writes, no write after it, then the owner's close.
void _expectAbortedOnce(CountingSink sink, {String reason = ''}) {
  expect(sink.aborts, 1, reason: 'aborted once. $reason');
  expect(sink.writesAfterAbort, 0, reason: 'no write after the abort');
  expect(sink.events.takeLast(2), [
    'abort',
    'close',
  ], reason: 'the abort comes before the owner closes the sink');
  expect(sink.length, 0, reason: 'the sink holds no partial output');
}

extension on List<String> {
  List<String> takeLast(int count) => sublist(length - count);
}

/// The bytes a save of [source] writes after [prepare], on a fresh editor.
Future<Uint8List> _saved(
  Pdf pdf,
  Uint8List source,
  Future<void> Function(PdfEditor editor) prepare,
) async {
  final editor = await pdf.edit(src(source));
  await prepare(editor);
  final sink = TestSink();
  await editor.save(sink);
  await editor.dispose();
  return sink.takeBytes();
}

void registerAbortTests(Pdf Function() createPdf) {
  group('abort', () {
    test('a save that succeeds never aborts its sink', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fTwoPageMarkers));
      final sink = CountingSink();
      await editor.save(sink);
      await sink.close();
      await editor.dispose();
      expect(sink.aborts, 0);
      expect(sink.events.last, 'close');
      expect(sink.length, greaterThan(0));
    }, timeout: t(2));

    test(
      'a cancelled save aborts its sink once, then the owner closes it',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fTwoPageMarkers));
        final sink = CountingSink();
        final task = editor.save(sink);
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        await sink.close();
        await editor.dispose();
        _expectAbortedOnce(sink);
      },
      timeout: t(2),
    );

    test(
      'a failed save aborts its sink once, then the owner closes it',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fTwoPageMarkers));
        final sink = CountingSink();
        // The engine refuses an incremental save of a document with a page
        // removed, before it writes a byte.
        await editor.deletePages(const PdfPages.single(0));
        await expectLater(
          editor.save(sink, options: const PdfSaveOptions.incremental()),
          throwsA(
            isA<PdfError>().having(
              (e) => e is PdfCancelled,
              'is a cancel',
              isFalse,
            ),
          ),
        );
        await sink.close();
        await editor.dispose();
        expect(sink.writes, 0, reason: 'the engine failed before it wrote');
        _expectAbortedOnce(sink);
      },
      timeout: t(2),
    );

    test('a cancelled extractText aborts its sink once', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(fTwoPageMarkers));
      final sink = CountingSink();
      final task = doc.extractText(sink);
      task.cancel();
      await expectLater(task, throwsA(isA<PdfCancelled>()));
      await sink.close();
      await doc.dispose();
      _expectAbortedOnce(sink);
    }, timeout: t(2));

    test('a failed extractText aborts its sink once', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(fTwoPageMarkers));
      final sink = CountingSink(failOnWrite: 1);
      await expectLater(
        doc.extractText(sink),
        throwsA(isNot(isA<PdfCancelled>())),
      );
      await sink.close();
      await doc.dispose();
      _expectAbortedOnce(sink);
    }, timeout: t(2));

    test('a cancelled save leaves the editor as it was', () async {
      final pdf = createPdf();
      Future<void> stage(PdfEditor editor) =>
          editor.rotatePages(const PdfPages.single(0), by: PdfRotation.r90);
      final expected = await _saved(pdf, fTwoPageMarkers, stage);

      final editor = await pdf.edit(src(fTwoPageMarkers));
      await stage(editor);
      final cancelled = editor.save(CountingSink());
      cancelled.cancel();
      await expectLater(cancelled, throwsA(isA<PdfCancelled>()));
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(sink.takeBytes(), expected);
    }, timeout: t(2));

    // A cancel at every poll of the engine's loops is proved in the engine
    // (`cancel_at_every_poll`); here the cancel lands before the op runs.
    final mutations = <String, PdfTask<Object?> Function(PdfEditor)>{
      'reduceImages': (editor) => editor.reduceImages(PdfImagePolicy.screen),
      'convertToPdfA': (editor) => editor.convertToPdfA(),
    };
    for (final MapEntry(key: name, value: start) in mutations.entries) {
      test('a cancelled $name leaves the editor as it was', () async {
        final pdf = createPdf();
        final expected = await _saved(pdf, fImagesHires, (_) async {});

        final editor = await pdf.edit(src(fImagesHires));
        final task = start(editor);
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        expect(sink.takeBytes(), expected);
      }, timeout: t(5));
    }

    group('a combinator that ends cancelled aborts its output', () {
      /// Runs [start] with an output sink that cancels the task as its first
      /// write arrives, so the cancel lands once the engine has produced the
      /// document: whether the save had finished or not, the task ends
      /// cancelled and the sink is aborted once.
      Future<void> cancelledFromTheSink(
        PdfTask<void> Function(CountingSink out) start,
      ) async {
        late final PdfTask<void> task;
        final out = CountingSink(onFirstWrite: () => task.cancel());
        task = start(out);
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        await out.close();
        _expectAbortedOnce(out);
        expect(
          out.writes,
          greaterThan(0),
          reason: 'the cancel came from a write',
        );
      }

      test('rewrite', () async {
        final pdf = createPdf();
        await cancelledFromTheSink(
          (out) => pdf.rewrite(src(fTwoPageMarkers), out, (editor) async {}),
        );
      }, timeout: t(2));

      test('buildTo', () async {
        final pdf = createPdf();
        await cancelledFromTheSink(
          (out) => pdf.buildTo(out, (builder) async {
            final page = await builder.addPage();
            await page.text('abort');
            await page.done();
          }),
        );
      }, timeout: t(2));

      test('edits', () async {
        final pdf = createPdf();
        await cancelledFromTheSink(
          (out) => pdf
              .edits(src(fTwoPageMarkers), out)
              .rotatePages(const PdfPages.single(0), by: PdfRotation.r90),
        );
      }, timeout: t(2));

      test('a sugar operation (merge)', () async {
        final pdf = createPdf();
        await cancelledFromTheSink(
          (out) => pdf.merge([src(fTwoPageMarkers), src(minimalPdf)], out),
        );
      }, timeout: t(2));

      test('extractPages', () async {
        final pdf = createPdf();
        await cancelledFromTheSink(
          (out) => pdf.extractPages(src(fTwoPageMarkers), out, pages: [0]),
        );
      }, timeout: t(2));

      test('reads, one extractText', () async {
        final pdf = createPdf();
        await cancelledFromTheSink(
          (out) => pdf.reads(src(fTwoPageMarkers)).extractText(out),
        );
      }, timeout: t(2));

      test('rewrite whose edits throw', () async {
        final pdf = createPdf();
        final out = CountingSink();
        await expectLater(
          pdf.rewrite(src(fTwoPageMarkers), out, (editor) async {
            throw StateError('the edits failed');
          }),
          throwsStateError,
        );
        await out.close();
        expect(out.aborts, 1);
        expect(out.writes, 0, reason: 'nothing is written when edits throw');
        expect(out.events, ['abort', 'close']);
      }, timeout: t(2));

      test(
        'split aborts the chunk in flight and keeps the delivered ones',
        () async {
          final pdf = createPdf();
          final sinks = <CountingSink>[];
          late final PdfTask<void> task;
          task = pdf.split(src(fTwoPageMarkers), (index) {
            final sink = CountingSink(
              onFirstWrite: index == 1 ? () => task.cancel() : null,
            );
            sinks.add(sink);
            return sink;
          }, every: 1);
          await expectLater(task, throwsA(isA<PdfCancelled>()));
          expect(sinks, hasLength(2), reason: 'no chunk after the cancel');
          expect(sinks[0].aborts, 0, reason: 'a delivered chunk stands');
          expect(sinks[0].closes, 1);
          expect(sinks[0].length, greaterThan(0));
          _expectAbortedOnce(sinks[1]);
          expect(
            sinks[1].closes,
            1,
            reason: 'the library closes a factory sink',
          );
        },
        timeout: t(2),
      );
    });

    test(
      'render aborts the page in flight and keeps the delivered ones',
      () async {
        final pdf = createPdf();
        final doc = await pdf.open(src(fTwoPageMarkers));
        final sinks = <CountingSink>[];
        late final PdfTask<List<PdfRenderedPage>> task;
        task = doc.render((page) {
          final sink = CountingSink();
          sinks.add(sink);
          if (sinks.length == 2) task.cancel();
          return sink;
        }, size: const PdfRenderSize.thumbnail(120));
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        await doc.dispose();
        expect(sinks, hasLength(2));
        expect(sinks[0].aborts, 0, reason: 'a delivered page stands');
        expect(sinks[0].closes, 1);
        expect(sinks[0].length, greaterThan(0));
        expect(sinks[1].aborts, 1, reason: 'the page in flight is aborted');
        expect(sinks[1].writesAfterAbort, 0);
        expect(sinks[1].closes, 1, reason: 'the library closes it after');
        expect(sinks[1].events.takeLast(2), ['abort', 'close']);
      },
      timeout: t(5),
    );
  });
}
