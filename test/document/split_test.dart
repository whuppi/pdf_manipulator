import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/pdf_fixtures.dart';

Future<Uint8List> _fivePages() async {
  final pdf = Pdf();
  final editor = PdfEditor(await pdf.openEditor(minimalPdf));
  for (var i = 0; i < 4; i++) {
    await editor.mergeFrom(minimalPdf);
  }
  final result = await editor.save();
  await editor.dispose();
  pdf.kill();
  return result;
}

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.kill();
  });

  group('Pdf.split', () {
    test('splits 5-page PDF by 2 pages each', () async {
      final chunks = await pdf.split(await _fivePages(), every: 2);
      expect(chunks, hasLength(3)); // [2, 2, 1]

      final doc1 = await pdf.open(chunks[0]);
      expect(doc1.pageCount, equals(2));

      final doc2 = await pdf.open(chunks[1]);
      expect(doc2.pageCount, equals(2));

      final doc3 = await pdf.open(chunks[2]);
      expect(doc3.pageCount, equals(1));
    });

    test('splits 5-page PDF by 1 page each', () async {
      final chunks = await pdf.split(await _fivePages(), every: 1);
      expect(chunks, hasLength(5));
      for (final chunk in chunks) {
        final doc = await pdf.open(chunk);
        expect(doc.pageCount, equals(1));
      }
    });

    test('split with every >= pageCount returns single chunk', () async {
      final chunks = await pdf.split(await _fivePages(), every: 10);
      expect(chunks, hasLength(1));
      final doc = await pdf.open(chunks[0]);
      expect(doc.pageCount, equals(5));
    });

    test('split with every == pageCount returns single chunk', () async {
      final chunks = await pdf.split(await _fivePages(), every: 5);
      expect(chunks, hasLength(1));
      final doc = await pdf.open(chunks[0]);
      expect(doc.pageCount, equals(5));
    });

    test('each chunk is a re-openable valid PDF', () async {
      final chunks = await pdf.split(await _fivePages(), every: 2);
      for (final chunk in chunks) {
        final doc = await pdf.open(chunk);
        expect(doc.pageCount, greaterThan(0));
      }
    });

    test('throws on every < 1', () async {
      final bytes = await _fivePages();
      expect(
        () => pdf.split(bytes, every: 0),
        throwsArgumentError,
      );
    });
  });

  group('Pdf.extractPages', () {
    test('extracts single page', () async {
      final result = await pdf.extractPages(await _fivePages(), pages: [2]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(1));
    });

    test('extracts multiple pages', () async {
      final result =
          await pdf.extractPages(await _fivePages(), pages: [0, 2, 4]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(3));
    });

    test('extracts all pages', () async {
      final result =
          await pdf.extractPages(await _fivePages(), pages: [0, 1, 2, 3, 4]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(5));
    });

    test('extracted pages form re-openable PDF', () async {
      final result = await pdf.extractPages(await _fivePages(), pages: [1, 3]);
      final doc = await pdf.open(result);
      expect(doc.pageCount, equals(2));
    });
  });

  group('PdfEditor.extractPages', () {
    test('extracts pages via editor', () async {
      final editor = PdfEditor(await pdf.openEditor(await _fivePages()));
      final extracted = await editor.extractPages([0, 2, 4]);
      await editor.dispose();

      final doc = await pdf.open(extracted);
      expect(doc.pageCount, equals(3));
    });
  });
}
