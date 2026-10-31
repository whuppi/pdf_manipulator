// CHARTER — this battery alone proves: render writes each page to the sink
// its factory returns, with the page's size known before any byte, in chunks
// of at most 256 KiB that the sink is awaited for, and closes every sink on
// success, error and cancel. That the PNG rows equal the whole-buffer
// encoder's pixels is proven in Rust (engine_proofs/render_stream.rs): no
// Dart test can reach the encoder it is compared with.
//
// Diet: handwritten and generated foreign fixtures.

import 'dart:async';
import 'dart:typed_data';

import 'package:image/image.dart' as img;
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/render_pages.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// A sink that logs what happens to it, in order, into a shared journal.
class _JournalSink implements DataSink {
  _JournalSink(
    this.name,
    this.journal, {
    this.failOnWrite = false,
    this.onClose,
  });

  final String name;
  final List<String> journal;
  final bool failOnWrite;
  final void Function()? onClose;
  final _bytes = BytesBuilder(copy: false);
  final writes = <int>[];
  int closes = 0;
  int inFlight = 0;
  int overlapped = 0;

  Uint8List get bytes => _bytes.toBytes();

  @override
  Future<void> write(Uint8List chunk) async {
    if (++inFlight > 1) overlapped++;
    journal.add('$name write ${chunk.length}');
    if (failOnWrite) {
      inFlight--;
      throw const _SinkBoom();
    }
    // A slow sink: the engine must wait for each write, not run ahead.
    await Future<void>.delayed(const Duration(milliseconds: 1));
    writes.add(chunk.length);
    _bytes.add(chunk);
    inFlight--;
  }

  @override
  void close() {
    closes++;
    journal.add('$name close');
    onClose?.call();
  }
}

class _SinkBoom implements Exception {
  const _SinkBoom();
}

img.Image _decode(Uint8List png) {
  final decoded = img.decodePng(png);
  expect(decoded, isNotNull, reason: 'the sink must hold a valid PNG');
  return decoded!;
}

