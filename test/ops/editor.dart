// Editor — persistent handle: open, mutate, save, dispose.

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
  });
}
