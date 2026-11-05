// CHARTER — this battery alone proves: a full rewrite that keeps the source's
// own encryption (the default `PdfEncryption.keep()`) writes the source's
// handler, algorithm, file ID and key material back, writes nothing for an
// unencrypted source, and refuses with a typed error, in the engine's words,
// each encrypted source whose handler it cannot reproduce. That an explicit
// encryption config applies is the editor battery's claim, not this one's.
//
// Diet: `keptEncryptionPdf`, a handwritten AES-256 source whose /Encrypt
// dictionary is edited per case; the key wrapping behind it is the engine
// writer's own, committed once. Presence proofs are SEMANTIC where a reader
// sees them (the output opens with the source's password and reports its
// algorithm); the dictionary's keys and the file ID are read from the saved
// bytes.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerKeptEncryptionTests(Pdf Function() createPdf) {
  group('kept encryption', () {
    /// [source] saved by a full rewrite with the default encryption.
    Future<Uint8List> keepSave(Uint8List source) async {
      final editor = await createPdf().edit(src(source));
      try {
        final sink = TestSink();
        await editor.save(sink);
        return sink.takeBytes();
      } finally {
        await editor.dispose();
      }
    }

    /// The error that keeping [source]'s encryption ends in, whether the
    /// source is refused on open or on save.
    Future<PdfError> refusal(Uint8List source) async {
      try {
        await keepSave(source);
      } on PdfError catch (e) {
        return e;
      }
      fail('keeping the encryption was not refused');
    }

    /// Expects [source] refused as unsupported, with [reason] in the
    /// engine's message.
    Future<void> expectRefused(Uint8List source, String reason) async {
      final error = await refusal(source);
      expect(error, isA<PdfUnsupported>());
      expect(
        error.message,
        allOf(
          contains('keeping this source\'s encryption is not supported: '),
          contains(reason),
          endsWith('save it with explicit encryption'),
        ),
      );
    }

    test('a standard AES-256 source keeps its parameters', () async {
      final out = await keepSave(keptEncryptionPdf());

      final doc = await createPdf().open(src(out));
      expect(doc.encryption?.algorithm, PdfEncryptionAlgorithm.aes256);
      expect(doc.pageCount, 1);
      await doc.dispose();

      // bytegrep-exempt: the claim is the encryption dictionary and file ID.
      final text = String.fromCharCodes(out); // bytegrep-exempt
      expect(
        text,
        // The second string is sixteen 0x22 bytes, written as a literal.
        contains(RegExp(r'/ID\s*\[\s*<1{32}>\s*\("{16}\)\s*\]')),
        reason: 'the file ID is the source\'s',
      );
      final encrypt = RegExp(r'/Encrypt\s+(\d+) 0 R').firstMatch(text);
      expect(encrypt, isNotNull);
      final header = RegExp('^${encrypt![1]} 0 obj\$', multiLine: true);
      final from = text.indexOf(header);
      final dict = text.substring(from, text.indexOf('endobj', from));
      for (final key in [
        'Filter', 'V', 'R', 'O', 'U', 'OE', 'UE', 'P', 'Perms', 'CF', //
        'StmF', 'StrF',
      ]) {
        expect(dict, contains('/$key'), reason: '/$key of the kept dictionary');
      }
    }, timeout: t(1));

    test('an unencrypted source has nothing to keep', () async {
      final doc = await createPdf().open(src(await keepSave(minimalPdf)));
      expect(doc.encryption, isNull);
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('a non-standard security handler is refused', () async {
      await expectRefused(
        keptEncryptionPdf(encrypt: {'Filter': '/Adobe.PubSec'}),
        'public-key or non-standard security handler',
      );
    }, timeout: t(1));

    test('a /SubFilter is refused', () async {
      await expectRefused(
        keptEncryptionPdf(encrypt: {'SubFilter': '/adbe.pkcs7.s5'}),
        '/SubFilter',
      );
    }, timeout: t(1));

    test('version 3 is refused', () async {
      // The handler rejects V 3 while it initialises, before the keep checks.
      final error = await refusal(keptEncryptionPdf(encrypt: {'V': '3'}));
      expect(error, isA<PdfUnsupported>());
      expect(error.message, contains('V=3'));
    }, timeout: t(1));

    test(
      'strings and streams on different crypt filters are refused',
      () async {
        await expectRefused(
          keptEncryptionPdf(encrypt: {'StrF': '/Identity'}),
          'strings and streams use different crypt filters',
        );
      },
      timeout: t(1),
    );

    test('identity streams are refused', () async {
      await expectRefused(
        keptEncryptionPdf(encrypt: {'StmF': '/Identity'}),
        'streams use /Identity',
      );
    }, timeout: t(1));

    test('embedded files on another crypt filter are refused', () async {
      await expectRefused(
        keptEncryptionPdf(encrypt: {'EFF': '/Identity'}),
        'embedded files use a different crypt filter',
      );
    }, timeout: t(1));

    test('a crypt method that is not the version\'s is refused', () async {
      await expectRefused(
        keptEncryptionPdf(cfm: 'AESV2'),
        'crypt filter method is not AESV3',
      );
    }, timeout: t(1));

    test('a missing /ID is refused', () async {
      await expectRefused(keptEncryptionPdf(id: ''), 'no two-string /ID');
    }, timeout: t(1));

    test('a one-element /ID is refused', () async {
      await expectRefused(
        keptEncryptionPdf(id: '/ID [<11111111111111111111111111111111>]'),
        'no two-string /ID',
      );
    }, timeout: t(1));

    test('an /ID that is not strings is refused', () async {
      await expectRefused(
        keptEncryptionPdf(id: '/ID [1 2]'),
        'not two strings',
      );
    }, timeout: t(1));
  });
}
