// CHARTER — this battery alone proves: cancelling a combinator (`read`,
// `rewrite`, `buildTo`, `edits`, `reads`) reaches every op its body issued:
// an op in flight resolves PdfCancelled, an op issued after the cancel
// resolves PdfCancelled at once, the handle is disposed and the sink gets
// nothing. The hook's own contract is in test/types/cancel_hook_test.dart.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

const _pageCount = 200;

Future<Uint8List> _buildPages(Pdf pdf) async {
  final builder = await pdf.build();
  for (var i = 0; i < _pageCount; i++) {
    final page = await builder.addPage(PdfPageSize.custom(612, 792));
    await page.text('combinator cancel page $i');
    await page.done();
  }
  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

/// How many source reads opening [bytes] makes: every read past it belongs to
/// the op that follows the open.
Future<int> _readsAtOpen(Pdf pdf, Uint8List bytes) async {
  var reads = 0;
  final doc = await pdf.open(TestSource(bytes, onRead: (_, _) => reads++));
  await doc.dispose();
  return reads;
}

/// [skipMidOp] is a reason when this runner cannot land a cancel on a
/// running op (a web page without SharedArrayBuffer in OPFS mode), null
/// everywhere else. [skipExtract] is a reason when the runner's engine makes
/// no source read after open to mark that an op started (OPFS pre-copies its
/// sources).
void registerCombinatorCancelTests(
  Pdf Function() createPdf, {
  String? skipMidOp,
  String? skipExtract,
}) {
  group('combinator cancel', () {
    Uint8List? built;

    Future<Uint8List> pages200(Pdf pdf) async =>
        built ??= await _buildPages(pdf);

    test(
      'read: a body that catches the cancellation still ends cancelled',
      () async {
        final pdf = createPdf();
        late final PdfTask<String> task;
        task = pdf.read(src(fTwoPageMarkers), (doc) async {
          final op = doc.search(query: 'x', pages: const PdfPages.all());
          task.cancel();
          try {
            await op;
          } on PdfCancelled {
            // A body may swallow its op's cancellation; the job still ends.
          }
          return 'value';
        });
        await expectLater(task, throwsA(isA<PdfCancelled>()));
      },
      timeout: t(2),
    );

    test(
      'read: cancel reaches the op in flight and the ops after it',
      () async {
        final pdf = createPdf();
        late final PdfTask<void> task;
        late PdfDoc kept;
        Object? inFlight;
        Object? after;
        task = pdf.read(src(fTwoPageMarkers), (doc) async {
          kept = doc;
          final op = doc.search(query: 'x', pages: const PdfPages.all());
          task.cancel();
          try {
            await op;
          } on Object catch (e) {
            inFlight = e;
          }
          try {
            await doc.search(query: 'x', pages: const PdfPages.all());
          } on Object catch (e) {
            after = e;
            rethrow;
          }
        });
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(inFlight, isA<PdfCancelled>(), reason: 'the op in flight');
        expect(after, isA<PdfCancelled>(), reason: 'the op after the cancel');
        expect(
          () => kept.search(query: 'x', pages: const PdfPages.all()),
          throwsA(isA<PdfDisposed>()),
        );
      },
      timeout: t(2),
    );

    test(
      'read: cancel stops an op the engine is running',
      () async {
        final pdf = createPdf();
        final bytes = await pages200(pdf);
        var armed = false;
        final started = Completer<void>();
        late PdfDoc kept;
        final task = pdf.read(
          TestSource(
            bytes,
            onRead: (_, _) {
              if (armed && !started.isCompleted) started.complete();
            },
          ),
          (doc) {
            kept = doc;
            armed = true;
            return doc.extract(pages: const PdfPages.all());
          },
        );
        await started.future; // the engine is genuinely mid-op
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(
          () => kept.extract(pages: const PdfPages.all()),
          throwsA(isA<PdfDisposed>()),
        );
      },
      timeout: t(5),
      skip: skipMidOp ?? skipExtract,
    );

    test(
      'rewrite: cancel reaches the op in flight and writes nothing',
      () async {
        final pdf = createPdf();
        final out = TestSink();
        late final PdfTask<void> task;
        late PdfEditor kept;
        Object? inFlight;
        Object? after;
        task = pdf.rewrite(src(minimalPdf), out, (editor) async {
          kept = editor;
          final op = editor.rotatePage(0, by: PdfRotation.r90);
          task.cancel();
          try {
            await op;
          } on Object catch (e) {
            inFlight = e;
          }
          try {
            await editor.setTitle('after the cancel');
          } on Object catch (e) {
            after = e;
          }
        });
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(inFlight, isA<PdfCancelled>(), reason: 'the op in flight');
        expect(after, isA<PdfCancelled>(), reason: 'the op after the cancel');
        expect(() => kept.setTitle('x'), throwsA(isA<PdfDisposed>()));
        expect(out.length, 0, reason: 'the save never ran');
      },
      timeout: t(2),
    );

    test(
      'buildTo: cancel reaches builder and page ops and writes nothing',
      () async {
        final pdf = createPdf();
        final out = TestSink();
        late final PdfTask<void> task;
        late PdfBuilder kept;
        Object? builderOp;
        Object? pageOp;
        Object? after;
        task = pdf.buildTo(out, (builder) async {
          kept = builder;
          final page = await builder.addPage();
          final pageInFlight = page.text('in flight');
          final builderInFlight = builder.addPage();
          task.cancel();
          try {
            await pageInFlight;
          } on Object catch (e) {
            pageOp = e;
          }
          try {
            await builderInFlight;
          } on Object catch (e) {
            builderOp = e;
          }
          try {
            await page.text('after the cancel');
          } on Object catch (e) {
            after = e;
          }
        });
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(pageOp, isA<PdfCancelled>(), reason: 'a page op in flight');
        expect(
          builderOp,
          isA<PdfCancelled>(),
          reason: 'a builder op in flight',
        );
        expect(
          after,
          isA<PdfCancelled>(),
          reason: 'a page op after the cancel',
        );
        expect(
          () => kept.setMetadata(const PdfMetadata(title: 'x')),
          throwsA(isA<PdfDisposed>()),
        );
        expect(out.length, 0, reason: 'the save never ran');
      },
      timeout: t(2),
    );

    test(
      'reads: cancel stops the one op the engine is running',
      () async {
        final pdf = createPdf();
        final bytes = await pages200(pdf);
        final opened = await _readsAtOpen(pdf, bytes);
        var reads = 0;
        late final PdfTask<List<PdfSearchHit>> task;
        task = pdf
            .reads(
              TestSource(
                bytes,
                onRead: (_, _) {
                  if (++reads == opened + 1) task.cancel();
                },
              ),
            )
            .search(query: 'combinator', pages: const PdfPages.all());
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(reads, greaterThan(opened), reason: 'the op had started');
      },
      timeout: t(5),
      skip: skipMidOp ?? skipExtract,
    );

    test(
      'edits: cancel stops the one-shot once the edit has started',
      () async {
        final pdf = createPdf();
        final bytes = await pages200(pdf);
        final opened = await _readsAtOpen(pdf, bytes);
        var reads = 0;
        late final PdfTask<void> task;
        task = pdf
            .edits(
              TestSource(
                bytes,
                onRead: (_, _) {
                  if (++reads == opened + 1) task.cancel();
                },
              ),
              TestSink(),
            )
            .rotatePage(0, by: PdfRotation.r90);
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(reads, greaterThan(opened), reason: 'the edit had started');
      },
      timeout: t(5),
      skip: skipMidOp ?? skipExtract,
    );
  });
}
