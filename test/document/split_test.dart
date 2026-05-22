import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

Future<Uint8List> _fivePages() async {
  final pdf = Pdf();
  final editor = await Pdf.edit(sourceOf(minimalPdf));
  for (var i = 0; i < 4; i++) {
    await editor.mergeFrom(sourceOf(minimalPdf));
  }
  final sink = TestPdfSink();
  await editor.save(sink);
  editor.dispose();
  pdf.dispose();
  return sink.takeBytes();
}

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.split', () {
    test('splits 5-page PDF by 2 pages each', () async {
      final sinks = <TestPdfSink>[];
      await pdf.split(sourceOf(await _fivePages()), (i) {
        final s = TestPdfSink();
        sinks.add(s);
        return s;
      }, every: 2);
      final chunks = sinks.map((s) => s.takeBytes()).toList();
      expect(chunks, hasLength(3)); // [2, 2, 1]

      final doc1 = await pdf.open(sourceOf(chunks[0]));
      expect(doc1.pageCount, equals(2));

      final doc2 = await pdf.open(sourceOf(chunks[1]));
      expect(doc2.pageCount, equals(2));

      final doc3 = await pdf.open(sourceOf(chunks[2]));
      expect(doc3.pageCount, equals(1));
    });

    test('splits 5-page PDF by 1 page each', () async {
      final sinks = <TestPdfSink>[];
      await pdf.split(sourceOf(await _fivePages()), (i) {
        final s = TestPdfSink();
        sinks.add(s);
        return s;
      }, every: 1);
      final chunks = sinks.map((s) => s.takeBytes()).toList();
      expect(chunks, hasLength(5));
      for (final chunk in chunks) {
        final doc = await pdf.open(sourceOf(chunk));
        expect(doc.pageCount, equals(1));
      }
    });

    test('split with every >= pageCount returns single chunk', () async {
      final sinks = <TestPdfSink>[];
      await pdf.split(sourceOf(await _fivePages()), (i) {
        final s = TestPdfSink();
        sinks.add(s);
        return s;
      }, every: 10);
      final chunks = sinks.map((s) => s.takeBytes()).toList();
      expect(chunks, hasLength(1));
      final doc = await pdf.open(sourceOf(chunks[0]));
      expect(doc.pageCount, equals(5));
    });

    test('split with every == pageCount returns single chunk', () async {
      final sinks = <TestPdfSink>[];
      await pdf.split(sourceOf(await _fivePages()), (i) {
        final s = TestPdfSink();
        sinks.add(s);
        return s;
      }, every: 5);
      final chunks = sinks.map((s) => s.takeBytes()).toList();
      expect(chunks, hasLength(1));
      final doc = await pdf.open(sourceOf(chunks[0]));
      expect(doc.pageCount, equals(5));
    });

    test('each chunk is a re-openable valid PDF', () async {
      final sinks = <TestPdfSink>[];
      await pdf.split(sourceOf(await _fivePages()), (i) {
        final s = TestPdfSink();
        sinks.add(s);
        return s;
      }, every: 2);
      for (final s in sinks) {
        final doc = await pdf.open(sourceOf(s.takeBytes()));
        expect(doc.pageCount, greaterThan(0));
      }
    });

    test('throws on every < 1', () async {
      final bytes = await _fivePages();
      expect(
        () => pdf.split(sourceOf(bytes), (_) => TestPdfSink(), every: 0),
        throwsArgumentError,
      );
    });
  });

  group('Pdf.extractPages', () {
    test('extracts single page', () async {
      final sink = TestPdfSink();
      await pdf.extractPages(sourceOf(await _fivePages()), sink, pages: [2]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(1));
    });

    test('extracts multiple pages', () async {
      final sink = TestPdfSink();
      await pdf.extractPages(sourceOf(await _fivePages()), sink,
          pages: [0, 2, 4]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(3));
    });

    test('extracts all pages', () async {
      final sink = TestPdfSink();
      await pdf.extractPages(sourceOf(await _fivePages()), sink,
          pages: [0, 1, 2, 3, 4]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(5));
    });

    test('extracted pages form re-openable PDF', () async {
      final sink = TestPdfSink();
      await pdf.extractPages(sourceOf(await _fivePages()), sink,
          pages: [1, 3]);
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(2));
    });
  });

  group('PdfEditor.extractPages', () {
    test('extracts pages via editor', () async {
      final editor = await Pdf.edit(sourceOf(await _fivePages()));
      final sink = TestPdfSink();
      await editor.extractPages([0, 2, 4], sink);
      editor.dispose();

      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(3));
    });
  });
}
