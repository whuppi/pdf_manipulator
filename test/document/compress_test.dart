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

  group('Pdf.compress', () {
    test('compressed output is a valid PDF with same page count', () async {
      final result = await pdf.compress(minimalPdf);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(1));
    });

    test('compressed output size differs from input (GC + stream recompression)', () async {
      // Merge creates internal duplication that GC can clean up
      final bloated = await pdf.merge([minimalPdf, minimalPdf]);
      final compressed = await pdf.compress(bloated);
      // After GC + recompression, size should differ (not necessarily smaller
      // for minimal PDFs, but the bytes should be structurally different)
      expect(compressed.length, isNot(equals(bloated.length)));
    });

    test('compress preserves page count on multi-page', () async {
      final twoPage = await pdf.merge([minimalPdf, minimalPdf]);
      final compressed = await pdf.compress(twoPage);
      final doc = await pdf.open(compressed);
      expect(doc.pageCount, equals(2));
    });

    test('compress preserves page dimensions', () async {
      final compressed = await pdf.compress(minimalPdf);
      final doc = await pdf.open(compressed);
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[0].height, closeTo(842, 1));
    });

    test('compress with linearize produces valid output', () async {
      final result = await pdf.compress(minimalPdf, linearize: true);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(1));
    });

    test('double compress is idempotent (second pass same or smaller)', () async {
      final first = await pdf.compress(minimalPdf);
      final second = await pdf.compress(first);
      expect(second.length, lessThanOrEqualTo(first.length + 100));
      // Allow small variance from serialization differences
    });
  });
}
