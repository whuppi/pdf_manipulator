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

  group('Pdf.open', () {
    test('opens minimal PDF — page count is 1', () async {
      final doc = await pdf.open(minimalPdf);
      expect(doc.pageCount, equals(1));
    });

    test('reads version string with major.minor format', () async {
      final doc = await pdf.open(minimalPdf);
      expect(doc.version, contains('.'));
      // Version should be a real PDF version like "1.4", "1.7", "2.0"
      final parts = doc.version.split('.');
      expect(parts.length, equals(2));
      expect(int.tryParse(parts[0]), isNotNull);
      expect(int.tryParse(parts[1]), isNotNull);
    });

    test('reads A4 page dimensions', () async {
      final doc = await pdf.open(minimalPdf);
      expect(doc.pages, hasLength(1));
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[0].height, closeTo(842, 1));
    });

    test('reads Letter page dimensions', () async {
      final doc = await pdf.open(letterPdf);
      expect(doc.pages[0].width, closeTo(612, 1));
      expect(doc.pages[0].height, closeTo(792, 1));
    });

    test('page index is zero-based', () async {
      final doc = await pdf.open(minimalPdf);
      expect(doc.pages[0].index, equals(0));
    });

    test('rotation defaults to 0', () async {
      final doc = await pdf.open(minimalPdf);
      expect(doc.pages[0].rotation, equals(0));
    });

    test('effective dimensions equal actual when not rotated', () async {
      final doc = await pdf.open(minimalPdf);
      final p = doc.pages[0];
      expect(p.effectiveWidth, equals(p.width));
      expect(p.effectiveHeight, equals(p.height));
    });

    test('metadata is null for minimal PDF', () async {
      final doc = await pdf.open(minimalPdf);
      expect(doc.title, isNull);
      expect(doc.author, isNull);
      expect(doc.subject, isNull);
      expect(doc.keywords, isNull);
    });

    test('isEncrypted is false', () async {
      final doc = await pdf.open(minimalPdf);
      expect(doc.isEncrypted, isFalse);
    });

    test('throws on garbage bytes', () async {
      expect(() => pdf.open(garbageBytes), throwsA(isA<PdfError>()));
    });

    test('throws on empty bytes', () async {
      expect(() => pdf.open(emptyBytes), throwsA(isA<PdfError>()));
    });

    test('throws on broken PDF', () async {
      expect(() => pdf.open(brokenPdf), throwsA(isA<PdfError>()));
    });
  });
}
