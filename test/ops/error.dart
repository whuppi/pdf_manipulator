// Error — garbage input, empty input, recovery after failure.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/test_source_sink.dart';

void registerErrorTests(Pdf Function() createPdf) {
  group('error', () {
    test('open garbage bytes throws', () async {
      expect(
        () => createPdf().open(TestSource(Uint8List.fromList([1, 2, 3, 4]))),
        throwsA(anything),
      );
    });

    test('open empty bytes throws', () async {
      expect(
        () => createPdf().open(TestSource(Uint8List(0))),
        throwsA(anything),
      );
    });

    test('recover after error — next op works', () async {
      final pdf = createPdf();
      try {
        await pdf.open(TestSource(Uint8List.fromList([0xFF, 0xFE])));
      } catch (_) {}

      final doc = await pdf.open(TestSource(Uint8List.fromList(
        '%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n'
        '2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n'
        '3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]>>endobj\n'
        'xref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n'
        '0000000058 00000 n \n0000000115 00000 n \n'
        'trailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF'
            .codeUnits,
      )));
      expect(doc.pageCount, 1);
    });
  });
}
