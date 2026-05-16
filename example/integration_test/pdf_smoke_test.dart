// Integration test — runs on real Android emulator / iOS simulator.
// Verifies the native library loads, links, and produces correct results
// on mobile platforms where `dart test` can't run.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
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
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  testWidgets('probe returns valid info', (tester) async {
    final info = await pdf.probe(_minimalPdf);
    expect(info.isValid, isTrue);
    expect(info.pageCount, 1);
  });

  testWidgets('open returns correct page count', (tester) async {
    final doc = await pdf.open(_minimalPdf);
    expect(doc.pageCount, 1);
  });

  testWidgets('merge two PDFs', (tester) async {
    final merged = await pdf.merge([_minimalPdf, _minimalPdf]);
    expect(merged.length, greaterThan(_minimalPdf.length));
    final doc = await pdf.open(merged);
    expect(doc.pageCount, 2);
  });

  testWidgets('rotate produces output', (tester) async {
    final result = await pdf.rotateAllPages(_minimalPdf, degrees: 90);
    expect(result.length, greaterThan(0));
  });

  testWidgets('extractText returns string', (tester) async {
    final text = await pdf.extractText(_minimalPdf);
    expect(text, isA<String>());
  });

  testWidgets('compress produces output', (tester) async {
    final result = await pdf.compress(_minimalPdf);
    expect(result.length, greaterThan(0));
  });

  testWidgets('watermark produces larger output', (tester) async {
    final result = await pdf.watermark(_minimalPdf, text: 'TEST');
    expect(result.length, greaterThan(_minimalPdf.length));
  });

  testWidgets('encrypt produces output', (tester) async {
    final result = await pdf.encrypt(_minimalPdf, ownerPassword: 'test');
    expect(result.length, greaterThan(0));
  });

  testWidgets('editor opens and saves', (tester) async {
    final editor = await Pdf.edit(_minimalPdf);
    expect(await editor.pageCount, 1);
    final result = await editor.save();
    editor.dispose();
    expect(result.length, greaterThan(0));
  });

  testWidgets('builder creates PDF', (tester) async {
    final builder = await Pdf.build();
    final page = await builder.addA4Page();
    await page.font('Helvetica', 14);
    await page.text('Hello from mobile!');
    await page.done();
    final bytes = await builder.save();
    builder.dispose();

    final doc = await pdf.open(bytes);
    expect(doc.pageCount, 1);
  });

  testWidgets('kill prevents further operations', (tester) async {
    final p = Pdf();
    p.dispose();
    expect(() => p.probe(_minimalPdf), throwsStateError);
  });
}
