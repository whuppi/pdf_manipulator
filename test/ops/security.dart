// Security — watermark, encrypt/decrypt, sign, signatures.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerSecurityTests(PdfBridge Function() b) {
  group('security', () {
    test('watermark produces output', () async {
      final sink = TestSink();
      await b().watermark(src(minimalPdf), sink, text: 'DRAFT');
      expect(sink.takeBytes().length, greaterThan(0));
    });

    test('encrypt then decrypt roundtrip', () async {
      final encSink = TestSink();
      await b().encrypt(src(minimalPdf), encSink,
          encryption: const PdfEncryptionConfig(ownerPassword: 'owner', userPassword: 'user'));
      final encrypted = encSink.takeBytes();
      expect(encrypted.length, greaterThan(0));

      final decSink = TestSink();
      await b().decrypt(src(encrypted), decSink, password: 'owner');
      final decrypted = decSink.takeBytes();
      expect(decrypted.length, greaterThan(0));

      final doc = await b().open(src(decrypted));
      expect(doc.pageCount, 1);
    });

    test('getSignatures returns empty for unsigned PDF', () async {
      final sigs = await b().getSignatures(src(minimalPdf));
      expect(sigs, isEmpty);
    });

    test('verifySignatures returns false for unsigned PDF', () async {
      final valid = await b().verifySignatures(src(minimalPdf));
      expect(valid, isFalse);
    });

    test('sign with PKCS12 produces valid PDF', () async {
      final sink = TestSink();
      await b().sign(src(minimalPdf), sink,
          credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'));
      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
      final doc = await b().open(src(bytes));
      expect(doc.pageCount, 1);
    });

    test('sign with PEM produces valid PDF', () async {
      final sink = TestSink();
      await b().sign(src(minimalPdf), sink,
          credentials: const PdfSigningCredentials.pem(testCertPem, testKeyPem));
      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
      final doc = await b().open(src(bytes));
      expect(doc.pageCount, 1);
    });

    test('sign with invalid cert throws', () async {
      final sink = TestSink();
      expect(
        () => b().sign(src(minimalPdf), sink,
            credentials: PdfSigningCredentials.pkcs12(
                Uint8List.fromList([1, 2, 3, 4]), 'wrong')),
        throwsA(anything),
      );
    });
  });
}
