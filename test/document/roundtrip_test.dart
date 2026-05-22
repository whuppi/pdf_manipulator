import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

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

  group('round-trip integrity', () {
    test('open → merge → open preserves page count', () async {
      final sink1 = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(minimalPdf)], sink1);
      final merged = sink1.takeBytes();
      final doc = await pdf.open(sourceOf(merged));
      expect(doc.pageCount, equals(2));

      final sink2 = TestPdfSink();
      await pdf.merge([sourceOf(merged), sourceOf(minimalPdf)], sink2);
      final again = sink2.takeBytes();
      final doc2 = await pdf.open(sourceOf(again));
      expect(doc2.pageCount, equals(3));
    });

    test('rotate → compress → open produces valid PDF', () async {
      final rotSink = TestPdfSink();
      await pdf.rotatePages(sourceOf(minimalPdf), rotSink, pages: {0: 90});
      final rotated = rotSink.takeBytes();
      final compSink = TestPdfSink();
      await pdf.compress(sourceOf(rotated), compSink);
      final compressed = compSink.takeBytes();
      final doc = await pdf.open(sourceOf(compressed));
      expect(doc.pageCount, equals(1));
    });

    test('merge → rotate → compress → open', () async {
      final mergeSink = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(letterPdf)], mergeSink);
      final merged = mergeSink.takeBytes();
      final rotSink = TestPdfSink();
      await pdf.rotateAllPages(sourceOf(merged), rotSink, degrees: 90);
      final rotated = rotSink.takeBytes();
      final compSink = TestPdfSink();
      await pdf.compress(sourceOf(rotated), compSink);
      final compressed = compSink.takeBytes();
      final doc = await pdf.open(sourceOf(compressed));
      expect(doc.pageCount, equals(2));
    });

    test('editor save → open → verify metadata', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setTitle('Roundtrip');
      await editor.setAuthor('Miko');
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.title, equals('Roundtrip'));
      expect(doc.author, equals('Miko'));
    });

    test('editor merge → save → open → verify pages', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(letterPdf));
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(3));
    });

    test('compress is idempotent — double compress produces valid PDF', () async {
      final sink1 = TestPdfSink();
      await pdf.compress(sourceOf(minimalPdf), sink1);
      final first = sink1.takeBytes();
      final sink2 = TestPdfSink();
      await pdf.compress(sourceOf(first), sink2);
      final second = sink2.takeBytes();
      final doc = await pdf.open(sourceOf(second));
      expect(doc.pageCount, equals(1));
    });

    test('10 sequential merges', () async {
      var current = minimalPdf;
      for (var i = 0; i < 10; i++) {
        final sink = TestPdfSink();
        await pdf.merge([sourceOf(current), sourceOf(minimalPdf)], sink);
        current = sink.takeBytes();
      }
      final doc = await pdf.open(sourceOf(current));
      expect(doc.pageCount, equals(11)); // 1 + 10 merges
    });
  });
}
