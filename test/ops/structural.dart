// Structural — rotate, flatten, compress, delete, extract, reorder pages.
// Every test verifies actual output correctness via re-open + inspect.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/generators.dart';
import '../helpers/test_source_sink.dart';

void registerStructuralTests(Pdf Function() createPdf) {
  group('structural', () {
    // ── Rotate ──

    test('rotateAllPages 90° sets rotation on every page', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.rotateAllPages(src(minimalPdf), sink, degrees: 90);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
    });

    test('rotatePages rotates specific pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.rotatePages(src(minimalPdf), sink, pages: {0: 90});
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
    });

    // ── Flatten ──

    test('flattenForms on form PDF produces valid PDF with same page count',
        () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final sink = TestSink();
      await pdf.flattenForms(src(formBytes), sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1, reason: 'page count must be preserved');
      // Flattened output should still be a valid PDF.
      expect(output.length, greaterThan(formBytes.length ~/ 2),
          reason: 'output should not be trivially small');
    });

    // ── Compress ──

    test('compress produces valid PDF reopenable with same page count',
        () async {
      final pdf = createPdf();
      // Use a multi-page PDF so compression has material to work with.
      final multiPage = await _buildThreePagePdf(pdf);
      final sink = TestSink();
      await pdf.compress(src(multiPage), sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 3, reason: 'page count must be preserved');
      // Compressed output should be <= original (or at minimum a valid PDF).
      expect(output.length, lessThanOrEqualTo(multiPage.length * 2),
          reason: 'compressed output should not explode in size');
    });

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
    });

    // ── Extract pages ──

    test('extractPages keeps selected pages', () async {
      final pdf = createPdf();
      final multiPage = await _buildThreePagePdf(pdf);
      final sink = TestSink();
      await pdf.extractPages(src(multiPage), sink, pages: [0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    });

    // ── Reorder ──

    test('reorderPages preserves pages and produces valid PDF', () async {
      final pdf = createPdf();
      final multiPage = await _buildThreePagePdf(pdf);
      final sink = TestSink();
      await pdf.reorderPages(src(multiPage), sink, order: [2, 1, 0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3, reason: 'page count must be preserved');
    });

    // ── Move ──

    test('movePage changes page position verified by dimensions', () async {
      final pdf = createPdf();
      // Build 3-page: A4, Letter, A4.
      final s1 = TestSink();
      await pdf.merge([src(minimalPdf), src(letterPdf)], s1);
      final s2 = TestSink();
      await pdf.merge([src(s1.takeBytes()), src(minimalPdf)], s2);
      final threePages = s2.takeBytes();

      // Before: page 0=A4, page 1=Letter, page 2=A4.
      final before = await pdf.open(src(threePages));
      final letterWidth = before.pages[1].width; // ~612

      // Move page 1 (Letter) to position 0.
      final sink = TestSink();
      await pdf.movePage(src(threePages), sink, from: 1, to: 0);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
      expect(doc.pages[0].width, closeTo(letterWidth, 1.0),
          reason: 'Letter page should now be at position 0');
    });

    // ── Split ──

    test('split 3-page into 1-page chunks', () async {
      final pdf = createPdf();
      final multiPage = await _buildThreePagePdf(pdf);
      final sinks = <TestSink>[];
      await pdf.split(src(multiPage), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, every: 1);
      expect(sinks.length, 3);
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        expect(doc.pageCount, 1);
      }
    });

    test('splitBySize total pages equals original', () async {
      final pdf = createPdf();
      final multiPage = await _buildThreePagePdf(pdf);
      final sinks = <TestSink>[];
      final chunkSizes = await pdf.splitBySize(src(multiPage), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: 50000);
      expect(chunkSizes.length, greaterThanOrEqualTo(1));
      expect(sinks.length, chunkSizes.length);

      // Total pages across all chunks must equal original.
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        totalPages += doc.pageCount;
      }
      expect(totalPages, 3, reason: 'total pages across chunks must match');
    });

    // ── Split by bookmarks ──

    test('splitByBookmarks produces one chunk per chapter', () async {
      final pdf = createPdf();
      final sinks = <TestSink>[];
      await pdf.splitByBookmarks(src(bookmarkedPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      });
      // bookmarkedPdf has 2 chapters, so expect 2 chunks.
      expect(sinks.length, 2);
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        expect(doc.pageCount, 1,
            reason: 'each chapter should be exactly 1 page');
      }
    });

    test('planSplitByBookmarks returns chapter titles', () async {
      final pdf = createPdf();
      final splits = await pdf.planSplitByBookmarks(src(bookmarkedPdf));
      expect(splits.length, 2);
      expect(splits[0].title, contains('Chapter 1'));
      expect(splits[1].title, contains('Chapter 2'));
      expect(splits[0].startPage, greaterThanOrEqualTo(0));
    });

    // ── Redaction ──

    test('applyRedactions produces valid PDF with same page count', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.applyRedactions(src(bookmarkedPdf), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2, reason: 'page count must be preserved');
    });

    // ── Embed file ──

    test('embedFile increases output size and produces valid PDF', () async {
      final pdf = createPdf();
      final fileContent = Uint8List.fromList('hello embedded file'.codeUnits);
      final sink = TestSink();
      await pdf.embedFile(src(minimalPdf), sink,
          name: 'test.txt', fileData: src(fileContent));
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length),
          reason: 'embedded file must increase output size');
      // The embedded file name should appear in output bytes.
      expect(String.fromCharCodes(output), contains('test.txt'));
    });

    // ── Erase ──

    test('eraseRegions produces valid PDF with overlay content', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.eraseRegions(src(minimalPdf), sink,
          page: 0,
          regions: [const PdfRect(x: 0, y: 0, width: 100, height: 100)]);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      // Erase overlay adds a content stream → output should be larger.
      expect(output.length, greaterThan(minimalPdf.length),
          reason: 'erase overlay must increase output size');
    });

    // ── Stamps ──

    test('addStamp produces valid PDF with stamp annotation', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.addStamp(src(minimalPdf), sink,
          page: 0,
          type: PdfStampType.approved,
          rect: const PdfRect(x: 100, y: 100, width: 200, height: 50));
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length),
          reason: 'stamp annotation must increase output size');
      // Stamp type name should appear in output bytes.
      expect(String.fromCharCodes(output).toLowerCase(), contains('approved'));
    });

    test('addImageStamp embeds image data and produces valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.addImageStamp(src(minimalPdf), sink,
          page: 0,
          imageData: src(minimalPng),
          rect: const PdfRect(x: 100, y: 100, width: 200, height: 200));
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length),
          reason: 'image stamp must increase output size');
    });
  });
}

Future<Uint8List> _buildThreePagePdf(Pdf pdf) async {
  final s1 = TestSink();
  await pdf.merge([src(minimalPdf), src(minimalPdf)], s1);
  final s2 = TestSink();
  await pdf.merge([src(s1.takeBytes()), src(minimalPdf)], s2);
  return s2.takeBytes();
}
