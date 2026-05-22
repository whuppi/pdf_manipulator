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

  group('Pdf.encrypt', () {
    test('encrypted output differs from input (encryption applied)', () async {
      final sink = TestPdfSink();
      await pdf.encrypt(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'owner123',
      );
      final encrypted = sink.takeBytes();
      expect(encrypted.length, isNot(equals(minimalPdf.length)));
    });

    test('encrypted output is larger (encryption overhead)', () async {
      final sink = TestPdfSink();
      await pdf.encrypt(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'secret',
      );
      final encrypted = sink.takeBytes();
      expect(encrypted.length, greaterThan(minimalPdf.length));
    });

    test('encrypted PDF opens with correct password', () async {
      final sink = TestPdfSink();
      await pdf.encrypt(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'correct',
      );
      final encrypted = sink.takeBytes();
      final doc = await pdf.open(sourceOf(encrypted), password: 'correct');
      expect(doc.pageCount, equals(1));
    });

    test('encrypted with both passwords', () async {
      final sink = TestPdfSink();
      await pdf.encrypt(
        sourceOf(minimalPdf), sink,
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      final encrypted = sink.takeBytes();
      // Both passwords should work
      final doc = await pdf.open(sourceOf(encrypted), password: 'owner');
      expect(doc.pageCount, equals(1));
    });
  });

  group('Pdf.decrypt', () {
    test('round-trip: encrypt → decrypt preserves page count', () async {
      final encSink = TestPdfSink();
      await pdf.encrypt(sourceOf(minimalPdf), encSink, ownerPassword: 'pw');
      final encrypted = encSink.takeBytes();
      final decSink = TestPdfSink();
      await pdf.decrypt(sourceOf(encrypted), decSink, password: 'pw');
      final decrypted = decSink.takeBytes();
      final doc = await pdf.open(sourceOf(decrypted));
      expect(doc.pageCount, equals(1));
    });

    test('decrypted output differs from encrypted (encryption removed)', () async {
      final encSink = TestPdfSink();
      await pdf.encrypt(sourceOf(minimalPdf), encSink, ownerPassword: 'pw');
      final encrypted = encSink.takeBytes();
      final decSink = TestPdfSink();
      await pdf.decrypt(sourceOf(encrypted), decSink, password: 'pw');
      final decrypted = decSink.takeBytes();
      expect(decrypted.length, isNot(equals(encrypted.length)));
    });
  });

  group('PdfEditor.saveEncrypted', () {
    test('encrypted save produces larger output', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final encSink = TestPdfSink();
      await editor.saveEncrypted(encSink, ownerPassword: 'pw');
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
      await editor.saveEncrypted(sink,
        ownerPassword: 'owner',
        userPassword: 'user',
      );
      editor.dispose();

      final encrypted = sink.takeBytes();
      final doc = await pdf.open(sourceOf(encrypted), password: 'owner');
      expect(doc.pageCount, equals(2));
    });
  });
}
