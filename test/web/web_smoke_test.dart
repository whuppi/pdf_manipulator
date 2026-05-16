@TestOn('browser')
library;

import 'dart:typed_data';

import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

final _minimalPdf = Uint8List.fromList(
  '%PDF-1.4\n'
  '1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n'
  '2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n'
  '3 0 obj\n<< /Type /Page /Parent 2 0 R '
  '/MediaBox [0 0 595 842] >>\nendobj\n'
  'xref\n0 4\n'
  '0000000000 65535 f \n'
  '0000000009 00000 n \n'
  '0000000058 00000 n \n'
  '0000000115 00000 n \n'
  'trailer\n<< /Size 4 /Root 1 0 R >>\n'
  'startxref\n190\n%%EOF\n'
      .codeUnits,
);

void main() {
  late Pdf pdf;
  late int serverPort;

  setUpAll(() async {
    final channel = spawnHybridUri('asset_server.dart');
    serverPort = await channel.stream.first as int;
  });

  setUp(() {
    pdf = Pdf();
    pdf.configureWorkerUrl(
        'http://localhost:$serverPort/web_assets/worker.js');
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Web: Pdf.probe', () {
    test('returns valid for minimal PDF', () async {
      final info = await pdf.probe(_minimalPdf);
      expect(info.isValid, isTrue);
      expect(info.pageCount, 1);
    });
  });

  group('Web: Pdf.open', () {
    test('returns PdfDoc with correct page count', () async {
      final doc = await pdf.open(_minimalPdf);
      expect(doc.pageCount, 1);
    });
  });

  group('Web: Pdf.merge', () {
    test('merges two PDFs', () async {
      final merged = await pdf.merge([_minimalPdf, _minimalPdf]);
      expect(merged.length, greaterThan(_minimalPdf.length));
      final doc = await pdf.open(merged);
      expect(doc.pageCount, 2);
    });
  });

  group('Web: Pdf.rotateAllPages', () {
    test('rotates and produces output', () async {
      final result = await pdf.rotateAllPages(_minimalPdf, degrees: 90);
      expect(result.length, greaterThan(0));
    });
  });

  group('Web: Pdf.extractText', () {
    test('returns string', () async {
      final text = await pdf.extractText(_minimalPdf);
      expect(text, isA<String>());
    });
  });

  group('Web: Pdf.compress', () {
    test('produces output', () async {
      final result = await pdf.compress(_minimalPdf);
      expect(result.length, greaterThan(0));
    });
  });

  group('Web: Pdf.watermark', () {
    test('produces larger output', () async {
      final result = await pdf.watermark(_minimalPdf, text: 'TEST');
      expect(result.length, greaterThan(_minimalPdf.length));
    });
  });

  group('Web: Pdf.encrypt', () {
    test('produces output', () async {
      final result =
          await pdf.encrypt(_minimalPdf, ownerPassword: 'test');
      expect(result.length, greaterThan(0));
    });
  });
}
