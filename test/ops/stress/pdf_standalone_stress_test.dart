// Standalone stress — extractPages, convertTo on 1000-page PDF.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';

void registerStandaloneStressTests(Pdf Function() createPdf) {
  group('stress standalone', tags: 'stress', () {
    late Uint8List largePdf;

    test('build 1000-page PDF', () async {
      largePdf = await buildLargePdf(createPdf, pageCount: 1000);
      expect(largePdf.length, greaterThan(0));
    }, timeout: Timeout(Duration(seconds: 10)));

    test('extractPages first 10 from 1000-page', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.extractPages(src(largePdf), sink,
          pages: List.generate(10, (i) => i));
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 10);
    }, timeout: Timeout(Duration(seconds: 10)));
  });
}
