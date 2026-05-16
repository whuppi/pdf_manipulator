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

  group('PdfEditorHandle.addStamp', () {
    test('adds stamp — output is larger', () async {
      final handle = await pdf.openEditor(minimalPdf);
      await handle.addStamp(
        0,
        stampType: 0, // Approved
        x: 100,
        y: 100,
        width: 200,
        height: 60,
      );
      final saved = await handle.save();
      expect(saved.length, greaterThan(minimalPdf.length));
      await handle.dispose();
    });

    test('stamp marks document as modified', () async {
      final handle = await pdf.openEditor(minimalPdf);
      expect(await handle.isModified, isFalse);
      await handle.addStamp(
        0,
        stampType: 1, // Experimental
        x: 50,
        y: 50,
        width: 150,
        height: 50,
      );
      expect(await handle.isModified, isTrue);
      await handle.dispose();
    });

    test('stamp produces valid PDF', () async {
      final handle = await pdf.openEditor(minimalPdf);
      await handle.addStamp(
        0,
        stampType: 2, // NotApproved
        x: 200,
        y: 400,
        width: 200,
        height: 80,
      );
      final saved = await handle.save();
      await handle.dispose();

      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(1));
    });

    test('custom stamp with name', () async {
      final handle = await pdf.openEditor(minimalPdf);
      await handle.addStamp(
        0,
        stampType: 14, // Custom
        customName: 'MyStamp',
        x: 100,
        y: 100,
        width: 200,
        height: 60,
      );
      final saved = await handle.save();
      expect(saved.length, greaterThan(minimalPdf.length));
      await handle.dispose();
    });

    test('stamp with custom opacity', () async {
      final handle = await pdf.openEditor(minimalPdf);
      await handle.addStamp(
        0,
        stampType: 0,
        x: 100,
        y: 100,
        width: 200,
        height: 60,
        opacity: 0.5,
      );
      final saved = await handle.save();
      expect(saved.length, greaterThan(minimalPdf.length));
      await handle.dispose();
    });
  });
}
