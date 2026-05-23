// Builder — create PDF from scratch: pages, text, metadata, imagesToPdf.

import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerBuilderTests(PdfBridge Function() b) {
  group('builder', () {
    test('create → addPage → text → save', () async {
      final builder = await b().createBuilder();
      final page = await builder.addPage(width: 612, height: 792);
      await page.text('Hello World');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('create → setMetadata → save', () async {
      final builder = await b().createBuilder();
      await builder.setTitle('Test');
      await builder.setAuthor('Author');
      final page = await builder.addPage(width: 612, height: 792);
      await page.text('content');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('double dispose is safe', () async {
      final builder = await b().createBuilder();
      await builder.dispose();
      await builder.dispose();
    });

    test('imagesToPdf creates valid PDF', () async {
      final sink = TestSink();
      await b().imagesToPdf([minimalPng], sink);
      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
      final doc = await b().open(src(bytes));
      expect(doc.pageCount, 1);
    });
  });
}
