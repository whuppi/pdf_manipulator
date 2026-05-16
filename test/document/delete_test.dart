import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

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

  group('Pdf.deletePages', () {
    test('deleting 1 page from 3-page PDF leaves 2 pages', () async {
      final result = await pdf.deletePages(threePagePdf, pages: [1]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(2));
    });

    test('deleting 2 pages from 3-page PDF leaves 1 page', () async {
      final result = await pdf.deletePages(threePagePdf, pages: [0, 2]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(1));
    });

    test('deleting first page shifts remaining pages', () async {
      // Original: page 0 (A4 595), page 1 (Letter 612), page 2 (A4 595)
      // Delete page 0 → remaining: Letter (612), A4 (595)
      final result = await pdf.deletePages(threePagePdf, pages: [0]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(2));
      // First remaining page should be the Letter page (was page 1)
      expect(doc.pages[0].width, closeTo(612, 1));
    });

    test('deleting middle page preserves first and last', () async {
      // Original: A4, Letter, A4. Delete middle (Letter).
      final result = await pdf.deletePages(threePagePdf, pages: [1]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(2));
      // Both remaining should be A4
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[1].width, closeTo(595, 1));
    });

    test('deleting last page preserves earlier pages', () async {
      final result = await pdf.deletePages(threePagePdf, pages: [2]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(2));
      expect(doc.pages[0].width, closeTo(595, 1)); // A4
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter
    });

    test('result is re-openable', () async {
      final result = await pdf.deletePages(threePagePdf, pages: [1]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(2));
    });
  });

  group('Pdf.movePage', () {
    test('move preserves page count', () async {
      final result = await pdf.movePage(threePagePdf, from: 0, to: 2);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
    });

    test('move to same position preserves page count', () async {
      final result = await pdf.movePage(threePagePdf, from: 1, to: 1);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
    });

    test('result is re-openable', () async {
      final result = await pdf.movePage(threePagePdf, from: 2, to: 0);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
    });
  });

  group('PdfEditor.deletePage', () {
    test('reduces page count', () async {
      final editor = await Pdf.edit(threePagePdf);
      expect(await editor.pageCount, equals(3));
      await editor.deletePage(1);
      expect(await editor.pageCount, equals(2));
      editor.dispose();
    });

    test('multiple deletes in descending order', () async {
      final editor = await Pdf.edit(threePagePdf);
      await editor.deletePage(2);
      await editor.deletePage(0);
      expect(await editor.pageCount, equals(1));
      editor.dispose();
    });

    test('saved result has correct page count', () async {
      final editor = await Pdf.edit(threePagePdf);
      await editor.deletePage(1);
      final saved = await editor.save();
      editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(2));
    });

    test('delete first page — remaining starts with Letter', () async {
      final editor = await Pdf.edit(threePagePdf);
      await editor.deletePage(0); // Delete A4, Letter becomes first
      final saved = await editor.save();
      editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pages[0].width, closeTo(612, 1)); // Letter
    });
  });
}
