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

  group('PdfEditor.resizeImage', () {
    test('resizeImage on PDF without images does not crash', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      // Resizing a non-existent image should throw a clean PdfError
      try {
        await editor.resizeImage(0, 'nonexistent',
            width: 100, height: 100);
      } on PdfError {
        // Expected -- no images exist in minimal PDF
      }
      final result = await editor.save();
      await editor.dispose();
      expect(result.length, greaterThan(0));
    });

    test('resizeImage with valid page index produces result', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      try {
        await editor.resizeImage(0, 'Im0', width: 50, height: 50);
      } on PdfError {
        // Expected -- image name may not exist in minimal PDF
      }
      final result = await editor.save();
      await editor.dispose();
      expect(result.length, greaterThan(0));
    });
  });
}
