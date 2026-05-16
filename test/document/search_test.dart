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

  group('Pdf.searchPage', () {
    test('returns empty list when page has no text', () async {
      final results = await pdf.searchPage(minimalPdf, page: 0, query: 'hello');
      expect(results, isEmpty);
    });

    test('returns results as SearchResult with page index', () async {
      final results = await pdf.searchPage(minimalPdf, page: 0, query: 'test');
      expect(results, isA<List<SearchResult>>());
    });

    test('returns empty for out-of-range page', () async {
      final results = await pdf.searchPage(minimalPdf, page: 99, query: 'test');
      expect(results, isEmpty);
    });
  });

  group('Pdf.searchAll', () {
    test('returns empty list for no matches', () async {
      final results = await pdf.searchAll(minimalPdf, query: 'xyznonexistent');
      expect(results, isEmpty);
    });

    test('searches across multiple pages', () async {
      final threePages = await buildThreePagePdf();
      final results = await pdf.searchAll(threePages, query: 'test');
      expect(results, isA<List<SearchResult>>());
    });
  });
}
