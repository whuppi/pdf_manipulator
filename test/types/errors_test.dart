// PdfError sealed hierarchy — subtypes, messages, catch patterns.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void main() {
  group('PdfError hierarchy', () {
    test('PdfCorrupted carries message and cause', () {
      final cause = Exception('underlying');
      final e = PdfCorrupted('bad xref', cause: cause);
      expect(e.message, equals('bad xref'));
      expect(e.cause, equals(cause));
      expect(e.toString(), contains('PdfCorrupted'));
      expect(e.toString(), contains('bad xref'));
    });

    test('PdfPasswordRequired has descriptive message', () {
      const e = PdfPasswordRequired();
      expect(e.message, contains('password'));
    });

    test('PdfWrongPassword has descriptive message', () {
      const e = PdfWrongPassword();
      expect(e.message, contains('Incorrect'));
    });

    test('PdfPageOutOfRange carries page and count in message', () {
      const e = PdfPageOutOfRange(page: 5, pageCount: 3);
      expect(e.page, equals(5));
      expect(e.pageCount, equals(3));
      expect(e.message, contains('5'));
      expect(e.message, contains('3'));
    });

    test('PdfNotFound carries kind and name in message', () {
      const e = PdfNotFound(kind: 'attachment', name: 'a.txt');
      expect(e.kind, equals('attachment'));
      expect(e.name, equals('a.txt'));
      expect(e.message, equals('No attachment named "a.txt"'));
    });

    test('PdfIncrementalRefused carries exact message', () {
      const e = PdfIncrementalRefused('signed');
      expect(e.message, equals('signed'));
    });

    test('PdfCancelled has descriptive message', () {
      const e = PdfCancelled();
      expect(e.message, contains('cancelled'));
    });

    test('PdfUnsupported carries exact message', () {
      const e = PdfUnsupported('JBIG2');
      expect(e.message, equals('JBIG2'));
    });

    test('PdfIoFailed carries message and cause', () {
      final e = PdfIoFailed('disk full', cause: Exception('no space'));
      expect(e.message, equals('disk full'));
      expect(e.cause, isNotNull);
    });

    test('PdfCryptoFailed carries message and cause', () {
      final e = PdfCryptoFailed('AES failed', cause: Exception('key'));
      expect(e.message, equals('AES failed'));
      expect(e.cause, isNotNull);
    });

    test('PdfEngineFailed carries message and cause', () {
      final e = PdfEngineFailed('panic', cause: Exception('rust'));
      expect(e.message, equals('panic'));
      expect(e.cause, isNotNull);
    });

    test('catch PdfError catches any subtype', () {
      PdfError? caught;
      try {
        throw const PdfCorrupted('test');
      } on PdfError catch (e) {
        caught = e;
      }
      expect(caught, isNotNull);
      expect(caught, isA<PdfCorrupted>());
    });

    test('pattern-match exhaustive on sealed class', () {
      const PdfError e = PdfCorrupted('x');
      expect(describe(e), equals('corrupted'));
    });

    test('every subtype is matched by the exhaustive switch', () {
      final all = <PdfError>[
        const PdfIoFailed('a'),
        const PdfCorrupted('a'),
        const PdfEngineFailed('a'),
        const PdfPageOutOfRange(page: 1, pageCount: 1),
        const PdfNotFound(kind: 'k', name: 'n'),
        const PdfUnsupported('a'),
        const PdfPasswordRequired(),
        const PdfWrongPassword(),
        const PdfCryptoFailed('a'),
        const PdfIncrementalRefused('a'),
        const PdfCancelled(),
      ];
      expect(all.map(describe).toSet(), hasLength(11));
    });
  });
}

String describe(PdfError e) => switch (e) {
  PdfCorrupted() => 'corrupted',
  PdfPasswordRequired() => 'password',
  PdfWrongPassword() => 'wrong',
  PdfPageOutOfRange() => 'range',
  PdfNotFound() => 'not found',
  PdfIoFailed() => 'io',
  PdfUnsupported() => 'unsupported',
  PdfCryptoFailed() => 'crypto',
  PdfEngineFailed() => 'engine',
  PdfCancelled() => 'cancelled',
  PdfIncrementalRefused() => 'incremental refused',
};
