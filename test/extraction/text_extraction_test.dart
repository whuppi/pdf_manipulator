import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.extractText', () {
    test('blank page returns empty or whitespace-only string', () async {
      final text = await pdf.extractText(minimalPdf);
      expect(text.trim(), isEmpty);
    });

    test('specific page extraction on blank returns empty', () async {
      final text = await pdf.extractText(minimalPdf, page: 0);
      expect(text.trim(), isEmpty);
    });

    test('throws on invalid bytes', () async {
      expect(
        () => pdf.extractText(garbageBytes),
        throwsA(isA<PdfError>()),
      );
    });

    test('extraction from merged PDF returns text from both pages', () async {
      // Both pages are blank, so combined text should also be empty
      final merged = await pdf.merge([minimalPdf, minimalPdf]);
      final text = await pdf.extractText(merged);
      expect(text.trim(), isEmpty);
    });
  });

  group('Pdf.toMarkdown', () {
    test('blank page returns empty or minimal markdown', () async {
      final md = await pdf.toMarkdown(minimalPdf);
      // Blank page should produce empty or near-empty markdown
      expect(md.trim().length, lessThan(50));
    });

    test('specific page returns consistent result', () async {
      final md1 = await pdf.toMarkdown(minimalPdf, page: 0);
      final md2 = await pdf.toMarkdown(minimalPdf, page: 0);
      expect(md1, equals(md2));
    });
  });

  group('Pdf.toHtml', () {
    test('blank page produces minimal HTML', () async {
      final html = await pdf.toHtml(minimalPdf, page: 0);
      expect(html.trim().length, lessThan(100));
    });

    test('consistent output on same input', () async {
      final h1 = await pdf.toHtml(minimalPdf, page: 0);
      final h2 = await pdf.toHtml(minimalPdf, page: 0);
      expect(h1, equals(h2));
    });
  });

  group('Pdf.toPlainText', () {
    test('blank page returns empty or whitespace', () async {
      final text = await pdf.toPlainText(minimalPdf, page: 0);
      expect(text.trim(), isEmpty);
    });

    test('consistent output', () async {
      final t1 = await pdf.toPlainText(minimalPdf, page: 0);
      final t2 = await pdf.toPlainText(minimalPdf, page: 0);
      expect(t1, equals(t2));
    });
  });
}
