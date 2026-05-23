// Content — extract text (plain/markdown/html), search.

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerContentTests(PdfBridge Function() b) {
  group('content', () {
    test('extract text returns string', () async {
      final text = await b().extract(
        src(minimalPdf),
        pages: const PdfPages.single(0),
      );
      expect(text, isA<String>());
    });

    test('extract all pages returns string', () async {
      final text = await b().extract(
        src(minimalPdf),
        pages: const PdfPages.all(),
      );
      expect(text, isA<String>());
    });

    test('extract markdown returns string', () async {
      final text = await b().extract(
        src(minimalPdf),
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect(text, isA<String>());
    });

    test('search returns list', () async {
      final results = await b().search(
        src(minimalPdf),
        query: 'test',
        pages: const PdfPages.single(0),
      );
      expect(results, isA<List<SearchResult>>());
    });

    test('search for nonexistent returns empty', () async {
      final results = await b().search(
        src(minimalPdf),
        query: 'xyznonexistent',
        pages: const PdfPages.all(),
      );
      expect(results, isEmpty);
    });

    test('validatePdfA returns result', () async {
      final result = await b().validatePdfA(src(minimalPdf));
      expect(result, isNotNull);
    });

    test('validatePdfUa returns bool', () async {
      final result = await b().validatePdfUa(src(minimalPdf));
      expect(result, isA<bool>());
    });
  });
}
