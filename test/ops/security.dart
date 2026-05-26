// Security — watermark, encrypt/decrypt, sign, signatures.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerSecurityTests(Pdf Function() createPdf) {
  group('security', () {
    // ── Watermark ──

    test('watermark embeds text and preserves page count', () async {
      final pdf = createPdf();
      final input = bookmarkedPdf; // 2-page PDF with text content
      final sink = TestSink();
      await pdf.watermark(src(input), sink, text: 'CONFIDENTIAL');
      final output = sink.takeBytes();

      // Re-open to verify structural integrity.
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 2, reason: 'page count must be preserved');

      // Watermark annotation adds bytes.
      expect(output.length, greaterThan(input.length),
          reason: 'watermark annotation must increase file size');

      // The watermark text string appears in the output bytes.
      final asString = String.fromCharCodes(output);
      expect(asString, contains('CONFIDENTIAL'),
          reason: 'watermark text must be present in output bytes');
    });

    test('watermark position variants each produce valid output', () async {
      final pdf = createPdf();
      final input = bookmarkedPdf;
      final inputLen = input.length;

      final positions = <String, PdfWatermarkPosition>{
        'center': const PdfWatermarkPosition.center(),
        'corner': const PdfWatermarkPosition.corner(PdfCorner.topRight),
        'tiled': const PdfWatermarkPosition.tiled(columns: 2, rows: 2),
        'exact': const PdfWatermarkPosition.exact(
            x: 50, y: 50, width: 200, height: 100),
      };

      for (final entry in positions.entries) {
        final sink = TestSink();
        await pdf.watermark(src(input), sink,
            text: 'POS', position: entry.value);
        final output = sink.takeBytes();

        // Every variant must produce a valid PDF larger than input.
        final doc = await pdf.open(src(output));
        expect(doc.pageCount, 2, reason: '${entry.key}: page count preserved');
        expect(output.length, greaterThan(inputLen),
            reason: '${entry.key}: watermark must increase file size');
      }
    });

    test('watermark foreground and background layers produce valid output',
        () async {
      final pdf = createPdf();
      final input = bookmarkedPdf;

      for (final layer in PdfWatermarkLayer.values) {
        final sink = TestSink();
        await pdf.watermark(src(input), sink,
            text: 'LAYER', layer: layer);
        final output = sink.takeBytes();

        final doc = await pdf.open(src(output));
        expect(doc.pageCount, 2, reason: '${layer.name}: page count preserved');
        expect(output.length, greaterThan(input.length),
            reason: '${layer.name}: watermark must increase file size');

        // Both layers should contain the watermark text in the output bytes.
        final asString = String.fromCharCodes(output);
        expect(asString, contains('LAYER'),
            reason:
                '${layer.name}: watermark text must appear in output bytes');
      }
    });

    // ── Encrypt / Decrypt ──

    test('encrypt then decrypt roundtrip preserves content', () async {
      final pdf = createPdf();

      // Encrypt.
      final encSink = TestSink();
      await pdf.encrypt(src(minimalPdf), encSink,
          encryption: const PdfEncryptionConfig(
              ownerPassword: 'owner', userPassword: 'user'));
      final encrypted = encSink.takeBytes();
      expect(encrypted.length, greaterThan(0));

      // Verify the intermediate encrypted PDF reports isEncrypted.
      final encDoc = await pdf.open(src(encrypted), password: 'owner');
      expect(encDoc.isEncrypted, isTrue,
          reason: 'encrypted PDF must report isEncrypted == true');

      // Decrypt.
      final decSink = TestSink();
      await pdf.decrypt(src(encrypted), decSink, password: 'owner');
      final decrypted = decSink.takeBytes();
      expect(decrypted.length, greaterThan(0));

      // Verify the decrypted PDF opens with correct page count.
      final decDoc = await pdf.open(src(decrypted));
      expect(decDoc.pageCount, 1);
    });

    // ── Signatures ──

    test('getSignatures returns empty for unsigned PDF', () async {
      final sigs = await createPdf().getSignatures(src(minimalPdf));
      expect(sigs, isEmpty);
    });

    test('verifySignatures returns false for unsigned PDF', () async {
      final valid = await createPdf().verifySignatures(src(minimalPdf));
      expect(valid, isFalse);
    });

    test('sign with PKCS12 adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(src(minimalPdf), sink,
          credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'));
      final signed = sink.takeBytes();

      // Verify the signed PDF opens with correct page count.
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);

      // Verify at least one signature is retrievable.
      final sigs = await pdf.getSignatures(src(signed));
      expect(sigs, isNotEmpty,
          reason: 'signed PDF must contain at least one signature');
    });

    test('sign with PEM adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(src(minimalPdf), sink,
          credentials:
              const PdfSigningCredentials.pem(testCertPem, testKeyPem));
      final signed = sink.takeBytes();

      // Verify the signed PDF opens with correct page count.
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);

      // Verify at least one signature is retrievable.
      final sigs = await pdf.getSignatures(src(signed));
      expect(sigs, isNotEmpty,
          reason: 'signed PDF must contain at least one signature');
    });

    test('sign with invalid cert throws', () async {
      final pdf = createPdf();
      final sink = TestSink();
      expect(
        () => pdf.sign(src(minimalPdf), sink,
            credentials: PdfSigningCredentials.pkcs12(
                Uint8List.fromList([1, 2, 3, 4]), 'wrong')),
        throwsA(anything),
      );
    });
  });
}
