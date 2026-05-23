// Integration test — runs on real Android emulator / iOS simulator.
// Verifies the native library loads, links, and produces correct results
// on mobile platforms where `dart test` can't run.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

class _Source implements PdfSource {
  _Source(this._data);
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

class _Sink implements PdfSink {
  final _builder = BytesBuilder(copy: false);
  @override
  void write(Uint8List chunk) => _builder.add(chunk);
  Uint8List takeBytes() => _builder.takeBytes();
}

PdfSource _src(Uint8List bytes) => _Source(bytes);

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

  setUp(() => pdf = Pdf());
  tearDown(() => pdf.dispose());

  testWidgets('open returns correct page count', (tester) async {
    final doc = await pdf.open(_src(_minimalPdf));
    expect(doc.pageCount, 1);
  });

  testWidgets('merge two PDFs', (tester) async {
    final sink = _Sink();
    await pdf.merge([_src(_minimalPdf), _src(_minimalPdf)], sink);
    final merged = sink.takeBytes();
    expect(merged.length, greaterThan(_minimalPdf.length));
    final doc = await pdf.open(_src(merged));
    expect(doc.pageCount, 2);
  });

  testWidgets('rotate produces output', (tester) async {
    final sink = _Sink();
    await pdf.rotateAllPages(_src(_minimalPdf), sink, degrees: 90);
    expect(sink.takeBytes().length, greaterThan(0));
  });

  testWidgets('extract text returns string', (tester) async {
    final text = await pdf.extract(_src(_minimalPdf), pages: const PdfPages.all());
    expect(text, isA<String>());
  });

  testWidgets('compress produces output', (tester) async {
    final sink = _Sink();
    await pdf.compress(_src(_minimalPdf), sink);
    expect(sink.takeBytes().length, greaterThan(0));
  });

  testWidgets('watermark produces output', (tester) async {
    final sink = _Sink();
    await pdf.watermark(_src(_minimalPdf), sink, text: 'TEST');
    expect(sink.takeBytes().length, greaterThan(_minimalPdf.length));
  });

  testWidgets('encrypt produces output', (tester) async {
    final sink = _Sink();
    await pdf.encrypt(_src(_minimalPdf), sink,
        encryption: const PdfEncryptionConfig(ownerPassword: 'test'));
    expect(sink.takeBytes().length, greaterThan(0));
  });

  testWidgets('editor opens and saves', (tester) async {
    final editor = await pdf.edit(_src(_minimalPdf));
    expect(await editor.pageCount, 1);
    final sink = _Sink();
    await editor.save(sink);
    await editor.dispose();
    expect(sink.takeBytes().length, greaterThan(0));
  });

  testWidgets('builder creates PDF', (tester) async {
    final builder = await pdf.build();
    final page = await builder.addA4Page();
    await page.font('Helvetica', 14);
    await page.text('Hello from mobile!');
    await page.done();
    final sink = _Sink();
    await builder.save(sink);
    await builder.dispose();

    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
  });

  testWidgets('dispose prevents further operations', (tester) async {
    final p = Pdf();
    await p.dispose();
    expect(() => p.open(_src(_minimalPdf)), throwsStateError);
  });
}
