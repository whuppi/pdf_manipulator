import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/memory_io.dart';
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
      final images =
          await pdf.extractImages(sourceOf(minimalPdf), 0).toList();
      expect(images, isEmpty);
    });

    test('throws on invalid page index', () async {
      expect(
        () => pdf.extractImages(sourceOf(minimalPdf), 99).toList(),
        throwsA(isA<PdfError>()),
      );
    });

    test('returns PdfImage list type', () async {
      final images =
          await pdf.extractImages(sourceOf(minimalPdf), 0).toList();
      expect(images, isA<List<PdfImage>>());
    });
  });

  group('Pdf.extractAllImages', () {
    test('returns empty for PDF with no images', () async {
      final images = await pdf.extractAllImages(sourceOf(minimalPdf)).toList();
      expect(images, isEmpty);
    });

    test('works on multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final images =
          await pdf.extractAllImages(sourceOf(threePages)).toList();
      expect(images, isA<List<PdfImage>>());
    });
  });
}
