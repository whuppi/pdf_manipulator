// CHARTER — this battery alone proves: the one-shot machinery (`edits`,
// `reads`, `info`, `resave`, `rewrite`, `read`, `buildTo`) runs one call and
// leaves no handle open on any path, a sink a factory returns is closed, and
// a handle its developer never disposed is released by the backstop.
//
// Diet: generated foreign fixtures (dart-pdf) + handwritten micro fixtures.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/ops/handle_backstop.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// A sink that counts its closes.
class _CloseCountingSink extends TestSink {
  int closes = 0;

  @override
  void close() => closes++;
}

/// Whether `assert` runs: the debug warning exists only then.
bool get _assertsOn {
  var on = false;
  assert(on = true);
  return on;
}

void registerOneShotTests(Pdf Function() createPdf) {
  group('one-shot', () {
    // ── edits ──

    test('editOnce(...).rotatePages rotates, saves and closes', () async {
      final pdf = createPdf();
      final out = TestSink();
      await pdf
          .editOnce(PdfInput(src(minimalPdf)), out)
          .rotatePages(by: PdfRotation.r90, pages: PdfPages.single(0));
      final doc = await pdf.open(PdfInput(src(out.takeBytes())));
      expect((await doc.pageInfo())[0].rotation, PdfRotation.r90);
      await doc.dispose();
    }, timeout: t(1));

    test('editOnce(...).addWatermark puts the text on the page', () async {
      final pdf = createPdf();
      final out = TestSink();
      await pdf
          .editOnce(PdfInput(src(fTwoPageMarkers)), out)
          .addWatermark('ONESHOTMARK', pages: PdfPages.single(0));
      final doc = await pdf.open(PdfInput(src(out.takeBytes())));
      expect(doc.pageCount, fTwoPageMarkersTruth.pages);
      final hits = await doc.search(
        fTwoPageMarkersTruth.page0Marker,
        pages: const PdfPages.single(0),
      );
      expect(hits, isNotEmpty, reason: 'the page content survives the save');
      await doc.dispose();
    }, timeout: t(1));

    test('a second edit on one object throws before any I/O', () async {
      final pdf = createPdf();
      final out = TestSink();
      final oneShot = pdf.editOnce(PdfInput(src(minimalPdf)), out);
      final first = oneShot.rotatePages(
        by: PdfRotation.r90,
        pages: PdfPages.single(0),
      );
      expect(
        () => oneShot.rotatePages(
          by: PdfRotation.r180,
          pages: PdfPages.single(0),
        ),
        throwsA(
          isA<StateError>().having(
            (e) => e.message,
            'message',
            'a one-shot runs one edit; use rewrite for several',
          ),
        ),
        reason: 'synchronous: the call throws, it does not return a task',
      );
      await first;
      final doc = await pdf.open(PdfInput(src(out.takeBytes())));
      expect(
        (await doc.pageInfo())[0].rotation,
        PdfRotation.r90,
        reason: 'the second edit never ran',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('an edit that fails writes nothing', () async {
      final pdf = createPdf();
      final out = TestSink();
      await expectLater(
        pdf
            .editOnce(PdfInput(src(minimalPdf)), out)
            .rotatePages(by: PdfRotation.r90, pages: PdfPages.single(5)),
        throwsA(isA<PdfError>()),
      );
      expect(out.length, 0);
    }, timeout: t(1));

    test('resave writes a document that reads the same', () async {
      final pdf = createPdf();
      final out = TestSink();
      await pdf.resave(PdfInput(src(fTwoPageMarkers)), out);
      final doc = await pdf.open(PdfInput(src(out.takeBytes())));
      expect(doc.pageCount, fTwoPageMarkersTruth.pages);
      await doc.dispose();
    }, timeout: t(1));

    // ── reads and info ──

    test('readOnce(...).search finds a marker and closes', () async {
      final pdf = createPdf();
      final hits = await pdf
          .readOnce(PdfInput(src(fTwoPageMarkers)))
          .search(
            fTwoPageMarkersTruth.page1Marker,
            pages: const PdfPages.all(),
          );
      expect(hits, isNotEmpty);
      expect(hits.every((h) => h.page == 1), isTrue);
    }, timeout: t(1));

    test('a second read on one object throws before any I/O', () async {
      final pdf = createPdf();
      final oneShot = pdf.readOnce(PdfInput(src(fTwoPageMarkers)));
      final first = oneShot.search('ALPHA', pages: const PdfPages.all());
      expect(
        () => oneShot.search('BRAVO', pages: const PdfPages.all()),
        throwsA(isA<StateError>()),
      );
      await first;
    }, timeout: t(1));

    test(
      'info answers what open would, without keeping the file open',
      () async {
        final pdf = createPdf();
        final info = await pdf.info(PdfInput(src(fTwoPageMarkers)));
        final doc = await pdf.open(PdfInput(src(fTwoPageMarkers)));
        expect(info.pageCount, doc.pageCount);
        expect(info.version, doc.version);
        expect(info.metadata.title, doc.metadata.title);
        expect(info.encryption, doc.encryption);
        expect(info.pageCount, fTwoPageMarkersTruth.pages);
        expect(info.metadata.title, fTwoPageMarkersTruth.title);
        await doc.dispose();
      },
      timeout: t(1),
    );

    // ── combinators close the handle on every path ──

    test('read closes the doc after success and after an error', () async {
      final pdf = createPdf();
      late PdfDocument kept;
      final pages = await pdf.read(PdfInput(src(fTwoPageMarkers)), (doc) async {
        kept = doc;
        return doc.pageCount;
      });
      expect(pages, fTwoPageMarkersTruth.pages);
      expect(() => kept.extractText(MemorySink()), throwsA(isA<StateError>()));

      await expectLater(
        pdf.read<void>(PdfInput(src(fTwoPageMarkers)), (doc) async {
          kept = doc;
          throw const PdfUnsupported('boom');
        }),
        throwsA(isA<PdfUnsupported>()),
      );
      expect(() => kept.extractText(MemorySink()), throwsA(isA<StateError>()));
    }, timeout: t(1));

    test('rewrite saves after the edits and closes the editor', () async {
      final pdf = createPdf();
      final out = TestSink();
      late PdfEditor kept;
      await pdf.rewrite(PdfInput(src(minimalPdf)), out, (editor) async {
        kept = editor;
        await editor.rotatePages(
          by: PdfRotation.r90,
          pages: PdfPages.single(0),
        );
        await editor.setMetadata(
          (await editor.metadata()).copyWith(title: 'Rewritten'),
        );
      });
      expect(
        () => kept.setMetadata(const PdfMetadata(title: 'x')),
        throwsA(isA<StateError>()),
      );
      final doc = await pdf.open(PdfInput(src(out.takeBytes())));
      expect((await doc.pageInfo())[0].rotation, PdfRotation.r90);
      expect(doc.metadata.title, 'Rewritten');
      await doc.dispose();
    }, timeout: t(1));

    test(
      'rewrite closes the editor and writes nothing when the body throws',
      () async {
        final pdf = createPdf();
        final out = TestSink();
        late PdfEditor kept;
        await expectLater(
          pdf.rewrite(PdfInput(src(minimalPdf)), out, (editor) async {
            kept = editor;
            await editor.setMetadata(
              (await editor.metadata()).copyWith(title: 'never saved'),
            );
            throw const PdfUnsupported('boom');
          }),
          throwsA(isA<PdfUnsupported>()),
        );
        expect(
          () => kept.setMetadata(const PdfMetadata(title: 'x')),
          throwsA(isA<StateError>()),
        );
        expect(out.length, 0);
      },
      timeout: t(1),
    );

    test('buildTo saves the pages and closes the builder', () async {
      final pdf = createPdf();
      final out = TestSink();
      late PdfBuilder kept;
      await pdf.buildTo(out, (builder) async {
        kept = builder;
        final page = await builder.addPage();
        await page.paragraph('BUILTTO');
      });
      expect(
        () => kept.setMetadata(const PdfMetadata(title: 'x')),
        throwsA(isA<StateError>()),
      );
      final doc = await pdf.open(PdfInput(src(out.takeBytes())));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'buildTo closes the builder and writes nothing when the body throws',
      () async {
        final pdf = createPdf();
        final out = TestSink();
        late PdfBuilder kept;
        await expectLater(
          pdf.buildTo(out, (builder) async {
            kept = builder;
            throw const PdfUnsupported('boom');
          }),
          throwsA(isA<PdfUnsupported>()),
        );
        expect(
          () => kept.setMetadata(const PdfMetadata(title: 'x')),
          throwsA(isA<StateError>()),
        );
        expect(out.length, 0);
      },
      timeout: t(1),
    );

    // ── a sink a factory returns is the library's ──

    test('split closes each sink its factory returned', () async {
      final pdf = createPdf();
      final sinks = <_CloseCountingSink>[];
      await pdf.split(PdfInput(src(fThreePageMarkers)), (part) {
        final s = _CloseCountingSink();
        sinks.add(s);
        return s;
      }, by: const PdfSplitRule.every(1));
      expect(sinks, hasLength(3));
      expect(sinks.map((s) => s.closes), everyElement(1));
    }, timeout: t(1));

    test('split by maxBytes closes each sink its factory returned', () async {
      final pdf = createPdf();
      final sinks = <_CloseCountingSink>[];
      final parts = await pdf.split(PdfInput(src(fThreePageMarkers)), (part) {
        final s = _CloseCountingSink();
        sinks.add(s);
        return s;
      }, by: const PdfSplitRule.maxBytes(1 << 20));
      expect(sinks, hasLength(parts.length));
      expect(sinks.map((s) => s.closes), everyElement(1));
    }, timeout: t(1));

    test('a sink the caller passes is not closed by the library', () async {
      final pdf = createPdf();
      final out = _CloseCountingSink();
      await pdf.merge([
        PdfInput(src(minimalPdf)),
        PdfInput(src(minimalPdf)),
      ], out);
      expect(out.closes, 0);
    }, timeout: t(1));

    // ── the backstop for a handle nobody disposed ──

    group('backstop', () {
      late List<String> warnings;
      late void Function(String) saved;

      setUp(() {
        warnings = [];
        saved = HandleBackstop.warn;
        HandleBackstop.warn = warnings.add;
      });
      tearDown(() => HandleBackstop.warn = saved);

      test('an undisposed editor is released and logged', () async {
        final pdf = createPdf();
        final editor = await pdf.edit(PdfInput(src(minimalPdf)));
        await HandleBackstop.finalizeNow(editor);
        expect(
          () => editor.pageCount(),
          throwsA(isA<PdfEngineFailed>()),
          reason: 'the engine no longer holds the editor',
        );
        if (_assertsOn) {
          expect(warnings, hasLength(1));
          expect(
            warnings.single,
            contains('a PdfEditorHandle was never disposed'),
          );
          expect(warnings.single, contains('Opened at'));
        }
      }, timeout: t(1));

      test('an undisposed doc is released and logged', () async {
        final pdf = createPdf();
        final doc = await pdf.open(PdfInput(src(minimalPdf)));
        await HandleBackstop.finalizeNow(doc);
        await expectLater(
          doc.extractText(MemorySink()),
          throwsA(isA<PdfError>()),
        );
        if (_assertsOn) {
          expect(
            warnings.single,
            contains('a PdfDocumentHandle was never disposed'),
          );
        }
      }, timeout: t(1));

      test('an undisposed builder is released and logged', () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        await HandleBackstop.finalizeNow(builder);
        await expectLater(builder.addPage(), throwsA(isA<PdfError>()));
        if (_assertsOn) {
          expect(
            warnings.single,
            contains('a PdfBuilderHandle was never disposed'),
          );
        }
      }, timeout: t(1));

      test(
        'dispose detaches the backstop: nothing is released twice',
        () async {
          final pdf = createPdf();
          final editor = await pdf.edit(PdfInput(src(minimalPdf)));
          await editor.dispose();
          await expectLater(
            HandleBackstop.finalizeNow(editor),
            throwsA(isA<ArgumentError>()),
          );
          expect(warnings, isEmpty);
        },
        timeout: t(1),
      );

      test('a handle the library opens never reaches the backstop', () async {
        final pdf = createPdf();
        late PdfEditor kept;
        await pdf.rewrite(
          PdfInput(src(minimalPdf)),
          TestSink(),
          (e) async => kept = e,
        );
        await expectLater(
          HandleBackstop.finalizeNow(kept),
          throwsA(isA<ArgumentError>()),
        );
      }, timeout: t(1));
    });
  });
}
