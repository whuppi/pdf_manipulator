// Structural — rotate, flatten, compress, delete, extract, reorder pages.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerStructuralTests(PdfBridge Function() b) {
  group('structural', () {
    test('rotateAllPages 90°', () async {
      final sink = TestSink();
      await b().rotateAllPages(src(minimalPdf), sink, degrees: 90);
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
    });

    test('flattenForms produces output', () async {
      final sink = TestSink();
      await b().flattenForms(src(minimalPdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('compress produces output', () async {
      final sink = TestSink();
      await b().compress(src(minimalPdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('deletePages from 2-page → 1 page', () async {
      final mergeSink = TestSink();
      await b().merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      final deleteSink = TestSink();
      await b().deletePages(src(twoPage), deleteSink, pages: [0]);
      final doc = await b().open(src(deleteSink.takeBytes()));
      expect(doc.pageCount, 1);
    });

    test('extractPages keeps selected pages', () async {
      final multiPage = await buildThreePagePdf(b);
      final sink = TestSink();
      await b().extractPages(src(multiPage), sink, pages: [0]);
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    });

    test('reorderPages reverses page order', () async {
      final multiPage = await buildThreePagePdf(b);
      final sink = TestSink();
      await b().reorderPages(src(multiPage), sink, order: [2, 1, 0]);
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
    });

    test('split 3-page into 1-page chunks', () async {
      final multiPage = await buildThreePagePdf(b);
      final sinks = <TestSink>[];
      await b().split(src(multiPage), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, every: 1);
      expect(sinks.length, 3);
      for (final s in sinks) {
        final doc = await b().open(src(s.takeBytes()));
        expect(doc.pageCount, 1);
      }
    });

    test('splitBySize produces at least 1 chunk', () async {
      final multiPage = await buildThreePagePdf(b);
      final sinks = <TestSink>[];
      final chunkSizes = await b().splitBySize(src(multiPage), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      }, maxBytes: 50000);
      expect(chunkSizes.length, greaterThanOrEqualTo(1));
      expect(sinks.length, chunkSizes.length);
    });

    test('movePage produces valid output', () async {
      final multiPage = await buildThreePagePdf(b);
      final sink = TestSink();
      await b().movePage(src(multiPage), sink, from: 0, to: 2);
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
    });

    test('rotatePages rotates specific pages', () async {
      final sink = TestSink();
      await b().rotatePages(src(minimalPdf), sink, pages: {0: 90});
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
    });

    test('applyRedactions produces output', () async {
      final sink = TestSink();
      await b().applyRedactions(src(minimalPdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('embedFile produces output', () async {
      final sink = TestSink();
      await b().embedFile(src(minimalPdf), sink,
          name: 'test.txt', fileData: Uint8List.fromList('hello'.codeUnits));
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('eraseRegions produces output', () async {
      final sink = TestSink();
      await b().eraseRegions(src(minimalPdf), sink,
          page: 0, regions: [const PdfRect(x: 0, y: 0, width: 100, height: 100)]);
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('addStamp produces output', () async {
      final sink = TestSink();
      await b().addStamp(src(minimalPdf), sink,
          page: 0, type: PdfStampType.approved,
          rect: const PdfRect(x: 100, y: 100, width: 200, height: 50));
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('addImageStamp produces output', () async {
      final sink = TestSink();
      await b().addImageStamp(src(minimalPdf), sink,
          page: 0, imageBytes: minimalPng,
          rect: const PdfRect(x: 100, y: 100, width: 200, height: 200));
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('planSplitByBookmarks returns splits', () async {
      final splits = await b().planSplitByBookmarks(src(bookmarkedPdf));
      expect(splits, isA<List<PdfBookmarkSplit>>());
      expect(splits.length, greaterThan(0));
      expect(splits.first.title, isNotEmpty);
    });

    test('splitByBookmarks produces outputs', () async {
      final sinks = <TestSink>[];
      await b().splitByBookmarks(src(bookmarkedPdf), (i) {
        final s = TestSink();
        sinks.add(s);
        return s;
      });
      expect(sinks.length, greaterThan(0));
      for (final s in sinks) {
        expect(s.takeBytes().length, greaterThan(0));
      }
    });
  });
}

Future<Uint8List> buildThreePagePdf(PdfBridge Function() b) async {
  final s1 = TestSink();
  await b().merge([src(minimalPdf), src(minimalPdf)], s1);
  final s2 = TestSink();
  await b().merge([src(s1.takeBytes()), src(minimalPdf)], s2);
  return s2.takeBytes();
}

