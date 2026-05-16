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

  group('Pdf.encryptFull', () {
    test('encrypted output differs from input', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'owner123',
      );
      expect(encrypted.length, isNot(equals(minimalPdf.length)));
    });

    test('encrypted output is larger (encryption overhead)', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'secret',
      );
      expect(encrypted.length, greaterThan(minimalPdf.length));
    });

    test('encrypted PDF opens with correct password', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'correct',
      );
      final doc = await pdf.open(encrypted, password: 'correct');
      expect(doc.pageCount, equals(1));
    });

    test('encrypted with both passwords', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      final doc = await pdf.open(encrypted, password: 'owner');
      expect(doc.pageCount, equals(1));
    });

    test('custom algorithm produces valid PDF', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'pw',
        algorithm: 2, // AES_128
      );
      expect(encrypted.length, greaterThan(minimalPdf.length));
      final doc = await pdf.open(encrypted, password: 'pw');
      expect(doc.pageCount, equals(1));
    });

    test('restrictive permissions produce valid PDF', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'pw',
        allowPrint: false,
        allowCopy: false,
        allowModify: false,
      );
      expect(encrypted.length, greaterThan(minimalPdf.length));
      final doc = await pdf.open(encrypted, password: 'pw');
      expect(doc.pageCount, equals(1));
    });
  });

  group('PdfEditor.saveEncryptedFull', () {
    test('encrypted save produces larger output', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      final encrypted = await editor.saveEncryptedFull(ownerPassword: 'pw');
      final normal = await editor.save();
      expect(encrypted.length, greaterThan(normal.length));
      await editor.dispose();
    });

    test('modify then encrypt preserves modifications', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.mergeFrom(minimalPdf);
      final encrypted = await editor.saveEncryptedFull(
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      await editor.dispose();

      final doc = await pdf.open(encrypted, password: 'owner');
      expect(doc.pageCount, equals(2));
    });

    test('restrictive permissions via editor', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      final encrypted = await editor.saveEncryptedFull(
        ownerPassword: 'pw',
        allowPrint: false,
        allowCopy: false,
        algorithm: 3,
      );
      await editor.dispose();
      expect(encrypted.length, greaterThan(minimalPdf.length));
    });
  });
}
