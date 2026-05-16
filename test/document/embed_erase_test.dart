import 'dart:typed_data';

import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.kill();
  });

  group('Pdf.embedFile', () {
    test('embeds a file and produces larger output', () async {
      final attachment = Uint8List.fromList('Hello world attachment'.codeUnits);
      final result = await pdf.embedFile(
        minimalPdf,
        name: 'hello.txt',
        fileData: attachment,
      );
      expect(result.length, greaterThan(minimalPdf.length));
    });

    test('embedded file PDF is still valid', () async {
      final attachment = Uint8List.fromList('Test data'.codeUnits);
      final result = await pdf.embedFile(
        minimalPdf,
        name: 'test.txt',
        fileData: attachment,
      );
      final info = await pdf.probe(result);
      expect(info.isValid, isTrue);
      expect(info.pageCount, 1);
    });

    test('can embed into multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final attachment = Uint8List.fromList(List.generate(1000, (i) => i % 256));
      final result = await pdf.embedFile(
        threePages,
        name: 'data.bin',
        fileData: attachment,
      );
      final doc = await pdf.open(result);
      expect(doc.pageCount, 3);
    });
  });

  group('Pdf.eraseRegions', () {
    test('erase region produces valid output', () async {
      final result = await pdf.eraseRegions(
        minimalPdf,
        page: 0,
        regions: [const PdfRect(x: 10, y: 10, width: 100, height: 100)],
      );
      expect(result.length, greaterThan(0));
      final info = await pdf.probe(result);
      expect(info.isValid, isTrue);
    });

    test('erase multiple regions on same page', () async {
      final result = await pdf.eraseRegions(
        minimalPdf,
        page: 0,
        regions: [
          const PdfRect(x: 0, y: 0, width: 50, height: 50),
          const PdfRect(x: 100, y: 100, width: 200, height: 200),
          const PdfRect(x: 300, y: 300, width: 100, height: 100),
        ],
      );
      expect(result.length, greaterThan(0));
    });

    test('erase on multi-page PDF preserves page count', () async {
      final threePages = await buildThreePagePdf();
      final result = await pdf.eraseRegions(
        threePages,
        page: 1,
        regions: [const PdfRect(x: 0, y: 0, width: 200, height: 200)],
      );
      final doc = await pdf.open(result);
      expect(doc.pageCount, 3);
    });
  });

  group('PdfEditor.embedFile', () {
    test('embed via editor and save', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.embedFile('readme.txt', Uint8List.fromList('Read me'.codeUnits));
      final result = await editor.save();
      await editor.dispose();
      expect(result.length, greaterThan(minimalPdf.length));
    });
  });

  group('PdfEditor.eraseRegions', () {
    test('erase via editor and save', () async {
      final editor = PdfEditor(await pdf.openEditor(minimalPdf));
      await editor.eraseRegions(0, [const PdfRect(x: 10, y: 10, width: 50, height: 50)]);
      final result = await editor.save();
      await editor.dispose();
      expect(result.length, greaterThan(0));
    });
  });
}
