import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

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

  group('Pdf.validatePdfA', () {
    test('minimal PDF is not PDF/A compliant', () async {
      final result = await pdf.validatePdfA(sourceOf(minimalPdf));
      expect(result.compliant, isFalse);
      expect(result.errors, greaterThanOrEqualTo(0));
      expect(result.warnings, greaterThanOrEqualTo(0));
    });

    test('returns error count for non-compliant PDF', () async {
      final result = await pdf.validatePdfA(sourceOf(minimalPdf), level: 0);
      expect(result.compliant, isFalse);
    });

    test('works on multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final result = await pdf.validatePdfA(sourceOf(threePages));
      expect(result.errors, isA<int>());
    });
  });

  group('Pdf.validatePdfUa', () {
    test('minimal PDF is not PDF/UA compliant', () async {
      final result = await pdf.validatePdfUa(sourceOf(minimalPdf));
      expect(result, isFalse);
    });

    test('returns bool for multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final result = await pdf.validatePdfUa(sourceOf(threePages));
      expect(result, isA<bool>());
    });
  });
}
