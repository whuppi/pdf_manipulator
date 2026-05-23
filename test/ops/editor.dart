// Editor — persistent handle: open, mutate, save, dispose, redaction.

import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerEditorTests(PdfBridge Function() b) {
  group('editor', () {
    test('open → getMetadata → dispose', () async {
      final editor = await b().openEditor(src(minimalPdf));
      final pageCount = await editor.pageCount;
      expect(pageCount, 1);
      await editor.dispose();
    });

    test('open → setTitle → save', () async {
      final editor = await b().openEditor(src(minimalPdf));
      await editor.setTitle('Test Title');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('open → deletePage → save → verify', () async {
      final mergeSink = TestSink();
      await b().merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      final editor = await b().openEditor(src(twoPage));
      await editor.deletePage(0);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();

      final doc = await b().open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    });

    test('getPageMediaBox returns dimensions', () async {
      final editor = await b().openEditor(src(minimalPdf));
      final box = await editor.getPageMediaBox(0);
      expect(box.width, greaterThan(0));
      expect(box.height, greaterThan(0));
      await editor.dispose();
    });

    test('double dispose is safe', () async {
      final editor = await b().openEditor(src(minimalPdf));
      await editor.dispose();
      await editor.dispose();
    });

    test('addRedaction → applyRedactions → save', () async {
      final editor = await b().openEditor(src(minimalPdf));
      await editor.addRedaction(0, const PdfRect(x: 50, y: 50, width: 100, height: 20));
      await editor.applyRedactions();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
      final doc = await b().open(src(bytes));
      expect(doc.pageCount, 1);
    });

    test('redactionCount returns int', () async {
      final editor = await b().openEditor(src(minimalPdf));
      final count = await editor.redactionCount(0);
      expect(count, isA<int>());
      await editor.dispose();
    });

    test('scrubMetadata → save produces output', () async {
      final editor = await b().openEditor(src(minimalPdf));
      await editor.scrubMetadata();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(sink.takeBytes().length, greaterThan(0));
    });
  });
}
