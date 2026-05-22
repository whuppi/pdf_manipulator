// Integration test — runs on real Android emulator / iOS simulator.
// Verifies the native library loads, links, and produces correct results
// on mobile platforms where `dart test` can't run.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

// Inline test helpers — example can't import from test/helpers/
class _TestPdfSource implements PdfSource {
  _TestPdfSource(this._data);
  final Uint8List _data;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) {
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.sublistView(_data, offset, end);
  }
}

class _TestPdfSink implements PdfSink {
  final _builder = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _builder.add(chunk);

  Uint8List takeBytes() => _builder.takeBytes();
}

PdfSource _sourceOf(Uint8List bytes) => _TestPdfSource(bytes);

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
    final info = await pdf.probe(_sourceOf(_minimalPdf));
    expect(info.isValid, isTrue);
    expect(info.pageCount, 1);
  });

  testWidgets('open returns correct page count', (tester) async {
    final doc = await pdf.open(_sourceOf(_minimalPdf));
    expect(doc.pageCount, 1);
  });

  testWidgets('merge two PDFs', (tester) async {
    final sink = _TestPdfSink();
    await pdf.merge([_sourceOf(_minimalPdf), _sourceOf(_minimalPdf)], sink);
    final merged = sink.takeBytes();
    expect(merged.length, greaterThan(_minimalPdf.length));
    final doc = await pdf.open(_sourceOf(merged));
    expect(doc.pageCount, 2);
  });

  testWidgets('rotate produces output', (tester) async {
    final sink = _TestPdfSink();
    await pdf.rotateAllPages(_sourceOf(_minimalPdf), sink, degrees: 90);
    final result = sink.takeBytes();
    expect(result.length, greaterThan(0));
  });

  testWidgets('extractText returns string', (tester) async {
    final text = await pdf.extractText(_sourceOf(_minimalPdf));
    expect(text, isA<String>());
  });

  testWidgets('compress produces output', (tester) async {
    final sink = _TestPdfSink();
    await pdf.compress(_sourceOf(_minimalPdf), sink);
    final result = sink.takeBytes();
    expect(result.length, greaterThan(0));
  });

  testWidgets('watermark produces larger output', (tester) async {
    final sink = _TestPdfSink();
    await pdf.watermark(_sourceOf(_minimalPdf), sink, text: 'TEST');
    final result = sink.takeBytes();
    expect(result.length, greaterThan(_minimalPdf.length));
  });

  testWidgets('encrypt produces output', (tester) async {
    final sink = _TestPdfSink();
    await pdf.encrypt(_sourceOf(_minimalPdf), sink, ownerPassword: 'test');
    final result = sink.takeBytes();
    expect(result.length, greaterThan(0));
  });

  testWidgets('editor opens and saves', (tester) async {
    final editor = await Pdf.edit(_sourceOf(_minimalPdf));
    expect(await editor.pageCount, 1);
    final sink = _TestPdfSink();
    await editor.save(sink);
    editor.dispose();
    final result = sink.takeBytes();
    expect(result.length, greaterThan(0));
  });

  testWidgets('builder creates PDF', (tester) async {
    final builder = await Pdf.build();
    final page = await builder.addA4Page();
    await page.font('Helvetica', 14);
    await page.text('Hello from mobile!');
    await page.done();
    final sink = _TestPdfSink();
    await builder.save(sink);
    builder.dispose();

    final bytes = sink.takeBytes();
    final doc = await pdf.open(_sourceOf(bytes));
    expect(doc.pageCount, 1);
  });

  testWidgets('kill prevents further operations', (tester) async {
    final p = Pdf();
    p.dispose();
    expect(() => p.probe(_sourceOf(_minimalPdf)), throwsStateError);
  });
}
