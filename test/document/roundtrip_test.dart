import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.kill();
  });

  group('round-trip integrity', () {
    test('open → merge → open preserves page count', () async {
      final merged = await pdf.merge([minimalPdf, minimalPdf]);
      final doc = await pdf.open(merged);
      expect(doc.pageCount, equals(2));

      final again = await pdf.merge([merged, minimalPdf]);
      final doc2 = await pdf.open(again);
      expect(doc2.pageCount, equals(3));
    });

    test('rotate → compress → open produces valid PDF', () async {
      final rotated = await pdf.rotatePages(minimalPdf, pages: {0: 90});
      final compressed = await pdf.compress(rotated);
      final doc = await pdf.open(compressed);
      expect(doc.pageCount, equals(1));
    });

    test('merge → rotate → compress → open', () async {
      final merged = await pdf.merge([minimalPdf, letterPdf]);
      final rotated = await pdf.rotateAllPages(merged, degrees: 90);
      final compressed = await pdf.compress(rotated);
      final doc = await pdf.open(compressed);
      expect(doc.pageCount, equals(2));
    });

    test('editor save → open → verify metadata', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.setTitle('Roundtrip');
      await editor.setAuthor('Miko');
      final saved = await editor.save();
      await editor.dispose();

      final doc = await pdf.open(saved);
      expect(doc.title, equals('Roundtrip'));
      expect(doc.author, equals('Miko'));
    });

    test('editor merge → save → open → verify pages', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.mergeFrom(minimalPdf);
      await editor.mergeFrom(letterPdf);
      final saved = await editor.save();
      await editor.dispose();

      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(3));
    });

    test('compress is idempotent — double compress produces valid PDF', () async {
      final first = await pdf.compress(minimalPdf);
      final second = await pdf.compress(first);
      final doc = await pdf.open(second);
      expect(doc.pageCount, equals(1));
    });

    test('10 sequential merges', () async {
      var current = minimalPdf;
      for (var i = 0; i < 10; i++) {
        current = await pdf.merge([current, minimalPdf]);
      }
      final doc = await pdf.open(current);
      expect(doc.pageCount, equals(11)); // 1 + 10 merges
    });
  });
}
