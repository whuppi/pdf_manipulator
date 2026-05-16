import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.rotatePages', () {
    test('rotate 90° changes page rotation value', () async {
      final result = await pdf.rotatePages(minimalPdf, pages: {0: 90});
      final doc = await pdf.open(result);
      expect(doc.pages[0].rotation, equals(90));
    });

    test('rotate 180° sets rotation to 180', () async {
      final result = await pdf.rotatePages(minimalPdf, pages: {0: 180});
      final doc = await pdf.open(result);
      expect(doc.pages[0].rotation, equals(180));
    });

    test('rotate 270° sets rotation to 270', () async {
      final result = await pdf.rotatePages(minimalPdf, pages: {0: 270});
      final doc = await pdf.open(result);
      expect(doc.pages[0].rotation, equals(270));
    });

    test('rotate different pages with different angles', () async {
      final twoPage = await pdf.merge([minimalPdf, minimalPdf]);
      final result = await pdf.rotatePages(twoPage, pages: {0: 90, 1: 270});
      final doc = await pdf.open(result);
      expect(doc.pages[0].rotation, equals(90));
      expect(doc.pages[1].rotation, equals(270));
    });

    test('empty pages map preserves original rotation', () async {
      final result = await pdf.rotatePages(minimalPdf, pages: {});
      final doc = await pdf.open(result);
      expect(doc.pages[0].rotation, equals(0));
    });

    test('rotation preserves page count', () async {
      final result = await pdf.rotatePages(minimalPdf, pages: {0: 90});
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(1));
    });

    test('90° rotation swaps effective dimensions', () async {
      final result = await pdf.rotatePages(minimalPdf, pages: {0: 90});
      final doc = await pdf.open(result);
      final p = doc.pages[0];
      // Original A4: 595×842. After 90° rotation:
      // effectiveWidth should be height (842), effectiveHeight should be width (595)
      expect(p.effectiveWidth, closeTo(842, 1));
      expect(p.effectiveHeight, closeTo(595, 1));
    });
  });

  group('Pdf.rotateAllPages', () {
    test('rotates all pages to same angle', () async {
      final twoPage = await pdf.merge([minimalPdf, minimalPdf]);
      final result = await pdf.rotateAllPages(twoPage, degrees: 90);
      final doc = await pdf.open(result);
      expect(doc.pages[0].rotation, equals(90));
      expect(doc.pages[1].rotation, equals(90));
    });

    test('rotation by 0° leaves rotation unchanged', () async {
      final result = await pdf.rotateAllPages(minimalPdf, degrees: 0);
      final doc = await pdf.open(result);
      expect(doc.pages[0].rotation, equals(0));
    });
  });
}
