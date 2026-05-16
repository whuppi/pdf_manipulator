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

    test('PdfPageRangeError carries page and count in message', () {
      const e = PdfPageRangeError(page: 5, pageCount: 3);
      expect(e.page, equals(5));
      expect(e.pageCount, equals(3));
      expect(e.message, contains('5'));
      expect(e.message, contains('3'));
    });

    test('PdfInvalidArgument carries exact message', () {
      const e = PdfInvalidArgument('empty list');
      expect(e.message, equals('empty list'));
    });

    test('PdfUnsupported carries exact message', () {
      const e = PdfUnsupported('JBIG2');
      expect(e.message, equals('JBIG2'));
    });

    test('PdfIoError carries message and cause', () {
      final e = PdfIoError('disk full', cause: Exception('no space'));
      expect(e.message, equals('disk full'));
      expect(e.cause, isNotNull);
    });

    test('PdfExtractionFailed carries exact message', () {
      const e = PdfExtractionFailed('font missing');
      expect(e.message, equals('font missing'));
    });

    test('PdfSearchError carries exact message', () {
      const e = PdfSearchError('regex invalid');
      expect(e.message, equals('regex invalid'));
    });

    test('PdfCryptoError carries message and cause', () {
      final e = PdfCryptoError('AES failed', cause: Exception('key'));
      expect(e.message, equals('AES failed'));
      expect(e.cause, isNotNull);
    });

    test('PdfEngineError carries message and cause', () {
      final e = PdfEngineError('panic', cause: Exception('rust'));
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
      final desc = switch (e) {
        PdfCorrupted() => 'corrupted',
        PdfPasswordRequired() => 'password',
        PdfWrongPassword() => 'wrong',
        PdfPageRangeError() => 'range',
        PdfInvalidArgument() => 'invalid',
        PdfIoError() => 'io',
        PdfExtractionFailed() => 'extract',
        PdfUnsupported() => 'unsupported',
        PdfSearchError() => 'search',
        PdfCryptoError() => 'crypto',
        PdfEngineError() => 'engine',
      };
      expect(desc, equals('corrupted'));
    });
  });
}
