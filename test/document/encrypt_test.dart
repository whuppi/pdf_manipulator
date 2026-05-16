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

  group('Pdf.encrypt', () {
    test('encrypted output differs from input (encryption applied)', () async {
      final encrypted = await pdf.encrypt(
        minimalPdf,
        ownerPassword: 'owner123',
      );
      expect(encrypted.length, isNot(equals(minimalPdf.length)));
    });

    test('encrypted output is larger (encryption overhead)', () async {
      final encrypted = await pdf.encrypt(
        minimalPdf,
        ownerPassword: 'secret',
      );
      expect(encrypted.length, greaterThan(minimalPdf.length));
    });

    test('encrypted PDF opens with correct password', () async {
      final encrypted = await pdf.encrypt(
        minimalPdf,
        ownerPassword: 'correct',
      );
      final doc = await pdf.open(encrypted, password: 'correct');
      expect(doc.pageCount, equals(1));
    });

    test('encrypted with both passwords', () async {
      final encrypted = await pdf.encrypt(
        minimalPdf,
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      // Both passwords should work
      final doc = await pdf.open(encrypted, password: 'owner');
      expect(doc.pageCount, equals(1));
    });
  });

  group('Pdf.decrypt', () {
    test('round-trip: encrypt → decrypt preserves page count', () async {
      final encrypted = await pdf.encrypt(minimalPdf, ownerPassword: 'pw');
      final decrypted = await pdf.decrypt(encrypted, password: 'pw');
      final doc = await pdf.open(decrypted);
      expect(doc.pageCount, equals(1));
    });

    test('decrypted output differs from encrypted (encryption removed)', () async {
      final encrypted = await pdf.encrypt(minimalPdf, ownerPassword: 'pw');
      final decrypted = await pdf.decrypt(encrypted, password: 'pw');
      expect(decrypted.length, isNot(equals(encrypted.length)));
    });
  });

  group('PdfEditor.saveEncrypted', () {
    test('encrypted save produces larger output', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      final encrypted = await editor.saveEncrypted(ownerPassword: 'pw');
      final normal = await editor.save();
      expect(encrypted.length, greaterThan(normal.length));
      await editor.dispose();
    });

    test('modify then encrypt preserves modifications', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.mergeFrom(minimalPdf);
      final encrypted = await editor.saveEncrypted(
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      await editor.dispose();

      final doc = await pdf.open(encrypted, password: 'owner');
      expect(doc.pageCount, equals(2));
    });
  });
}
