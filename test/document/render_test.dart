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

  group('Pdf.renderPage', () {
    test('renders a page and returns pixel data', () async {
      final result = await pdf.renderPage(sourceOf(minimalPdf), 0);
      expect(result.width, greaterThan(0));
      expect(result.height, greaterThan(0));
      expect(result.data.length, greaterThan(0));
    });

    test('rendered dimensions are proportional to page aspect ratio', () async {
      final result = await pdf.renderPage(sourceOf(minimalPdf), 0);
      // A4 is 595x842 — portrait, so height > width
      expect(result.height, greaterThan(result.width));
    });

    test('throws on invalid page index', () async {
      expect(
        () => pdf.renderPage(sourceOf(minimalPdf), 99),
        throwsA(isA<PdfError>()),
      );
    });
  });

  group('Pdf.renderPageFit', () {
    test('fits render within specified dimensions', () async {
      final result = await pdf.renderPageFit(sourceOf(minimalPdf), 0,
          width: 200, height: 300);
      expect(result.width, lessThanOrEqualTo(200));
      expect(result.height, lessThanOrEqualTo(300));
      expect(result.data.length, greaterThan(0));
    });

    test('preserves aspect ratio when fitting', () async {
      final result = await pdf.renderPageFit(sourceOf(minimalPdf), 0,
          width: 1000, height: 1000);
      // A4 is portrait — fitted into a square, width should be smaller
      expect(result.height, greaterThanOrEqualTo(result.width));
    });
  });

  group('Pdf.renderPageThumbnail', () {
    test('renders a thumbnail with pixel data', () async {
      final result = await pdf.renderPageThumbnail(sourceOf(minimalPdf), 0, size: 100);
      expect(result.width, greaterThan(0));
      expect(result.height, greaterThan(0));
      expect(result.data.length, greaterThan(0));
    });

    test('thumbnail is smaller than full render', () async {
      final full = await pdf.renderPage(sourceOf(minimalPdf), 0);
      final thumb = await pdf.renderPageThumbnail(sourceOf(minimalPdf), 0, size: 50);
      expect(thumb.data.length, lessThan(full.data.length));
    });
  });

  group('Pdf.renderAllPages', () {
    test('renders every page of a multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final results = await pdf.renderAllPages(sourceOf(threePages),
          width: 150, height: 200).toList();
      expect(results.length, 3);
      for (final page in results) {
        expect(page.width, greaterThan(0));
        expect(page.height, greaterThan(0));
        expect(page.data.length, greaterThan(0));
      }
    });

    test('renders single-page PDF', () async {
      final results = await pdf.renderAllPages(sourceOf(minimalPdf),
          width: 100, height: 150).toList();
      expect(results.length, 1);
    });
  });
}
