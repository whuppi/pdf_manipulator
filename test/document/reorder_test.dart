import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;
  late Uint8List threePagePdf;

  setUp(() async {
    pdf = Pdf();
    threePagePdf = await buildThreePagePdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.reorderPages', () {
    test('reverse order — last page becomes first', () async {
      final sink = TestPdfSink();
      await pdf.reorderPages(sourceOf(threePagePdf), sink, order: [2, 1, 0]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(3));
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter stays middle
    });

    test('identity order preserves everything', () async {
      final sink = TestPdfSink();
      await pdf.reorderPages(sourceOf(threePagePdf), sink, order: [0, 1, 2]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(3));
      expect(doc.pages[0].width, closeTo(595, 1)); // A4
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter
      expect(doc.pages[2].width, closeTo(595, 1)); // A4
    });

    test('move Letter page to front', () async {
      final sink = TestPdfSink();
      await pdf.reorderPages(sourceOf(threePagePdf), sink, order: [1, 0, 2]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(3));
      expect(doc.pages[0].width, closeTo(612, 1)); // Letter now first
      expect(doc.pages[1].width, closeTo(595, 1)); // A4
    });

    test('extract subset via reorder (2 of 3 pages)', () async {
      final sink = TestPdfSink();
      await pdf.reorderPages(sourceOf(threePagePdf), sink, order: [2, 0]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(2));
    });

    test('result is re-openable', () async {
      final sink = TestPdfSink();
      await pdf.reorderPages(sourceOf(threePagePdf), sink, order: [1, 2, 0]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(3));
    });
  });
}
