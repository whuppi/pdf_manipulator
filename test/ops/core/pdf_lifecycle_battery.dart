// Lifecycle — dispose, double dispose, error recovery, garbage input.
// Cross-cutting behavior that applies to all handle types.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerLifecycleTests(Pdf Function() createPdf) {
  group('lifecycle', () {
    // ── Error handling ──

    test('open garbage bytes throws the typed engine error', () async {
      final pdf = createPdf();
      await expectLater(
        pdf.open(TestSource(Uint8List.fromList([1, 2, 3, 4]))),
        throwsA(isA<PdfCorrupted>()),
        reason:
            'engine failures are PdfError — never a bare '
            'StateError, which is reserved for use-after-dispose',
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('open empty bytes throws PdfCorrupted', () async {
      final pdf = createPdf();
      await expectLater(
        pdf.open(TestSource(Uint8List(0))),
        throwsA(isA<PdfCorrupted>()),
        reason: 'an empty file reaches the parser and fails its header check',
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('an empty first source keeps the next one at its own index', () async {
      // A transport that drops an empty source slides the second
      // document into slot 0, and the merge writes it alone.
      final pdf = createPdf();
      await expectLater(
        pdf.merge([
          TestSource(Uint8List(0)),
          TestSource(minimalPdf),
        ], TestSink()),
        throwsA(isA<PdfCorrupted>()),
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('recover after error — next op works', () async {
      final pdf = createPdf();
      try {
        await pdf.open(TestSource(Uint8List.fromList([0xFF, 0xFE])));
      } catch (_) {}

      final doc = await pdf.open(TestSource(recoverProbePdf));
      expect(doc.pageCount, 1);
      await doc.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    // ── Instance dispose ──

    test('instance dispose actually disposes — later ops refuse', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await pdf.dispose();
      expect(
        () => pdf.open(src(minimalPdf)),
        throwsStateError,
        reason:
            'dispose must leave the instance provably dead, '
            'not just return without error',
      );
    }, timeout: t(1));

    test('instance double dispose is safe and stays disposed', () async {
      final pdf = createPdf();
      await pdf.dispose();
      await pdf.dispose();
      expect(() => pdf.open(src(minimalPdf)), throwsStateError);
    }, timeout: t(1));

    test('operations after instance dispose throw', () async {
      final pdf = createPdf();
      await pdf.dispose();
      expect(() => pdf.open(src(minimalPdf)), throwsStateError);
    }, timeout: t(1));

    test('instance dispose returns quickly', () async {
      final pdf = createPdf();
      await pdf.open(src(minimalPdf));
      final sw = Stopwatch()..start();
      await pdf.dispose();
      sw.stop();
      expect(sw.elapsedMilliseconds, lessThan(2000));
    }, timeout: t(1));

    // ── Doc handle dispose ──

    test('doc dispose kills the doc, not the instance', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await doc.dispose();
      expect(
        () => doc.extract(pages: const PdfPages.all()),
        throwsStateError,
        reason: 'a disposed doc refuses ops',
      );
      final doc2 = await pdf.open(src(minimalPdf));
      expect(doc2.pageCount, 1, reason: 'the instance survives a doc dispose');
      await doc2.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('doc double dispose is safe and stays disposed', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      await doc.dispose();
      await doc.dispose();
      expect(() => doc.extract(pages: const PdfPages.all()), throwsStateError);
      await pdf.dispose();
    }, timeout: t(1));

    // ── Editor handle lifecycle ──

    test('edit then dispose on a plain source', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      expect(await editor.pageCount, 1);
      await editor.dispose();
      await editor.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('edit then dispose on an encrypted source', () async {
      final pdf = createPdf();
      final encryptor = await pdf.edit(src(minimalPdf));
      final encSink = TestSink();
      await encryptor.save(
        encSink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.config(
            ownerPassword: 'pw',
            userPassword: 'pw',
          ),
        ),
      );
      await encryptor.dispose();
      final encrypted = encSink.takeBytes();

      final doc = await pdf.open(src(encrypted), password: 'pw');
      expect(doc.isEncrypted, isTrue);
      await doc.dispose();

      final editor = await pdf.edit(src(encrypted), password: 'pw');
      expect(await editor.pageCount, 1);
      await editor.dispose();
      await editor.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    // ── Keeping the source's encryption ──

    group('save keeps the source encryption', () {
      const printOnly = PdfPermissions(
        print: true,
        printHq: false,
        modify: false,
        copy: false,
        annotate: false,
        fillForms: false,
        accessibility: false,
        assemble: false,
      );

      Future<Uint8List> encrypted(
        Pdf pdf,
        PdfEncryptionAlgorithm algorithm,
      ) async {
        final sink = TestSink();
        await pdf.encrypt(
          src(minimalPdf),
          sink,
          encryption: PdfEncryptionConfig(
            ownerPassword: 'owner-pw',
            userPassword: 'user-pw',
            algorithm: algorithm,
            permissions: printOnly,
          ),
        );
        return sink.takeBytes();
      }

      Future<Uint8List> keepSave(Pdf pdf, Uint8List source) async {
        final editor = await pdf.edit(src(source), password: 'user-pw');
        await editor.setTitle('kept');
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        return sink.takeBytes();
      }

      // The encryption dictionary's own bytes: a reader here reports no
      // owner-versus-user authentication, so the claim that the user
      // password stays a user password is read off /O, /U and /ID.
      String entry(Uint8List pdfBytes, String pattern) {
        // bytegrep-exempt: the claim is about the encryption dictionary.
        final text = String.fromCharCodes(pdfBytes); // bytegrep-exempt
        final match = RegExp(pattern).firstMatch(text);
        expect(match, isNotNull, reason: 'no $pattern in the output');
        return match!.group(1)!;
      }

      for (final algorithm in [
        PdfEncryptionAlgorithm.aes256,
        PdfEncryptionAlgorithm.aes128,
        PdfEncryptionAlgorithm.rc4_128,
      ]) {
        test(
          '${algorithm.name}: owner, user and permissions survive',
          () async {
            final pdf = createPdf();
            final source = await encrypted(pdf, algorithm);
            final out = await keepSave(pdf, source);

            final asUser = await pdf.open(src(out), password: 'user-pw');
            expect(asUser.isEncrypted, isTrue);
            expect(asUser.title, 'kept');
            final original = await pdf.open(src(source), password: 'user-pw');
            expect(
              asUser.permissions?.toBits(),
              original.permissions?.toBits(),
              reason: 'the permissions read back are the source\'s',
            );
            await original.dispose();
            await asUser.dispose();

            final asOwner = await pdf.open(src(out), password: 'owner-pw');
            expect(asOwner.title, 'kept');
            await asOwner.dispose();

            expect(
              entry(out, r'/P\s*(-?\d+)'),
              printOnly.toBits().toString(),
              reason: '/P is the print-only permission word',
            );
            for (final key in ['O', 'U']) {
              final pattern = '/$key\\s*<([0-9A-Fa-f]+)>';
              expect(
                entry(out, pattern),
                entry(source, pattern),
                reason: '/$key must be the source\'s, so owner != user',
              );
            }
            const idPattern = r'/ID\s*\[\s*<([0-9A-Fa-f]+)>';
            expect(entry(out, idPattern), entry(source, idPattern));
            await pdf.dispose();
          },
          timeout: t(1),
        );
      }

      test('remove opens with no password', () async {
        final pdf = createPdf();
        final source = await encrypted(pdf, PdfEncryptionAlgorithm.aes256);
        final editor = await pdf.edit(src(source), password: 'user-pw');
        final sink = TestSink();
        await editor.save(
          sink,
          options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.remove(),
          ),
        );
        await editor.dispose();
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(doc.isEncrypted, isFalse);
        await doc.dispose();
        await pdf.dispose();
      }, timeout: t(1));

      test('an explicit config replaces the passwords', () async {
        final pdf = createPdf();
        final source = await encrypted(pdf, PdfEncryptionAlgorithm.aes256);
        final editor = await pdf.edit(src(source), password: 'user-pw');
        final sink = TestSink();
        await editor.save(
          sink,
          options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.config(
              ownerPassword: 'new-owner',
              userPassword: 'new-user',
            ),
          ),
        );
        await editor.dispose();
        final out = sink.takeBytes();

        final doc = await pdf.open(src(out), password: 'new-user');
        expect(doc.isEncrypted, isTrue);
        await doc.dispose();
        for (final old in ['user-pw', 'owner-pw']) {
          await expectLater(
            pdf.open(src(out), password: old),
            throwsA(isA<PdfWrongPassword>()),
            reason: 'the old $old must not open the new file',
          );
        }
        await pdf.dispose();
      }, timeout: t(1));
    });
  });
}
