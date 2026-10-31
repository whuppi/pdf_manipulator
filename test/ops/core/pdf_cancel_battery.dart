// CHARTER — this battery alone proves: a cancel reaches a running op
// and a streamed render stops within a page of it. Each CPU loop's
// stop-within-one-item contract is proven in Rust (CancelCheck::each);
// here the transport carries the cancel end to end.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/bridge/shared_bridge.dart'
    show onEngineFrame;
import 'package:test/test.dart';

import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

const _pageCount = 200;

/// Frames the engine may write after the consumer cancelled: the frames of
/// one page (header, data, end) already in flight when the cancel lands.
const _framesAfterCancelBound = 3;

/// A sink that counts its closes and the bytes it was given.
class _ClosesCounted implements DataSink {
  int closes = 0;
  int bytes = 0;

  @override
  void close() => closes++;

  @override
  FutureOr<void> write(Uint8List chunk) {
    bytes += chunk.length;
  }
}

Future<Uint8List> _buildPages(Pdf pdf) async {
  final builder = await pdf.build();
  for (var i = 0; i < _pageCount; i++) {
    final page = await builder.addPage(width: 612, height: 792);
    await page.text('cancel battery page $i');
    await page.done();
  }
  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

/// [skipMidOp] is a reason when this runner cannot land a cancel on a
/// running op (a web page without SharedArrayBuffer in OPFS mode),
/// null everywhere else. [skipExtract] is a reason when the runner's
/// engine makes no source read after open to mark that an op started
/// (OPFS pre-copies its sources).
void registerCancelTests(
  Pdf Function() createPdf, {
  String? skipMidOp,
  String? skipExtract,
}) {
  group('cancel mid-loop', () {
    Uint8List? built;

    Future<Uint8List> pages200(Pdf pdf) async =>
        built ??= await _buildPages(pdf);

    test(
      'extract of all pages cancelled mid-op ends PdfCancelled',
      () async {
        final pdf = createPdf();
        final bytes = await pages200(pdf);

        var armed = false;
        final started = Completer<void>();
        final doc = await pdf.open(
          TestSource(
            bytes,
            onRead: (_, _) {
              if (armed && !started.isCompleted) started.complete();
            },
          ),
        );
        expect(doc.pageCount, _pageCount);

        armed = true;
        final task = doc.extract(pages: const PdfPages.all());
        await started.future; // the engine is genuinely mid-op
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        await doc.dispose();
      },
      timeout: t(5),
      skip: skipMidOp ?? skipExtract,
    );

    test(
      'render cancelled on the first frame stops the engine',
      () async {
        final pdf = createPdf();
        final bytes = await pages200(pdf);
        final doc = await pdf.open(src(bytes));

        var written = 0;
        onEngineFrame = () => written++;
        addTearDown(() => onEngineFrame = null);

        var opened = 0;
        var writtenAtCancel = -1;
        final sinks = <_ClosesCounted>[];
        late final PdfTask<List<PdfRenderedPage>> task;
        task = doc.render(
          (page) {
            opened++;
            if (opened == 1) {
              writtenAtCancel = written;
              task.cancel();
            }
            final sink = _ClosesCounted();
            sinks.add(sink);
            return sink;
          },
          pages: const PdfPages.all(),
          size: const PdfRenderSize.thumbnail(120),
        );
        await expectLater(task, throwsA(isA<PdfCancelled>()));

        // The lane runs one job at a time: once this read returns, the
        // render job is over.
        await doc.extract(pages: const PdfPages.single(0));

        expect(opened, lessThanOrEqualTo(2));
        expect(
          written - writtenAtCancel,
          lessThanOrEqualTo(_framesAfterCancelBound),
          reason:
              'frames the engine wrote after the cancel: '
              '${written - writtenAtCancel} of $_pageCount pages',
        );
        expect(
          sinks.map((s) => s.closes),
          everyElement(1),
          reason: 'the sink of the page in flight is closed, once',
        );
        expect(
          sinks.first.bytes,
          0,
          reason: 'a cancel stops the writes: the first page got no data',
        );
        await doc.dispose();
      },
      timeout: t(5),
      skip: skipMidOp,
    );
  });
}
