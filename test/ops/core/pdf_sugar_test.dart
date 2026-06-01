// PdfSugar — convenience wrappers over editor/builder/standalone.
// Mirrors lib/src/ops/pdf_sugar.dart.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/fixtures.dart';
import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';

void registerSugarTests(Pdf Function() createPdf) {
  group('sugar', () {
    // ── Merge ──

    test('merge two PDFs → 2 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
    }, timeout: Timeout(Duration(seconds: 2)));

    test('merge three PDFs → 3 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf), src(minimalPdf)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
    }, timeout: Timeout(Duration(seconds: 2)));

    test('merge different sized PDFs', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(letterPdf)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
    }, timeout: Timeout(Duration(seconds: 2)));

    test('merged output is valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], sink);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x25); // %PDF
      expect(bytes[1], 0x50);
      expect(bytes[2], 0x44);
      expect(bytes[3], 0x46);
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Rotate ──

    test('rotateAllPages 90° sets rotation', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.rotateAllPages(src(minimalPdf), sink, degrees: 90);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
    }, timeout: Timeout(Duration(seconds: 2)));

    test('rotatePages rotates specific pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.rotatePages(src(minimalPdf), sink, pages: {0: 90});
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Flatten ──

    test('flattenForms preserves page count', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final sink = TestSink();
      await pdf.flattenForms(src(formBytes), sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(formBytes.length ~/ 2));
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Compress ──

    test('compress preserves page count', () async {
      final pdf = createPdf();
      final multiPage = await _buildThreePagePdf(pdf);
      final sink = TestSink();
      await pdf.compress(src(multiPage), sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 3);
      expect(output.length, lessThanOrEqualTo(multiPage.length * 2));
    }, timeout: Timeout(Duration(seconds: 2)));

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
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Reorder ──

    test('reorderPages preserves pages', () async {
      final pdf = createPdf();
      final multiPage = await _buildThreePagePdf(pdf);
      final sink = TestSink();
      await pdf.reorderPages(src(multiPage), sink, order: [2, 1, 0]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
    }, timeout: Timeout(Duration(seconds: 2)));

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

      final sink = TestSink();
      await pdf.movePage(src(threePages), sink, from: 1, to: 0);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
      expect(doc.pages[0].width, closeTo(letterWidth, 1.0));
    }, timeout: Timeout(Duration(seconds: 2)));

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
    }, timeout: Timeout(Duration(seconds: 2)));

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
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        totalPages += doc.pageCount;
      }
      expect(totalPages, 3);
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Split by bookmarks ──

    test('splitByBookmarks produces one chunk per chapter', () async {
      final pdf = createPdf();
      final sinks = <TestSink>[];
      await pdf.splitByBookmarks(src(bookmarkedPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      });
      expect(sinks.length, 2);
      for (final s in sinks) {
        final doc = await pdf.open(src(s.takeBytes()));
        expect(doc.pageCount, 1);
      }
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Redaction ──

    test('applyRedactions preserves page count', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.applyRedactions(src(bookmarkedPdf), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Embed file ──

    test('embedFile increases output size', () async {
      final pdf = createPdf();
      final fileContent = Uint8List.fromList('hello embedded file'.codeUnits);
      final sink = TestSink();
      await pdf.embedFile(src(minimalPdf), sink,
          name: 'test.txt', fileData: src(fileContent));
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length));
      expect(String.fromCharCodes(output), contains('test.txt'));
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Erase ──

    test('eraseRegions produces valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.eraseRegions(src(minimalPdf), sink,
          page: 0, regions: [const PdfRect(x: 0, y: 0, width: 100, height: 100)]);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length));
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Stamps ──

    test('addStamp produces valid PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.addStamp(src(minimalPdf), sink,
          page: 0, type: PdfStampType.approved,
          rect: const PdfRect(x: 100, y: 100, width: 200, height: 50));
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length));
      expect(String.fromCharCodes(output).toLowerCase(), contains('approved'));
    }, timeout: Timeout(Duration(seconds: 2)));

    test('addImageStamp embeds image data', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.addImageStamp(src(minimalPdf), sink,
          page: 0, imageData: src(minimalPng),
          rect: const PdfRect(x: 100, y: 100, width: 200, height: 200));
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length));
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Watermark ──

    test('watermark embeds text and preserves page count', () async {
      final pdf = createPdf();
      final input = bookmarkedPdf;
      final sink = TestSink();
      await pdf.watermark(src(input), sink, text: 'CONFIDENTIAL');
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 2);
      await doc.dispose();
      expect(output.length, greaterThan(input.length));
      expect(String.fromCharCodes(output), contains('CONFIDENTIAL'));
    }, timeout: Timeout(Duration(seconds: 2)));

    test('watermark position variants produce valid output', () async {
      final pdf = createPdf();
      final input = bookmarkedPdf;
      final positions = <String, PdfWatermarkPosition>{
        'center': const PdfWatermarkPosition.center(),
        'corner': const PdfWatermarkPosition.corner(PdfCorner.topRight),
        'tiled': const PdfWatermarkPosition.tiled(columns: 2, rows: 2),
        'exact': const PdfWatermarkPosition.exact(x: 50, y: 50, width: 200, height: 100),
      };
      for (final entry in positions.entries) {
        final sink = TestSink();
        await pdf.watermark(src(input), sink, text: 'POS', position: entry.value);
        final output = sink.takeBytes();
        final doc = await pdf.open(src(output));
        expect(doc.pageCount, 2, reason: '${entry.key}: page count preserved');
        await doc.dispose();
        expect(output.length, greaterThan(input.length),
            reason: '${entry.key}: watermark increases file size');
      }
    }, timeout: Timeout(Duration(seconds: 2)));

    test('watermark layers produce valid output', () async {
      final pdf = createPdf();
      final input = bookmarkedPdf;
      for (final layer in PdfWatermarkLayer.values) {
        final sink = TestSink();
        await pdf.watermark(src(input), sink, text: 'LAYER', layer: layer);
        final output = sink.takeBytes();
        final doc = await pdf.open(src(output));
        expect(doc.pageCount, 2, reason: '${layer.name}: page count preserved');
        await doc.dispose();
        expect(output.length, greaterThan(input.length));
        expect(String.fromCharCodes(output), contains('LAYER'));
      }
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── Encrypt / Decrypt ──

    test('encrypt then decrypt roundtrip', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.encrypt(src(minimalPdf), encSink,
          encryption: const PdfEncryptionConfig(
              ownerPassword: 'owner', userPassword: 'user'));
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
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── ConvertToPdfA ──

    test('convertToPdfA produces valid PDF', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final sink = TestSink();
      await pdf.convertToPdfA(src(formBytes), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── ExtractPages ──

    test('extractPages keeps selected pages only', () async {
      final pdf = createPdf();
      final threePage = await _buildThreePagePdf(pdf);
      final sink = TestSink();
      await pdf.extractPages(src(threePage), sink, pages: [0, 2]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
    }, timeout: Timeout(Duration(seconds: 2)));

    // ── ImagesToPdf ──

    test('imagesToPdf creates valid PDF from images', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.imagesToPdf([src(minimalPng)], sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 2)));
  });
}

Future<Uint8List> _buildThreePagePdf(Pdf pdf) async {
  final s1 = TestSink();
  await pdf.merge([src(minimalPdf), src(minimalPdf)], s1);
  final s2 = TestSink();
  await pdf.merge([src(s1.takeBytes()), src(minimalPdf)], s2);
  return s2.takeBytes();
}
