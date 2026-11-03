// CHARTER — this battery alone proves: every one-shot sugar op
// transforms CONTENT correctly on foreign-produced PDFs (order moves,
// pages die with their content, overlays land, encryption locks).
// Scale claims live in the stress batteries; editor-session claims in
// the editor battery; creation claims in the builder battery.
//
// Diet: generated foreign fixtures (dart-pdf) + handwritten micro
// fixtures. Never this package's own builder output — self-feeding
// lets a writer bug and a reader bug mirror into green tests.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/extract_images.dart';
import '../../harness/extract_text.dart';
import '../../harness/page_prune_expect.dart';
import '../../harness/render_pages.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerSugarTests(Pdf Function() createPdf) {
  group('sugar', () {
    // ── Merge ──

    test('merge two PDFs → 2 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));

    test('merge three PDFs → 3 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([
        src(minimalPdf),
        src(minimalPdf),
        src(minimalPdf),
      ], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
      await doc.dispose();
    }, timeout: t(1));

    test('merge different sized PDFs', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(letterPdf)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));

    test('merged output is valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], sink);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x25); // %PDF
      expect(bytes[1], 0x50);
      expect(bytes[2], 0x44);
      expect(bytes[3], 0x46);
    }, timeout: t(1));

    test('merge opens a password-protected source with its password', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.resave(
        src(minimalPdf),
        encSink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.config(
            ownerPassword: 'owner-pw',
            userPassword: 'user-pw',
          ),
        ),
      );
      final encrypted = src(encSink.takeBytes());
      final plain = src(minimalPdf);

      final later = TestSink();
      await pdf.merge(
        [plain, encrypted],
        later,
        passwords: {encrypted: 'user-pw'},
      );
      final laterDoc = await pdf.open(src(later.takeBytes()));
      expect(laterDoc.pageCount, 2);
      await laterDoc.dispose();

      final first = TestSink();
      await pdf.merge(
        [encrypted, plain],
        first,
        passwords: {encrypted: 'user-pw'},
      );
      final firstDoc = await pdf.open(
        src(first.takeBytes()),
        password: 'user-pw',
      );
      expect(firstDoc.pageCount, 2);
      await firstDoc.dispose();
    }, timeout: t(1));

    test('merge of a protected source without its password throws', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.resave(
        src(minimalPdf),
        encSink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.config(
            ownerPassword: 'owner-pw',
            userPassword: 'user-pw',
          ),
        ),
      );
      final encrypted = src(encSink.takeBytes());
      final plain = src(minimalPdf);
      await expectLater(
        pdf.merge([plain, encrypted], TestSink()),
        throwsA(isA<PdfPasswordRequired>()),
      );
      await expectLater(
        pdf.merge(
          [plain, encrypted],
          TestSink(),
          passwords: {encrypted: 'not-the-password'},
        ),
        throwsA(isA<PdfWrongPassword>()),
      );
    }, timeout: t(1));

    test('merge refuses a password for a source it was not given', () {
      final pdf = createPdf();
      expect(
        () => pdf.merge(
          [src(minimalPdf)],
          TestSink(),
          passwords: {src(minimalPdf): 'x'},
        ),
        throwsA(isA<PdfInvalidArgument>()),
      );
    });

    // ── Rotate ──

    test('rotating every page 90° sets rotation', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf
          .edits(src(minimalPdf), sink)
          .rotatePages(const PdfPages.all(), by: PdfRotation.r90);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect((await doc.pageInfo())[0].rotation, PdfRotation.r90);
      await doc.dispose();
    }, timeout: t(1));

    test('rotatePages rotates specific pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf
          .edits(src(minimalPdf), sink)
          .rotatePages(const PdfPages.single(0), by: PdfRotation.r90);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect((await doc.pageInfo())[0].rotation, PdfRotation.r90);
      await doc.dispose();
    }, timeout: t(1));

    // ── Flatten ──

    test('flattenForms renders the untouched default value', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final sink = TestSink();
      await pdf.edits(src(formBytes), sink).flattenForms();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, fFormFieldsTruth.pages);
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(
        text,
        contains(fFormFieldsTruth.textFieldDefault),
        reason:
            'flattening a form nobody filled must render the '
            'producer\'s default value into the page',
      );
      await doc.dispose();
    }, timeout: t(1));

    // ── Compress ──

    test('compress preserves page count', () async {
      final pdf = createPdf();
      final multiPage = fThreePageMarkers;
      final sink = TestSink();
      await pdf.compress(src(multiPage), sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 3);
      expect(output.length, lessThanOrEqualTo(multiPage.length * 2));
      await doc.dispose();
    }, timeout: t(1));

    test(
      'compress opens a password-protected source with its password',
      () async {
        final pdf = createPdf();
        final encSink = TestSink();
        await pdf.resave(
          src(fThreePageMarkers),
          encSink,
          options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.config(
              ownerPassword: 'owner-pw',
              userPassword: 'user-pw',
            ),
          ),
        );
        final encrypted = encSink.takeBytes();
        await expectLater(
          pdf.compress(src(encrypted), TestSink()),
          throwsA(isA<PdfPasswordRequired>()),
        );
        final sink = TestSink();
        await pdf.compress(src(encrypted), sink, password: 'user-pw');
        final doc = await pdf.open(src(sink.takeBytes()), password: 'user-pw');
        expect(doc.pageCount, 3);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('compress writes one copy of an image and a font merged in three '
        'times', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([
        src(imageAndFontPdf),
        src(imageAndFontPdf),
        src(imageAndFontPdf),
      ], mergeSink);
      final merged = mergeSink.takeBytes();

      final aloneSink = TestSink();
      await pdf.compress(
        src(imageAndFontPdf),
        aloneSink,
        images: PdfImagePolicy.lossless,
      );
      final alone = aloneSink.takeBytes();
      expect(alone.length, lessThan(imageAndFontPdf.length));

      final sink = TestSink();
      await pdf.compress(src(merged), sink, images: PdfImagePolicy.lossless);
      final output = sink.takeBytes();
      expect(output.length, lessThan(alone.length * 1.3));

      final before = await pdf.open(src(merged));
      final after = await pdf.open(src(output));
      expect(after.pageCount, 3);
      final text = await extractTextToString(
        after,
        pages: const PdfPages.all(),
      );
      expect(
        text,
        await extractTextToString(before, pages: const PdfPages.all()),
      );
      expect(
        RegExp(imageAndFontMarker).allMatches(text),
        hasLength(3),
        reason: 'the text of every page survives',
      );
      for (var page = 0; page < 3; page++) {
        final expected = await extractToMemory(
          before,
          pages: PdfPages.single(page),
        );
        final actual = await extractToMemory(
          after,
          pages: PdfPages.single(page),
        );
        expect(actual, hasLength(1));
        expect(actual.single.image.width, expected.single.image.width);
        expect(actual.single.data, expected.single.data);
      }
      final renderedBefore = await renderToMemory(
        before,
        pages: const PdfPages.all(),
      );
      final renderedAfter = await renderToMemory(
        after,
        pages: const PdfPages.all(),
      );
      expect(renderedAfter, hasLength(3));
      for (var page = 0; page < 3; page++) {
        expect(renderedAfter[page].data, renderedBefore[page].data);
      }
      await before.dispose();
      await after.dispose();
    }, timeout: t(2));

    test('a full rewrite without mergeDuplicates keeps every copy', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([
        src(imageAndFontPdf),
        src(imageAndFontPdf),
        src(imageAndFontPdf),
      ], mergeSink);
      final merged = mergeSink.takeBytes();
      final aloneSink = TestSink();
      await pdf.compress(
        src(imageAndFontPdf),
        aloneSink,
        images: PdfImagePolicy.lossless,
      );

      final editor = await pdf.edit(src(merged));
      final kept = TestSink();
      await editor.save(
        kept,
        options: const PdfSaveOptions.fullRewrite(mergeDuplicates: false),
      );
      final merging = TestSink();
      await editor.save(
        merging,
        options: const PdfSaveOptions.fullRewrite(mergeDuplicates: true),
      );
      await editor.dispose();
      final aloneLength = aloneSink.takeBytes().length;
      expect(kept.takeBytes().length, greaterThan(aloneLength * 2.5));
      expect(merging.takeBytes().length, lessThan(aloneLength * 1.3));
    }, timeout: t(2));

    test('compress drops the fonts and images no content uses', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.compress(
        src(unusedResourcesPdf),
        sink,
        images: PdfImagePolicy.lossless,
      );
      final pruned = sink.takeBytes();

      final editor = await pdf.edit(src(unusedResourcesPdf));
      final keptSink = TestSink();
      await editor.save(
        keptSink,
        options: const PdfSaveOptions.fullRewrite(
          mergeDuplicates: true,
          pruneResources: false,
        ),
      );
      await editor.dispose();
      final kept = keptSink.takeBytes();
      // The unused image alone is 16 KiB of bytes that do not deflate.
      expect(pruned.length, lessThan(kept.length - 16 * 1024));

      final before = await pdf.open(src(unusedResourcesPdf));
      final after = await pdf.open(src(pruned));
      final text = await extractTextToString(
        after,
        pages: const PdfPages.all(),
      );
      expect(
        text,
        await extractTextToString(before, pages: const PdfPages.all()),
      );
      expect(text, contains(unusedResourcesMarker));
      final expected = await extractToMemory(
        before,
        pages: const PdfPages.all(),
      );
      final actual = await extractToMemory(after, pages: const PdfPages.all());
      expect(actual, hasLength(expected.length));
      expect(actual.single.image.width, expected.single.image.width);
      expect(actual.single.data, expected.single.data);
      await before.dispose();
      await after.dispose();
    }, timeout: t(2));

    test('compress shrinks a source that deflates', () async {
      final pdf = createPdf();
      final source = repeatedStreamPdf(5000);
      final sink = TestSink();
      await pdf.compress(src(source), sink, images: PdfImagePolicy.lossless);
      expect(sink.takeBytes().length, lessThan(source.length ~/ 4));
    }, timeout: t(1));

    test('compress of a small source never writes a larger file', () async {
      final pdf = createPdf();
      // Random bytes do not deflate and the rewrite saves nothing else, so
      // its result is not smaller: the output is the source, byte for byte.
      final source = randomStreamPdf(20000);
      expect(source.length, lessThanOrEqualTo(4 * 1024 * 1024));
      final sink = TestSink();
      await pdf.compress(src(source), sink, images: PdfImagePolicy.lossless);
      expect(sink.takeBytes(), source);
    }, timeout: t(1));

    test(
      'compress of a source over 4 MiB never writes a larger file',
      () async {
        final pdf = createPdf();
        final source = randomStreamPdf(4 * 1024 * 1024 + 4096);
        expect(source.length, greaterThan(4 * 1024 * 1024));
        final sink = TestSink();
        await pdf.compress(src(source), sink, images: PdfImagePolicy.lossless);
        expect(sink.takeBytes(), source);
      },
      timeout: t(3),
    );

    test('compress of a source over 4 MiB that deflates writes the file it '
        'counted', () async {
      final pdf = createPdf();
      final source = repeatedStreamPdf(160000);
      expect(source.length, greaterThan(4 * 1024 * 1024));
      final sink = TestSink();
      await pdf.compress(src(source), sink, images: PdfImagePolicy.lossless);
      final output = sink.takeBytes();
      expect(output.length, lessThan(source.length ~/ 4));

      // The counting save and the writing save are two saves of one
      // editor: the bytes counted are the bytes a fresh editor saves.
      final editor = await pdf.edit(src(source));
      final fresh = TestSink();
      await editor.save(fresh, options: const PdfSaveOptions.fullRewrite());
      await editor.dispose();
      expect(output, fresh.takeBytes());
    }, timeout: t(3));

    test('compress reads its source in few, whole chunks', () async {
      final pdf = createPdf();
      final source = fThousandPage;
      var reads = 0;
      await pdf.compress(
        TestSource(source, onRead: (_, _) => reads++),
        TestSink(),
      );
      // Every read is a round trip to the host: a thread wake-up on native,
      // a suspension or an Atomics.wait on the web. This 1000-page file is
      // 10 chunks and reads 163 times; a reader that asks for the size the
      // engine wants, not a whole chunk, reads it 383 times.
      final chunks = (source.length / maxReadChunk).ceil();
      expect(
        reads,
        lessThanOrEqualTo(chunks * 20),
        reason: 'source is $chunks chunks',
      );
    }, timeout: t(3));

    test('compress with an image policy shrinks a photo PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.compress(
        src(fImagesHires),
        sink,
        images: PdfImagePolicy.screen,
      );
      final output = sink.takeBytes();
      expect(output.length, lessThan(fImagesHires.length ~/ 4));
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      final images = await extractedImages(
        doc,
        pages: const PdfPages.single(0),
      );
      expect(images.single.width, 32);
      expect(images.single.height, 32);
      await doc.dispose();
    }, timeout: t(1));

    // ── Delete ──

    test('deletePages from 2-page → 1 page', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();
      final deleteSink = TestSink();
      await pdf
          .edits(src(twoPage), deleteSink)
          .deletePages(const PdfPages.single(0));
      final doc = await pdf.open(src(deleteSink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    // ── Reorder ──

    test('keepPages preserves pages', () async {
      final pdf = createPdf();
      final multiPage = fThreePageMarkers;
      final sink = TestSink();
      await pdf.edits(src(multiPage), sink).keepPages([2, 1, 0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
      await doc.dispose();
    }, timeout: t(1));

    // ── Move ──

    test('movePage changes page position', () async {
      final pdf = createPdf();
      final s1 = TestSink();
      await pdf.merge([src(minimalPdf), src(letterPdf)], s1);
      final s2 = TestSink();
      await pdf.merge([src(s1.takeBytes()), src(minimalPdf)], s2);
      final threePages = s2.takeBytes();

      final before = await pdf.open(src(threePages));
      final letterWidth = (await before.pageInfo())[1].width;
      await before.dispose();

      final sink = TestSink();
      await pdf.edits(src(threePages), sink).movePage(from: 1, to: 0);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
      expect((await doc.pageInfo())[0].width, closeTo(letterWidth, 1.0));
      await doc.dispose();
    }, timeout: t(1));

    // ── Split ──

    test(
      'split 3-page into 1-page chunks, each with its own content',
      () async {
        final pdf = createPdf();
        final multiPage = fThreePageMarkers;
        final sinks = <TestSink>[];
        final parts = await pdf.split(src(multiPage), (part) {
          final s = TestSink();
          sinks.add(s);
          return s;
        }, by: const PdfSplitRule.every(1));
        expect(sinks.length, 3);
        expect(parts, [
          const PdfSplitPart(index: 0, start: 0, end: 0),
          const PdfSplitPart(index: 1, start: 1, end: 1),
          const PdfSplitPart(index: 2, start: 2, end: 2),
        ]);
        for (var i = 0; i < sinks.length; i++) {
          final doc = await pdf.open(src(sinks[i].takeBytes()));
          expect(doc.pageCount, 1);
          final text = await extractTextToString(
            doc,
            pages: const PdfPages.all(),
          );
          expect(
            text,
            contains(fThreePageMarkersTruth.markers[i]),
            reason:
                'chunk $i must hold page $i\'s content — right '
                'counts with shuffled content is still corruption',
          );
          await doc.dispose();
        }
      },
      timeout: t(1),
    );

    test('split by maxBytes: total pages equals original', () async {
      final pdf = createPdf();
      final multiPage = fThreePageMarkers;
      final sinks = <TestSink>[];
      final parts = await pdf.split(src(multiPage), (part) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, by: const PdfSplitRule.maxBytes(50000));
      expect(parts.length, greaterThanOrEqualTo(1));
      expect(sinks.length, parts.length);
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        totalPages += doc.pageCount;
        await doc.dispose();
      }
      expect(totalPages, 3);
    }, timeout: t(1));

    test('split by maxBytes cuts a part exactly where the next page would '
        'not fit', () async {
      final pdf = createPdf();
      final multiPage = fThreePageMarkers;
      // The size of pages 0 and 1 as a file of their own: the limit that
      // admits those two and not the third.
      final measured = <TestSink>[];
      await pdf.split(src(multiPage), (part) {
        final s = TestSink();
        measured.add(s);
        return s;
      }, by: const PdfSplitRule.every(2));
      final twoPages = measured.first.takeBytes().length;

      final sinks = <TestSink>[];
      final parts = await pdf.split(src(multiPage), (part) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, by: PdfSplitRule.maxBytes(twoPages));
      expect(parts, [
        const PdfSplitPart(index: 0, start: 0, end: 1),
        const PdfSplitPart(index: 1, start: 2, end: 2),
      ]);
      // The engine's count is the file's size: a part is never larger than
      // the limit.
      expect(sinks[0].takeBytes().length, lessThanOrEqualTo(twoPages));
    }, timeout: t(1));

    test('split by maxBytes makes a page that exceeds the limit a part of '
        'its own', () async {
      final pdf = createPdf();
      final sinks = <TestSink>[];
      final parts = await pdf.split(src(fThreePageMarkers), (part) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, by: const PdfSplitRule.maxBytes(1));
      expect(parts, [
        const PdfSplitPart(index: 0, start: 0, end: 0),
        const PdfSplitPart(index: 1, start: 1, end: 1),
        const PdfSplitPart(index: 2, start: 2, end: 2),
      ]);
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        expect(doc.pageCount, 1);
        await doc.dispose();
      }
    }, timeout: t(1));

    test('split asks for each part\'s sink before it is written, in order, '
        'and closes it', () async {
      final pdf = createPdf();
      final asked = <PdfSplitPart>[];
      final sinks = <CountingSink>[];
      await pdf.split(src(fThreePageMarkers), (part) async {
        asked.add(part);
        // The part before this one is written and closed by now.
        if (sinks.isNotEmpty) expect(sinks.last.closes, 1);
        final s = CountingSink();
        sinks.add(s);
        return s;
      }, by: const PdfSplitRule.every(1));
      expect(asked.map((p) => p.index), [0, 1, 2]);
      for (final s in sinks) {
        expect(s.closes, 1);
        expect(s.aborts, 0);
      }
    }, timeout: t(1));

    test('split rejects a rule with no pages or bytes in it', () {
      final pdf = createPdf();
      expect(
        () => pdf.split(
          src(minimalPdf),
          (part) => TestSink(),
          by: const PdfSplitRule.every(0),
        ),
        throwsA(isA<PdfInvalidArgument>()),
      );
      expect(
        () => pdf.split(
          src(minimalPdf),
          (part) => TestSink(),
          by: const PdfSplitRule.maxBytes(0),
        ),
        throwsA(isA<PdfInvalidArgument>()),
      );
    });

    // ── Split by bookmarks ──

    test('split by bookmarks: one part per chapter, right content', () async {
      // A REAL outline tree from the foreign producer — each chunk
      // must start at its chapter and carry that chapter's text.
      final pdf = createPdf();
      final sinks = <TestSink>[];
      final parts = await pdf.split(src(fOutlineChapters), (part) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, by: const PdfSplitRule.bookmarks());
      expect(sinks.length, fOutlineChaptersTruth.chapters.length);
      expect(parts.map((p) => p.title), fOutlineChaptersTruth.chapters);
      expect(parts.map((p) => p.start), fOutlineChaptersTruth.startPages);
      expect(parts.map((p) => p.end - p.start), everyElement(1));
      for (var i = 0; i < sinks.length; i++) {
        final doc = await pdf.open(src(sinks[i].takeBytes()));
        expect(
          doc.pageCount,
          2,
          reason: 'every chapter spans exactly two pages',
        );
        final text = await extractTextToString(
          doc,
          pages: const PdfPages.all(),
        );
        expect(
          text,
          contains(fOutlineChaptersTruth.chapters[i]),
          reason:
              'chunk $i must hold its own chapter, not a '
              'neighbor\'s',
        );
        await doc.dispose();
      }
    }, timeout: t(1));

    // ── Redaction ──

    test('applyRedactions preserves page count', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.rewrite(src(fTwoPageMarkers), sink, (e) async {
        await e.applyRedactions();
      });
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, fTwoPageMarkersTruth.pages);
      await doc.dispose();
    }, timeout: t(1));

    // ── Embed file ──

    test('embedFile increases output size', () async {
      final pdf = createPdf();
      final fileContent = Uint8List.fromList('hello embedded file'.codeUnits);
      final sink = TestSink();
      await pdf
          .edits(src(minimalPdf), sink)
          .embedFile('test.txt', src(fileContent));
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
      expect(
        output.length,
        greaterThan(minimalPdf.length),
        reason:
            'the embedded payload must be in there somewhere; '
            'a semantic presence proof needs an attachment-listing '
            'API (tracked in CAPABILITY_ROADMAP)',
      );
    }, timeout: t(1));

    // ── Erase ──

    test('redact with no overlay produces valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.edits(src(minimalPdf), sink).redact(0, [
        const PdfRect(x: 0, y: 0, width: 100, height: 100),
      ], overlay: const PdfRedactionOverlay.none());
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      // No size assertion: erase REMOVES content, so the output may
      // legitimately be smaller than the input. The destructive
      // contract itself is pinned by the full-page erase test below.
      await doc.dispose();
    }, timeout: t(1));

    // ── Stamps ──

    test('addStamp produces valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf
          .edits(src(minimalPdf), sink)
          .addStamp(
            const PdfPages.single(0),
            type: PdfStampType.approved,
            rect: const PdfRect(x: 100, y: 100, width: 200, height: 50),
          );
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
      // Semantic presence: flatten the stamp annotation into page
      // content, then the stamp's appearance text must be extractable.
      final flatSink = TestSink();
      await pdf.edits(src(output), flatSink).flattenAnnotations();
      final flat = await pdf.open(src(flatSink.takeBytes()));
      final text = await extractTextToString(flat, pages: const PdfPages.all());
      expect(
        text.toUpperCase(),
        contains('APPROVED'),
        reason:
            'a stamp whose appearance survives flattening is '
            'provably on the page',
      );
      await flat.dispose();
    }, timeout: t(1));

    test('addImageStamp embeds image data', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf
          .edits(src(minimalPdf), sink)
          .addImageStamp(
            const PdfPages.single(0),
            src(minimalPng),
            rect: const PdfRect(x: 100, y: 100, width: 200, height: 200),
          );
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
      expect(output.length, greaterThan(minimalPdf.length));
    }, timeout: t(1));

    // ── Watermark ──

    test('background watermark text is extractable on every page', () async {
      // The background layer draws INTO the content stream — its text
      // must come back out of extraction (the semantic presence proof;
      // the annotation layer's proof rides flattenAnnotations in
      // the stamp test above).
      final pdf = createPdf();
      final sink = TestSink();
      await pdf
          .edits(src(fTwoPageMarkers), sink)
          .addWatermark('CONFIDENTIAL', layer: PdfWatermarkLayer.background);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, fTwoPageMarkersTruth.pages);
      for (var p = 0; p < fTwoPageMarkersTruth.pages; p++) {
        final text = await extractTextToString(doc, pages: PdfPages.single(p));
        expect(
          text,
          contains('CONFIDENTIAL'),
          reason: 'page $p must carry the watermark',
        );
      }
      await doc.dispose();
    }, timeout: t(1));

    test('watermark position variants produce valid output', () async {
      final pdf = createPdf();
      final input = fTwoPageMarkers;
      final positions = <String, PdfWatermarkPosition>{
        'center': const PdfWatermarkPosition.center(),
        'corner': const PdfWatermarkPosition.corner(PdfCorner.topRight),
        'tiled': const PdfWatermarkPosition.tiled(columns: 2, rows: 2),
        'exact': const PdfWatermarkPosition.exact(
          x: 50,
          y: 50,
          width: 200,
          height: 100,
        ),
      };
      for (final entry in positions.entries) {
        final sink = TestSink();
        await pdf
            .edits(src(input), sink)
            .addWatermark('POS', position: entry.value);
        final output = sink.takeBytes();
        final doc = await pdf.open(src(output));
        expect(doc.pageCount, 2, reason: '${entry.key}: page count preserved');
        await doc.dispose();
        // The foreground layer is an annotation: flatten it into the page
        // content, then its text must extract beside the original text.
        final flatSink = TestSink();
        await pdf.edits(src(output), flatSink).flattenAnnotations();
        final flat = await pdf.open(src(flatSink.takeBytes()));
        final text = await extractTextToString(
          flat,
          pages: const PdfPages.all(),
        );
        expect(
          text,
          contains('POS'),
          reason: '${entry.key}: the watermark is on the page',
        );
        expect(
          text,
          contains('ALPHA UNIQUE'),
          reason: '${entry.key}: the page content is kept',
        );
        await flat.dispose();
      }
    }, timeout: t(1));

    test('every watermark layer preserves the original content', () async {
      final pdf = createPdf();
      final input = fTwoPageMarkers;
      for (final layer in PdfWatermarkLayer.values) {
        final sink = TestSink();
        await pdf.edits(src(input), sink).addWatermark('LAYER', layer: layer);
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(
          doc.pageCount,
          fTwoPageMarkersTruth.pages,
          reason: '${layer.name}: page count preserved',
        );
        final text = await extractTextToString(
          doc,
          pages: const PdfPages.all(),
        );
        expect(
          text,
          contains(fTwoPageMarkersTruth.page0Marker),
          reason: '${layer.name}: original content preserved',
        );
        await doc.dispose();
      }
    }, timeout: t(1));

    // ── Encrypt / Decrypt ──

    test('encrypt then decrypt roundtrip', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.resave(
        src(minimalPdf),
        encSink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.config(
            ownerPassword: 'owner',
            userPassword: 'user',
          ),
        ),
      );
      final encrypted = encSink.takeBytes();
      expect(encrypted.length, greaterThan(0));
      final encDoc = await pdf.open(src(encrypted), password: 'owner');
      expect(encDoc.encryption, isNotNull);
      await encDoc.dispose();
      final decSink = TestSink();
      await pdf.resave(
        src(encrypted),
        decSink,
        password: 'owner',
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.remove(),
        ),
      );
      final decrypted = decSink.takeBytes();
      final decDoc = await pdf.open(src(decrypted));
      expect(decDoc.pageCount, 1);
      await decDoc.dispose();
    }, timeout: t(1));

    // ── ConvertToPdfA ──

    test('convertToPdfA produces valid PDF', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final sink = TestSink();
      await pdf.edits(src(formBytes), sink).convertToPdfA();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    // ── ImagesToPdf ──

    test('imagesToPdf creates valid PDF from images', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.imagesToPdf([src(minimalPng)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('imagesToPdf gives every page the size it is told', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.imagesToPdf(
        [src(minimalPng), src(minimalPng)],
        sink,
        page: const PdfImagePageOptions(size: PdfPageSize.custom(200, 300)),
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(200, 2));
      expect(pages[0].height, closeTo(300, 2));
      await doc.dispose();
    }, timeout: t(1));

    // ── Content integrity — the bytes must mean what they meant ──
    //
    // Structural checks (pageCount) cannot see content corruption.
    // These use a fixture with distinct per-page markers (ALPHA on
    // page one, BRAVO on page two) and prove the CONTENT moved,
    // survived, or died exactly as the op promised.

    test('keepPages actually reorders content', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.edits(src(twoPage), sink).keepPages([1, 0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      final first = await extractTextToString(
        doc,
        pages: const PdfPages.single(0),
      );
      final second = await extractTextToString(
        doc,
        pages: const PdfPages.single(1),
      );
      expect(
        first,
        contains(fTwoPageMarkersTruth.page1Marker),
        reason: 'page two must now be first',
      );
      expect(first, isNot(contains(fTwoPageMarkersTruth.page0Marker)));
      expect(second, contains(fTwoPageMarkersTruth.page0Marker));
      await doc.dispose();
    }, timeout: t(1));

    test('movePage actually moves content', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.edits(src(twoPage), sink).movePage(from: 0, to: 1);
      final doc = await pdf.open(src(sink.takeBytes()));
      final first = await extractTextToString(
        doc,
        pages: const PdfPages.single(0),
      );
      expect(first, contains(fTwoPageMarkersTruth.page1Marker));
      expect(first, isNot(contains(fTwoPageMarkersTruth.page0Marker)));
      await doc.dispose();
    }, timeout: t(1));

    test('deletePages removes exactly the deleted content', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.edits(src(twoPage), sink).deletePages(const PdfPages.single(0));
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(
        text,
        isNot(contains(fTwoPageMarkersTruth.page0Marker)),
        reason: 'the deleted page content must be gone',
      );
      expect(
        text,
        contains(fTwoPageMarkersTruth.page1Marker),
        reason: 'the kept page content must survive',
      );
      await doc.dispose();
    }, timeout: t(1));

    test(
      'deletePages writes nothing only a deleted page reaches (#261)',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf
            .edits(src(pagePrunePdf), sink)
            .deletePages(const PdfPages.list([0, 1, 2, 4, 5, 7]));
        await expectOnlyPrunedPages(pdf, sink.takeBytes(), const [3, 6]);
      },
      timeout: t(2),
    );

    test('watermark preserves the existing page text', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.edits(src(twoPage), sink).addWatermark('OVERLAY');
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains(fTwoPageMarkersTruth.page0Marker));
      expect(text, contains(fTwoPageMarkersTruth.page1Marker));
      await doc.dispose();
    }, timeout: t(1));

    test('compress preserves the existing page text', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.compress(src(twoPage), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains(fTwoPageMarkersTruth.page0Marker));
      expect(text, contains(fTwoPageMarkersTruth.page1Marker));
      await doc.dispose();
    }, timeout: t(1));

    test('encryption hides the plaintext from the raw bytes', () async {
      // Needs the UNCOMPRESSED handwritten probe: only an input whose
      // bytes provably contain the plaintext can prove the ciphertext
      // does not. (Compressed fixtures hide it before encryption ever
      // runs — the check would pass vacuously.)
      final pdf = createPdf();
      expect(
        String.fromCharCodes(leakProbePdf), // bytegrep-exempt
        contains(leakProbeMarker),
        reason:
            'byte-level sanity: the plaintext must provably be '
            'in the input before its absence in the output means '
            'anything',
      );
      final sink = TestSink();
      await pdf.resave(
        src(leakProbePdf),
        sink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.config(
            ownerPassword: 'owner-pw',
            userPassword: 'user-pw',
          ),
        ),
      );
      final enc = sink.takeBytes();
      expect(
        String.fromCharCodes(enc), // bytegrep-exempt: leak negative
        isNot(contains(leakProbeMarker)),
        reason:
            'encrypted output leaking plaintext is not '
            'encryption',
      );
      // And the content survives the round trip.
      final doc = await pdf.open(src(enc), password: 'user-pw');
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains(leakProbeMarker));
      await doc.dispose();
    }, timeout: t(1));

    test(
      'redact with no overlay over the full page destroys its text',
      () async {
        final pdf = createPdf();
        final twoPage = fTwoPageMarkers;
        final sink = TestSink();
        await pdf.edits(src(twoPage), sink).redact(0, [
          const PdfRect(x: 0, y: 0, width: 612, height: 792),
        ], overlay: const PdfRedactionOverlay.none());
        final doc = await pdf.open(src(sink.takeBytes()));
        final first = await extractTextToString(
          doc,
          pages: const PdfPages.single(0),
        );
        expect(
          first,
          isNot(contains(fTwoPageMarkersTruth.page0Marker)),
          reason:
              'erase promises the content is GONE, not painted '
              'over',
        );
        final second = await extractTextToString(
          doc,
          pages: const PdfPages.single(1),
        );
        expect(
          second,
          contains(fTwoPageMarkersTruth.page1Marker),
          reason: 'the untouched page must keep its content',
        );
        await doc.dispose();
      },
      timeout: t(1),
    );
  });
}
