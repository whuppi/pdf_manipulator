// Open — inspect PDF metadata, pages, version, encryption status.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/transport/bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerOpenTests(PdfBridge Function() b) {
  group('open', () {
    test('minimal PDF — pageCount 1', () async {
      final doc = await b().open(src(minimalPdf));
      expect(doc.pageCount, 1);
    });

    test('version string contains dot', () async {
      final doc = await b().open(src(minimalPdf));
      expect(doc.version, contains('.'));
    });

    test('page dimensions are positive', () async {
      final doc = await b().open(src(minimalPdf));
      expect(doc.pages, hasLength(1));
      expect(doc.pages[0].width, greaterThan(0));
      expect(doc.pages[0].height, greaterThan(0));
    });

    test('isEncrypted false for unencrypted', () async {
      final doc = await b().open(src(minimalPdf));
      expect(doc.isEncrypted, isFalse);
    });

    test('throws on garbage bytes', () async {
      expect(
        () => b().open(TestSource(Uint8List.fromList([1, 2, 3, 4]))),
        throwsA(anything),
      );
    });

    test('throws on empty bytes', () async {
      expect(
        () => b().open(TestSource(Uint8List(0))),
        throwsA(anything),
      );
    });

    test('letter-size PDF has different dimensions', () async {
      final doc = await b().open(src(letterPdf));
      expect(doc.pageCount, 1);
      expect(doc.pages[0].width, greaterThan(0));
    });
  });
}
