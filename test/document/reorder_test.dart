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

  group('Pdf.reorderPages', () {
    test('reverse order — last page becomes first', () async {
      // Original: A4 (595), Letter (612), A4 (595)
      // Reversed: A4 (595), Letter (612), A4 (595) — but positions swapped
      final result = await pdf.reorderPages(threePagePdf, order: [2, 1, 0]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
      // After reverse: page 0 was originally page 2 (A4)
      // page 1 was originally page 1 (Letter) — still in middle
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter stays middle
    });

    test('identity order preserves everything', () async {
      final result = await pdf.reorderPages(threePagePdf, order: [0, 1, 2]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
      expect(doc.pages[0].width, closeTo(595, 1)); // A4
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter
      expect(doc.pages[2].width, closeTo(595, 1)); // A4
    });

    test('move Letter page to front', () async {
      // Original: A4, Letter, A4. Reorder: [1, 0, 2] → Letter, A4, A4
      final result = await pdf.reorderPages(threePagePdf, order: [1, 0, 2]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
      expect(doc.pages[0].width, closeTo(612, 1)); // Letter now first
      expect(doc.pages[1].width, closeTo(595, 1)); // A4
    });

    test('extract subset via reorder (2 of 3 pages)', () async {
      final result = await pdf.reorderPages(threePagePdf, order: [2, 0]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(2));
    });

    test('result is re-openable', () async {
      final result = await pdf.reorderPages(threePagePdf, order: [1, 2, 0]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
    });
  });
}
