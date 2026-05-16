import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.extractImages', () {
    test('returns empty list for page with no images', () async {
      final images = await pdf.extractImages(minimalPdf, 0);
      expect(images, isEmpty);
    });

    test('throws on invalid page index', () async {
      expect(
        () => pdf.extractImages(minimalPdf, 99),
        throwsA(isA<PdfError>()),
      );
    });

    test('returns PdfImage list type', () async {
      final images = await pdf.extractImages(minimalPdf, 0);
      expect(images, isA<List<PdfImage>>());
    });
  });

  group('Pdf.extractAllImages', () {
    test('returns empty for PDF with no images', () async {
      final images = await pdf.extractAllImages(minimalPdf);
      expect(images, isEmpty);
    });

    test('works on multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final images = await pdf.extractAllImages(threePages);
      expect(images, isA<List<PdfImage>>());
    });
  });
}
