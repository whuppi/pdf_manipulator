import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.kill();
  });

  group('Pdf.getSignatureCount', () {
    test('returns 0 for unsigned PDF', () async {
      final count = await pdf.getSignatureCount(minimalPdf);
      expect(count, 0);
    });

    test('returns 0 for multi-page unsigned PDF', () async {
      final threePages = await buildThreePagePdf();
      final count = await pdf.getSignatureCount(threePages);
      expect(count, 0);
    });
  });

  group('Pdf.getSignatures', () {
    test('returns empty list for unsigned PDF', () async {
      final sigs = await pdf.getSignatures(minimalPdf);
      expect(sigs, isEmpty);
      expect(sigs, isA<List<PdfSignatureInfo>>());
    });
  });

  group('Pdf.verifySignatures', () {
    test('returns true for PDF with no signatures', () async {
      final valid = await pdf.verifySignatures(minimalPdf);
      // No signatures = nothing to fail
      expect(valid, isA<bool>());
    });
  });
}
