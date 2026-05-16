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

  group('Pdf.getPermissions', () {
    test('unencrypted PDF has all permissions enabled', () async {
      final perms = await pdf.getPermissions(minimalPdf);
      expect(perms.print, isTrue);
      expect(perms.printHq, isTrue);
      expect(perms.modify, isTrue);
      expect(perms.copy, isTrue);
      expect(perms.annotate, isTrue);
      expect(perms.fillForms, isTrue);
      expect(perms.accessibility, isTrue);
      expect(perms.assemble, isTrue);
    });

    test('encrypted with restrictive permissions reports them', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'owner',
        allowPrint: false,
        allowCopy: false,
        allowModify: false,
      );
      final perms = await pdf.getPermissions(encrypted, password: 'owner');
      expect(perms.print, isFalse);
      expect(perms.copy, isFalse);
      expect(perms.modify, isFalse);
    });

    test('encrypted with all permissions enabled', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'pw',
      );
      final perms = await pdf.getPermissions(encrypted, password: 'pw');
      expect(perms.print, isTrue);
      expect(perms.copy, isTrue);
      expect(perms.modify, isTrue);
    });
  });

  group('Pdf.getEncryptionAlgorithm', () {
    test('unencrypted PDF returns -1', () async {
      final alg = await pdf.getEncryptionAlgorithm(minimalPdf);
      expect(alg, equals(-1));
    });

    test('AES-256 encrypted returns 3', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'pw',
        algorithm: 3,
      );
      final alg = await pdf.getEncryptionAlgorithm(encrypted, password: 'pw');
      expect(alg, equals(3));
    });

    test('AES-128 encrypted returns 2', () async {
      final encrypted = await pdf.encryptFull(
        minimalPdf,
        ownerPassword: 'pw',
        algorithm: 2,
      );
      final alg = await pdf.getEncryptionAlgorithm(encrypted, password: 'pw');
      expect(alg, equals(2));
    });
  });
}
