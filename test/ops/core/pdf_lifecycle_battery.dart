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
        pdf.open(PdfInput(TestSource(Uint8List.fromList([1, 2, 3, 4])))),
        throwsA(isA<PdfCorrupted>()),
        reason:
            'engine failures are PdfError — never StateError, which is '
            'reserved for use-after-dispose',
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('open empty bytes throws PdfCorrupted', () async {
      final pdf = createPdf();
      await expectLater(
        pdf.open(PdfInput(TestSource(Uint8List(0)))),
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
          PdfInput(TestSource(Uint8List(0))),
          PdfInput(TestSource(minimalPdf)),
        ], TestSink()),
        throwsA(isA<PdfCorrupted>()),
      );
      await pdf.dispose();
    }, timeout: t(1));

    test('recover after error — next op works', () async {
      final pdf = createPdf();
      try {
        await pdf.open(PdfInput(TestSource(Uint8List.fromList([0xFF, 0xFE]))));
      } catch (_) {}

      final doc = await pdf.open(PdfInput(TestSource(recoverProbePdf)));
      expect(doc.pageCount, 1);
      await doc.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    // ── Instance dispose ──

    test('instance dispose actually disposes — later ops refuse', () async {
      final pdf = createPdf();
      final doc = await pdf.open(PdfInput(src(minimalPdf)));
      expect(doc.pageCount, 1);
      await pdf.dispose();
      expect(
        () => pdf.open(PdfInput(src(minimalPdf))),
        throwsA(isA<StateError>()),
        reason:
            'dispose must leave the instance provably dead, '
            'not just return without error',
      );
    }, timeout: t(1));

    test('instance double dispose is safe and stays disposed', () async {
      final pdf = createPdf();
      await pdf.dispose();
      await pdf.dispose();
      expect(
        () => pdf.open(PdfInput(src(minimalPdf))),
        throwsA(isA<StateError>()),
      );
    }, timeout: t(1));

    test('operations after instance dispose throw', () async {
      final pdf = createPdf();
      await pdf.dispose();
      expect(
        () => pdf.open(PdfInput(src(minimalPdf))),
        throwsA(isA<StateError>()),
      );
    }, timeout: t(1));

    test('instance dispose returns quickly', () async {
      final pdf = createPdf();
      await pdf.open(PdfInput(src(minimalPdf)));
      final sw = Stopwatch()..start();
      await pdf.dispose();
      sw.stop();
      expect(sw.elapsedMilliseconds, lessThan(2000));
    }, timeout: t(1));

    // ── Doc handle dispose ──

    test('doc dispose kills the doc, not the instance', () async {
      final pdf = createPdf();
      final doc = await pdf.open(PdfInput(src(minimalPdf)));
      expect(doc.pageCount, 1);
      await doc.dispose();
      expect(
        () => doc.extractText(MemorySink()),
        throwsA(isA<StateError>()),
        reason: 'a disposed doc refuses ops',
      );
      final doc2 = await pdf.open(PdfInput(src(minimalPdf)));
      expect(doc2.pageCount, 1, reason: 'the instance survives a doc dispose');
      await doc2.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('doc double dispose is safe and stays disposed', () async {
      final pdf = createPdf();
      final doc = await pdf.open(PdfInput(src(minimalPdf)));
      await doc.dispose();
      await doc.dispose();
      expect(() => doc.extractText(MemorySink()), throwsA(isA<StateError>()));
      await pdf.dispose();
    }, timeout: t(1));

    // ── Editor handle lifecycle ──

    test('edit then dispose on a plain source', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(minimalPdf)));
      expect(await editor.pageCount(), 1);
      await editor.dispose();
      await editor.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    test('edit then dispose on an encrypted source', () async {
      final pdf = createPdf();
      final encryptor = await pdf.edit(PdfInput(src(minimalPdf)));
      final encSink = TestSink();
      await encryptor.save(
        encSink,
        save: const PdfSave.full(
          encryption: PdfEncryption.password(
            ownerPassword: 'pw',
            userPassword: 'pw',
          ),
        ),
      );
      await encryptor.dispose();
      final encrypted = encSink.takeBytes();

      final doc = await pdf.open(PdfInput(src(encrypted), password: 'pw'));
      expect(doc.encryption, isNotNull);
      await doc.dispose();

      final editor = await pdf.edit(PdfInput(src(encrypted), password: 'pw'));
      expect(await editor.pageCount(), 1);
      await editor.dispose();
      await editor.dispose();
      await pdf.dispose();
    }, timeout: t(1));

    // ── Keeping the source's encryption ──

    group('save keeps the source encryption', () {
      const printOnly = PdfPermissions(
        canPrint: true,
        canPrintHighQuality: false,
        canModify: false,
        canCopy: false,
        canAnnotate: false,
        canFillForms: false,
        canExtractForAccessibility: false,
        canAssemble: false,
      );

      Future<Uint8List> encrypted(
        Pdf pdf,
        PdfEncryptionAlgorithm algorithm,
      ) async {
        final sink = TestSink();
        await pdf.resave(
          PdfInput(src(minimalPdf)),
          sink,
          save: PdfSave.full(
            encryption: PdfEncryption.password(
              ownerPassword: 'owner-pw',
              userPassword: 'user-pw',
              algorithm: algorithm,
              permissions: printOnly,
            ),
          ),
        );
        return sink.takeBytes();
      }

      Future<Uint8List> keepSave(Pdf pdf, Uint8List source) async {
        final editor = await pdf.edit(
          PdfInput(src(source), password: 'user-pw'),
        );
        await editor.setMetadata(
          (await editor.metadata()).copyWith(title: 'kept'),
        );
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
        PdfEncryptionAlgorithm.aesKey256,
        PdfEncryptionAlgorithm.aesKey128,
        PdfEncryptionAlgorithm.rc4Key128,
      ]) {
        test(
          '${algorithm.name}: owner, user and permissions survive',
          () async {
            final pdf = createPdf();
            final source = await encrypted(pdf, algorithm);
            final out = await keepSave(pdf, source);

            final asUser = await pdf.open(
              PdfInput(src(out), password: 'user-pw'),
            );
            expect(asUser.encryption, isNotNull);
            expect(asUser.metadata.title, 'kept');
            final original = await pdf.open(
              PdfInput(src(source), password: 'user-pw'),
            );
            expect(
              asUser.encryption?.permissions.bits,
              original.encryption?.permissions.bits,
              reason: 'the permissions read back are the source\'s',
            );
            await original.dispose();
            await asUser.dispose();

            final asOwner = await pdf.open(
              PdfInput(src(out), password: 'owner-pw'),
            );
            expect(asOwner.metadata.title, 'kept');
            await asOwner.dispose();

            expect(
              entry(out, r'/P\s*(-?\d+)'),
              printOnly.bits.toString(),
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
        final source = await encrypted(pdf, PdfEncryptionAlgorithm.aesKey256);
        final editor = await pdf.edit(
          PdfInput(src(source), password: 'user-pw'),
        );
        final sink = TestSink();
        await editor.save(
          sink,
          save: const PdfSave.full(encryption: PdfEncryption.remove()),
        );
        await editor.dispose();
        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        expect(doc.encryption, isNull);
        await doc.dispose();
        await pdf.dispose();
      }, timeout: t(1));

      test('an explicit config replaces the passwords', () async {
        final pdf = createPdf();
        final source = await encrypted(pdf, PdfEncryptionAlgorithm.aesKey256);
        final editor = await pdf.edit(
          PdfInput(src(source), password: 'user-pw'),
        );
        final sink = TestSink();
        await editor.save(
          sink,
          save: const PdfSave.full(
            encryption: PdfEncryption.password(
              ownerPassword: 'new-owner',
              userPassword: 'new-user',
            ),
          ),
        );
        await editor.dispose();
        final out = sink.takeBytes();

        final doc = await pdf.open(PdfInput(src(out), password: 'new-user'));
        expect(doc.encryption, isNotNull);
        await doc.dispose();
        for (final old in ['user-pw', 'owner-pw']) {
          await expectLater(
            pdf.open(PdfInput(src(out), password: old)),
            throwsA(isA<PdfWrongPassword>()),
            reason: 'the old $old must not open the new file',
          );
        }
        await pdf.dispose();
      }, timeout: t(1));
    });
  });
}
