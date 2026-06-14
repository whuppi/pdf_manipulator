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

    // ── Rotate ──

    test('rotateAllPages 90° sets rotation', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.rotateAllPages(src(minimalPdf), sink, degrees: 90);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
      await doc.dispose();
    }, timeout: t(1));

    test('rotatePages rotates specific pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.rotatePages(src(minimalPdf), sink, pages: {0: 90});
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
      await doc.dispose();
    }, timeout: t(1));

    // ── Flatten ──

    test('flattenForms renders the untouched default value', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final sink = TestSink();
      await pdf.flattenForms(src(formBytes), sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, fFormFieldsTruth.pages);
      final text = await doc.extract(pages: const PdfPages.all());
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

    // ── Delete ──

    test('deletePages from 2-page → 1 page', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();
      final deleteSink = TestSink();
      await pdf.deletePages(src(twoPage), deleteSink, pages: [0]);
      final doc = await pdf.open(src(deleteSink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    // ── Reorder ──

    test('reorderPages preserves pages', () async {
      final pdf = createPdf();
      final multiPage = fThreePageMarkers;
      final sink = TestSink();
      await pdf.reorderPages(src(multiPage), sink, order: [2, 1, 0]);
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
      final letterWidth = before.pages[1].width;
      await before.dispose();

      final sink = TestSink();
      await pdf.movePage(src(threePages), sink, from: 1, to: 0);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
      expect(doc.pages[0].width, closeTo(letterWidth, 1.0));
      await doc.dispose();
    }, timeout: t(1));

    // ── Split ──

    test(
      'split 3-page into 1-page chunks, each with its own content',
      () async {
        final pdf = createPdf();
        final multiPage = fThreePageMarkers;
        final sinks = <TestSink>[];
        await pdf.split(src(multiPage), (i) {
          final s = TestSink();
          sinks.add(s);
          return s;
        }, every: 1);
        expect(sinks.length, 3);
        for (var i = 0; i < sinks.length; i++) {
          final doc = await pdf.open(src(sinks[i].takeBytes()));
          expect(doc.pageCount, 1);
          final text = await doc.extract(pages: const PdfPages.all());
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

    test('splitBySize total pages equals original', () async {
      final pdf = createPdf();
      final multiPage = fThreePageMarkers;
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(multiPage), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: 50000);
      expect(chunkSizes.length, greaterThanOrEqualTo(1));
      expect(sinks.length, chunkSizes.length);
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        totalPages += doc.pageCount;
        await doc.dispose();
      }
      expect(totalPages, 3);
    }, timeout: t(1));

    // ── Split by bookmarks ──

    test('splitByBookmarks: one chunk per chapter, right content', () async {
      // A REAL outline tree from the foreign producer — each chunk
      // must start at its chapter and carry that chapter's text.
      final pdf = createPdf();
      final sinks = <TestSink>[];
      await pdf.splitByBookmarks(src(fOutlineChapters), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      });
      expect(sinks.length, fOutlineChaptersTruth.chapters.length);
      for (var i = 0; i < sinks.length; i++) {
        final doc = await pdf.open(src(sinks[i].takeBytes()));
        expect(
          doc.pageCount,
          2,
          reason: 'every chapter spans exactly two pages',
        );
        final text = await doc.extract(pages: const PdfPages.all());
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
      await pdf.applyRedactions(src(fTwoPageMarkers), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, fTwoPageMarkersTruth.pages);
      await doc.dispose();
    }, timeout: t(1));

    // ── Embed file ──

    test('embedFile increases output size', () async {
      final pdf = createPdf();
      final fileContent = Uint8List.fromList('hello embedded file'.codeUnits);
      final sink = TestSink();
      await pdf.embedFile(
        src(minimalPdf),
        sink,
        name: 'test.txt',
        fileData: src(fileContent),
      );
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

    test('eraseRegions produces valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.eraseRegions(
        src(minimalPdf),
        sink,
        page: 0,
        regions: [const PdfRect(x: 0, y: 0, width: 100, height: 100)],
      );
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
      await pdf.addStamp(
        src(minimalPdf),
        sink,
        page: 0,
        type: PdfStampType.approved,
        rect: const PdfRect(x: 100, y: 100, width: 200, height: 50),
      );
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
      // Semantic presence: flatten the stamp annotation into page
      // content, then the stamp's appearance text must be extractable.
      final e = await pdf.edit(src(output));
      await e.flattenAllAnnotations();
      final flatSink = TestSink();
      await e.save(flatSink);
      await e.dispose();
      final flat = await pdf.open(src(flatSink.takeBytes()));
      final text = await flat.extract(pages: const PdfPages.all());
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
      await pdf.addImageStamp(
        src(minimalPdf),
        sink,
        page: 0,
        imageData: src(minimalPng),
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
      // the annotation layer's proof rides flattenAllAnnotations in
      // the stamp test above).
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.watermark(
        src(fTwoPageMarkers),
        sink,
        text: 'CONFIDENTIAL',
        layer: PdfWatermarkLayer.background,
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, fTwoPageMarkersTruth.pages);
      for (var p = 0; p < fTwoPageMarkersTruth.pages; p++) {
        final text = await doc.extract(pages: PdfPages.single(p));
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
        await pdf.watermark(
          src(input),
          sink,
          text: 'POS',
          position: entry.value,
        );
        final output = sink.takeBytes();
        final doc = await pdf.open(src(output));
        expect(doc.pageCount, 2, reason: '${entry.key}: page count preserved');
        await doc.dispose();
        expect(
          output.length,
          greaterThan(input.length),
          reason: '${entry.key}: watermark increases file size',
        );
      }
    }, timeout: t(1));

    test('every watermark layer preserves the original content', () async {
      final pdf = createPdf();
      final input = fTwoPageMarkers;
      for (final layer in PdfWatermarkLayer.values) {
        final sink = TestSink();
        await pdf.watermark(src(input), sink, text: 'LAYER', layer: layer);
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(
          doc.pageCount,
          fTwoPageMarkersTruth.pages,
          reason: '${layer.name}: page count preserved',
        );
        final text = await doc.extract(pages: const PdfPages.all());
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
      await pdf.encrypt(
        src(minimalPdf),
        encSink,
        encryption: const PdfEncryptionConfig(
          ownerPassword: 'owner',
          userPassword: 'user',
        ),
      );
      final encrypted = encSink.takeBytes();
      expect(encrypted.length, greaterThan(0));
      final encDoc = await pdf.open(src(encrypted), password: 'owner');
      expect(encDoc.isEncrypted, isTrue);
      await encDoc.dispose();
      final decSink = TestSink();
      await pdf.decrypt(src(encrypted), decSink, password: 'owner');
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
      await pdf.convertToPdfA(src(formBytes), sink);
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

    // ── Content integrity — the bytes must mean what they meant ──
    //
    // Structural checks (pageCount) cannot see content corruption.
    // These use a fixture with distinct per-page markers (ALPHA on
    // page one, BRAVO on page two) and prove the CONTENT moved,
    // survived, or died exactly as the op promised.

    test('reorderPages actually reorders content', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.reorderPages(src(twoPage), sink, order: [1, 0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      final first = await doc.extract(pages: const PdfPages.single(0));
      final second = await doc.extract(pages: const PdfPages.single(1));
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
      await pdf.movePage(src(twoPage), sink, from: 0, to: 1);
      final doc = await pdf.open(src(sink.takeBytes()));
      final first = await doc.extract(pages: const PdfPages.single(0));
      expect(first, contains(fTwoPageMarkersTruth.page1Marker));
      expect(first, isNot(contains(fTwoPageMarkersTruth.page0Marker)));
      await doc.dispose();
    }, timeout: t(1));

    test('deletePages removes exactly the deleted content', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.deletePages(src(twoPage), sink, pages: [0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
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

    test('watermark preserves the existing page text', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.watermark(src(twoPage), sink, text: 'OVERLAY');
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
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
      final text = await doc.extract(pages: const PdfPages.all());
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
      await pdf.encrypt(
        src(leakProbePdf),
        sink,
        encryption: const PdfEncryptionConfig(
          ownerPassword: 'owner-pw',
          userPassword: 'user-pw',
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
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains(leakProbeMarker));
      await doc.dispose();
    }, timeout: t(1));

    test('eraseRegions over the full page destroys its text', () async {
      final pdf = createPdf();
      final twoPage = fTwoPageMarkers;
      final sink = TestSink();
      await pdf.eraseRegions(
        src(twoPage),
        sink,
        page: 0,
        regions: [const PdfRect(x: 0, y: 0, width: 612, height: 792)],
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      final first = await doc.extract(pages: const PdfPages.single(0));
      expect(
        first,
        isNot(contains(fTwoPageMarkersTruth.page0Marker)),
        reason:
            'erase promises the content is GONE, not painted '
            'over',
      );
      final second = await doc.extract(pages: const PdfPages.single(1));
      expect(
        second,
        contains(fTwoPageMarkersTruth.page1Marker),
        reason: 'the untouched page must keep its content',
      );
      await doc.dispose();
    }, timeout: t(1));
  });
}
