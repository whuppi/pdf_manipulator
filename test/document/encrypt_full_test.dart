import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.encryptFull', () {
    test('encrypted output differs from input', () async {
      final sink = TestPdfSink();
      await pdf.encryptFull(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'owner123',
      );
      final encrypted = sink.takeBytes();
      expect(encrypted.length, isNot(equals(minimalPdf.length)));
    });

    test('encrypted output is larger (encryption overhead)', () async {
      final sink = TestPdfSink();
      await pdf.encryptFull(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'secret',
      );
      final encrypted = sink.takeBytes();
      expect(encrypted.length, greaterThan(minimalPdf.length));
    });

    test('encrypted PDF opens with correct password', () async {
      final sink = TestPdfSink();
      await pdf.encryptFull(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'correct',
      );
      final encrypted = sink.takeBytes();
      final doc = await pdf.open(sourceOf(encrypted), password: 'correct');
      expect(doc.pageCount, equals(1));
    });

    test('encrypted with both passwords', () async {
      final sink = TestPdfSink();
      await pdf.encryptFull(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      final encrypted = sink.takeBytes();
      final doc = await pdf.open(sourceOf(encrypted), password: 'owner');
      expect(doc.pageCount, equals(1));
    });

    test('custom algorithm produces valid PDF', () async {
      final sink = TestPdfSink();
      await pdf.encryptFull(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'pw',
        algorithm: 2, // AES_128
      );
      final encrypted = sink.takeBytes();
      expect(encrypted.length, greaterThan(minimalPdf.length));
      final doc = await pdf.open(sourceOf(encrypted), password: 'pw');
      expect(doc.pageCount, equals(1));
    });

    test('restrictive permissions produce valid PDF', () async {
      final sink = TestPdfSink();
      await pdf.encryptFull(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'pw',
        allowPrint: false,
        allowCopy: false,
        allowModify: false,
      );
      final encrypted = sink.takeBytes();
      expect(encrypted.length, greaterThan(minimalPdf.length));
      final doc = await pdf.open(sourceOf(encrypted), password: 'pw');
      expect(doc.pageCount, equals(1));
    });
  });

  group('PdfEditor.saveEncryptedFull', () {
    test('encrypted save produces larger output', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final encSink = TestPdfSink();
      await editor.saveEncryptedFull(encSink, ownerPassword: 'pw');
      final encrypted = encSink.takeBytes();
      final normSink = TestPdfSink();
      await editor.save(normSink);
      final normal = normSink.takeBytes();
      expect(encrypted.length, greaterThan(normal.length));
      editor.dispose();
    });

    test('modify then encrypt preserves modifications', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(minimalPdf));
      final sink = TestPdfSink();
      await editor.saveEncryptedFull(sink,
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      editor.dispose();

      final encrypted = sink.takeBytes();
      final doc = await pdf.open(sourceOf(encrypted), password: 'owner');
      expect(doc.pageCount, equals(2));
    });

    test('restrictive permissions via editor', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final sink = TestPdfSink();
      await editor.saveEncryptedFull(sink,
        ownerPassword: 'pw',
        allowPrint: false,
        allowCopy: false,
        algorithm: 3,
      );
      editor.dispose();
      final encrypted = sink.takeBytes();
      expect(encrypted.length, greaterThan(minimalPdf.length));
    });
  });
}
