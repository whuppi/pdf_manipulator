// Merge — combine multiple PDFs into one.

import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerMergeTests(PdfBridge Function() b) {
  group('merge', () {
    test('two identical PDFs → 2 pages', () async {
      final sink = TestSink();
      await b().merge([src(minimalPdf), src(minimalPdf)], sink);
      final merged = sink.takeBytes();
      expect(merged.length, greaterThan(0));
      final doc = await b().open(src(merged));
      expect(doc.pageCount, 2);
    });

    test('three PDFs → 3 pages', () async {
      final sink = TestSink();
      await b().merge([src(minimalPdf), src(minimalPdf), src(minimalPdf)], sink);
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
    });

    test('merge different sized PDFs', () async {
      final sink = TestSink();
      await b().merge([src(minimalPdf), src(letterPdf)], sink);
      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
    });

    test('merged output is valid PDF', () async {
      final sink = TestSink();
      await b().merge([src(minimalPdf), src(minimalPdf)], sink);
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x25); // '%' — PDF header
      expect(bytes[1], 0x50); // 'P'
      expect(bytes[2], 0x44); // 'D'
      expect(bytes[3], 0x46); // 'F'
    });
  });
}
