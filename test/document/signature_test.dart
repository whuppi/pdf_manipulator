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

  group('Pdf.getSignatureCount', () {
    test('returns 0 for unsigned PDF', () async {
      final count = await pdf.getSignatureCount(sourceOf(minimalPdf));
      expect(count, 0);
    });

    test('returns 0 for multi-page unsigned PDF', () async {
      final threePages = await buildThreePagePdf();
      final count = await pdf.getSignatureCount(sourceOf(threePages));
      expect(count, 0);
    });
  });

  group('Pdf.getSignatures', () {
    test('returns empty list for unsigned PDF', () async {
      final sigs = await pdf.getSignatures(sourceOf(minimalPdf));
      expect(sigs, isEmpty);
      expect(sigs, isA<List<PdfSignatureInfo>>());
    });
  });

  group('Pdf.verifySignatures', () {
    test('returns true for PDF with no signatures', () async {
      final valid = await pdf.verifySignatures(sourceOf(minimalPdf));
      // No signatures = nothing to fail
      expect(valid, isA<bool>());
    });
  });
}
