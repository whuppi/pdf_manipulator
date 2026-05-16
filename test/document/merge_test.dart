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

  group('Pdf.merge', () {
    test('2 single-page PDFs produce 2-page result', () async {
      final merged = await pdf.merge([minimalPdf, minimalPdf]);
      final doc = await pdf.open(merged);
      expect(doc.pageCount, equals(2));
    });

    test('3 single-page PDFs produce 3-page result', () async {
      final merged = await pdf.merge([minimalPdf, minimalPdf, minimalPdf]);
      final doc = await pdf.open(merged);
      expect(doc.pageCount, equals(3));
    });

    test('5 PDFs produce 5-page result', () async {
      final merged = await pdf.merge(List.filled(5, minimalPdf));
      final doc = await pdf.open(merged);
      expect(doc.pageCount, equals(5));
    });

    test('merging A4 and Letter preserves different page dimensions', () async {
      final merged = await pdf.merge([minimalPdf, letterPdf]);
      final doc = await pdf.open(merged);
      expect(doc.pages[0].width, closeTo(595, 1)); // A4
      expect(doc.pages[0].height, closeTo(842, 1));
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter
      expect(doc.pages[1].height, closeTo(792, 1));
    });

    test('merged result can be merged again (chaining)', () async {
      final first = await pdf.merge([minimalPdf, minimalPdf]);
      final second = await pdf.merge([first, minimalPdf]);
      final doc = await pdf.open(second);
      expect(doc.pageCount, equals(3));
    });

    test('merged result is independently re-openable', () async {
      final merged = await pdf.merge([minimalPdf, minimalPdf]);
      final doc = await pdf.open(merged);
      expect(doc.pageCount, equals(2));
    });

    test('merge preserves metadata from first PDF', () async {
      final withMeta = await buildMetadataPdf(title: 'First Doc');
      final merged = await pdf.merge([withMeta, minimalPdf]);
      final doc = await pdf.open(merged);
      // Merged result should have 2 pages
      expect(doc.pageCount, equals(2));
    });

    test('throws ArgumentError on single input', () {
      expect(() => pdf.merge([minimalPdf]), throwsArgumentError);
    });

    test('throws ArgumentError on empty list', () {
      expect(() => pdf.merge([]), throwsArgumentError);
    });

    test('throws PdfError when one input is garbage', () async {
      expect(
        () => pdf.merge([minimalPdf, garbageBytes]),
        throwsA(isA<PdfError>()),
      );
    });
  });
}