void registerRenderTests(Pdf Function() createPdf) {
  group('render', () {
    test('the default size is 150 dpi: a letter page is 1275 x 1650', () async {
      final doc = await createPdf().open(src(letterPdf));
      final pages = await renderToMemory(doc, pages: const PdfPages.single(0));
      expect(pages.single.width, 1275);
      expect(pages.single.height, 1650);
      final bitmap = _decode(pages.single.data);
      expect((bitmap.width, bitmap.height), (1275, 1650));
      await doc.dispose();
    }, timeout: t(2));

    test('dpi, fit and thumbnail size the page from the page box', () async {
      final doc = await createPdf().open(src(letterPdf));
      Future<RenderedBytes> one(PdfRenderSize size) async =>
          (await renderToMemory(
            doc,
            pages: const PdfPages.single(0),
            size: size,
          )).single;

      final dpi72 = await one(const PdfRenderSize.dpi(72));
      expect((dpi72.width, dpi72.height), (612, 792));
      final fit = await one(
        const PdfRenderSize.fit(maxWidth: 100, maxHeight: 100),
      );
      expect(fit.height, 100, reason: 'the taller side fills the box');
      expect(fit.width, closeTo(77, 1));
      final thumb = await one(const PdfRenderSize.thumbnail(200));
      expect(thumb.height, 200);
      expect(thumb.width, closeTo(155, 1));
      for (final page in [dpi72, fit, thumb]) {
        final bitmap = _decode(page.data);
        expect((bitmap.width, bitmap.height), (page.width, page.height));
      }
      await doc.dispose();
    }, timeout: t(2));

    test('a non-positive size is a PdfInvalidArgument', () async {
      final doc = await createPdf().open(src(letterPdf));
      expect(
        () => doc.render(
          (page) => MemorySink(),
          pages: const PdfPages.single(0),
          size: const PdfRenderSize.dpi(0),
        ),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'synchronous: the call throws, it does not return a task',
      );
      expect(
        () => doc.render(
          (page) => MemorySink(),
          pages: const PdfPages.single(0),
          size: const PdfRenderSize.thumbnail(-5),
        ),
        throwsA(isA<PdfInvalidArgument>()),
      );
      await doc.dispose();
    }, timeout: t(2));

    test(
      'rgba rows are the PNG decoded: straight alpha, top to bottom',
      () async {
        final doc = await createPdf().open(src(fTwoPageMarkers));
        const size = PdfRenderSize.thumbnail(220);
        final png = await renderToMemory(
          doc,
          pages: const PdfPages.single(0),
          size: size,
        );
        final rgba = await renderToMemory(
          doc,
          pages: const PdfPages.single(0),
          size: size,
          format: PdfRenderFormat.rgba,
        );
        final bitmap = _decode(png.single.data);
        expect(
          (rgba.single.width, rgba.single.height),
          (bitmap.width, bitmap.height),
        );
        expect(rgba.single.data.length, bitmap.width * bitmap.height * 4);
        final decoded = bitmap.getBytes(order: img.ChannelOrder.rgba);
        expect(rgba.single.data, decoded);
        var ink = 0;
        for (var i = 0; i < decoded.length; i += 4) {
          if (decoded[i] < 128) ink++;
        }
        expect(ink, greaterThan(0), reason: 'the fixture has text to compare');
        await doc.dispose();
      },
      timeout: t(2),
    );

    test('a blank page is white in every pixel, alpha opaque', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final rgba = await renderToMemory(
        doc,
        pages: const PdfPages.single(0),
        size: const PdfRenderSize.dpi(36),
        format: PdfRenderFormat.rgba,
      );
      expect(rgba.single.data.every((b) => b == 255), isTrue);
      await doc.dispose();
    }, timeout: t(2));

    test('the factory sees the size before any byte is written', () async {
      final doc = await createPdf().open(src(fThreePageMarkers));
      final journal = <String>[];
      final sinks = <_JournalSink>[];
      final seen = <PdfRenderedPage>[];
      final pages = await doc.render(
        (page) {
          seen.add(page);
          journal.add('factory ${page.page} ${page.width}x${page.height}');
          final sink = _JournalSink('p${page.page}', journal);
          sinks.add(sink);
          return sink;
        },
        pages: const PdfPages.all(),
        size: const PdfRenderSize.thumbnail(160),
      );
      expect(pages, seen);
      expect(pages.map((p) => p.page), [0, 1, 2]);
      for (var i = 0; i < 3; i++) {
        final firstEvent = journal.indexWhere((e) => e.startsWith('p$i '));
        expect(
          journal.indexWhere((e) => e.startsWith('factory $i ')),
          lessThan(firstEvent),
          reason: 'page $i: the descriptor comes before its first write',
        );
        final bitmap = _decode(sinks[i].bytes);
        expect(
          (bitmap.width, bitmap.height),
          (pages[i].width, pages[i].height),
        );
      }
      await doc.dispose();
    }, timeout: t(2));

    test(
      'every sink is closed once, after its page and in page order',
      () async {
        final doc = await createPdf().open(src(fThreePageMarkers));
        final journal = <String>[];
        final sinks = <_JournalSink>[];
        await doc.render(
          (page) {
            final sink = _JournalSink('p${page.page}', journal);
            sinks.add(sink);
            return sink;
          },
          pages: const PdfPages.all(),
          size: const PdfRenderSize.thumbnail(160),
        );
        expect(sinks.map((s) => s.closes), everyElement(1));
        final closes = journal.where((e) => e.endsWith('close')).toList();
        expect(closes, ['p0 close', 'p1 close', 'p2 close']);
        for (var i = 0; i < 3; i++) {
          expect(
            journal.lastIndexWhere((e) => e.startsWith('p$i write')),
            lessThan(journal.indexOf('p$i close')),
            reason: 'a sink closes after its last write',
          );
        }
        await doc.dispose();
      },
      timeout: t(2),
    );

    test('writes are at most 256 KiB, in order, one at a time', () async {
      final doc = await createPdf().open(src(letterPdf));
      final sinks = <_JournalSink>[];
      final pages = await doc.render(
        (page) {
          final sink = _JournalSink('p', []);
          sinks.add(sink);
          return sink;
        },
        pages: const PdfPages.single(0),
        format: PdfRenderFormat.rgba,
      );
      final sink = sinks.single;
      expect(sink.bytes.length, pages.single.width * pages.single.height * 4);
      expect(sink.writes.length, greaterThan(1), reason: 'many frames');
      expect(
        sink.writes.reduce((a, b) => a > b ? a : b),
        lessThanOrEqualTo(256 * 1024),
      );
      expect(sink.overlapped, 0, reason: 'a write is awaited before the next');
      await doc.dispose();
    }, timeout: t(3));

    test(
      'a sink that throws ends the render with its error, sinks closed',
      () async {
        final doc = await createPdf().open(src(fThreePageMarkers));
        final sinks = <_JournalSink>[];
        await expectLater(
          doc.render(
            (page) {
              final sink = _JournalSink(
                'p${page.page}',
                [],
                failOnWrite: page.page == 1,
              );
              sinks.add(sink);
              return sink;
            },
            pages: const PdfPages.all(),
            size: const PdfRenderSize.thumbnail(160),
          ),
          throwsA(isA<_SinkBoom>()),
          reason: 'the caller error, not the engine echo of it',
        );
        expect(sinks, hasLength(2), reason: 'page 2 never started');
        expect(sinks.map((s) => s.closes), everyElement(1));
        expect(_decode(sinks[0].bytes).width, greaterThan(0));
        await doc.dispose();
      },
      timeout: t(2),
    );

    test('a factory that throws ends the render with its error', () async {
      final doc = await createPdf().open(src(fTwoPageMarkers));
      var calls = 0;
      await expectLater(
        doc.render(
          (page) {
            calls++;
            throw const _SinkBoom();
          },
          pages: const PdfPages.all(),
          size: const PdfRenderSize.thumbnail(160),
        ),
        throwsA(isA<_SinkBoom>()),
      );
      expect(calls, 1);
      await doc.dispose();
    }, timeout: t(2));

    test(
      'a page past the end is PdfPageRangeError and opens no sink',
      () async {
        final doc = await createPdf().open(src(minimalPdf));
        var opened = 0;
        await expectLater(
          doc.render((page) {
            opened++;
            return MemorySink();
          }, pages: const PdfPages.single(4)),
          throwsA(isA<PdfPageRangeError>()),
        );
        expect(opened, 0);
        await doc.dispose();
      },
      timeout: t(2),
    );

    test(
      'a cancel keeps the pages delivered and closes the page in flight',
      () async {
        final doc = await createPdf().open(src(fThreePageMarkers));
        final sinks = <_JournalSink>[];
        late final PdfTask<List<PdfRenderedPage>> task;
        task = doc.render(
          (page) {
            // Page 0's sink cancels the render when it is closed: the page is
            // delivered, and nothing after it may be.
            final sink = _JournalSink(
              'p${page.page}',
              [],
              onClose: page.page == 0 ? () => task.cancel() : null,
            );
            sinks.add(sink);
            return sink;
          },
          pages: const PdfPages.all(),
          size: const PdfRenderSize.thumbnail(160),
        );
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(sinks, hasLength(1), reason: 'no page opens after the cancel');
        expect(sinks.single.closes, 1);
        expect(
          _decode(sinks.single.bytes).width,
          greaterThan(0),
          reason: 'the delivered page stands',
        );
        await doc.dispose();
      },
      timeout: t(3),
    );

    // ── the one-shot ──

    test('pdf.reads(src).render writes the pages and is single-use', () async {
      final pdf = createPdf();
      final reads = pdf.reads(src(fTwoPageMarkers));
      final sinks = <MemorySink>[];
      final first = reads.render(
        (page) {
          final sink = MemorySink();
          sinks.add(sink);
          return sink;
        },
        pages: const PdfPages.all(),
        size: const PdfRenderSize.thumbnail(120),
      );
      expect(
        () => reads.render((p) => MemorySink(), pages: const PdfPages.all()),
        throwsA(isA<PdfInvalidArgument>()),
      );
      final pages = await first;
      expect(pages, hasLength(2));
      for (final sink in sinks) {
        expect(_decode(sink.takeBytes()).height, 120);
      }
    }, timeout: t(2));

    test('the factory may be async', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final sink = MemorySink();
      await doc.render(
        (page) async {
          await Future<void>.delayed(const Duration(milliseconds: 2));
          return sink;
        },
        pages: const PdfPages.single(0),
        size: const PdfRenderSize.thumbnail(100),
      );
      expect(_decode(sink.takeBytes()).height, 100);
      await doc.dispose();
    }, timeout: t(2));
  });
}
