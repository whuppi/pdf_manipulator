import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  group('Pdf.kill', () {
    test('kill does not throw', () {
      final pdf = Pdf();
      pdf.kill();
    });

    test('operations work on a new instance after killing old one', () async {
      final pdf1 = Pdf();
      pdf1.kill();
      final pdf2 = Pdf();
      final doc = await pdf2.open(minimalPdf);
      expect(doc.pageCount, equals(1));
      pdf2.kill();
    });

    test('double kill is safe', () {
      final pdf = Pdf();
      pdf.kill();
      pdf.kill();
    });

    test('heavy usage then kill then new instance works', () async {
      final pdf = Pdf();
      // Use the worker
      await pdf.merge([minimalPdf, minimalPdf]);
      await pdf.rotatePages(minimalPdf, pages: {0: 90});
      await pdf.extractText(minimalPdf);

      // Kill it
      pdf.kill();

      // New instance should work
      final pdf2 = Pdf();
      final doc = await pdf2.open(minimalPdf);
      expect(doc.pageCount, equals(1));
      pdf2.kill();
    });
  });
}
