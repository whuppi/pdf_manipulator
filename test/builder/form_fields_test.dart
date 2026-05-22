import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../helpers/memory_io.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('PdfPageBuilder form fields', () {
    test('textField creates valid PDF', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.textField('name', 72, 700, 200, 20);
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
      expect(info.pageCount, 1);
    });

    test('textField with default value', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.textField('email', 72, 700, 200, 20,
          defaultValue: 'user@example.com');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('checkbox creates valid PDF', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.checkbox('agree', 72, 700, 20, 20);
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
    });

    test('checkbox with checked=true', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.checkbox('agree', 72, 700, 20, 20, checked: true);
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('comboBox creates valid PDF', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.comboBox('country', 72, 700, 200, 20,
          ['US', 'UK', 'IN']);
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
    });

    test('comboBox with selected value', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.comboBox('country', 72, 700, 200, 20,
          ['US', 'UK', 'IN'], selected: 'UK');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('pushButton creates valid PDF', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.pushButton('submit', 72, 700, 100, 30, 'Submit');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
    });

    test('signatureField creates valid PDF', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.signatureField('sig', 72, 700, 200, 50);
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
    });

    test('multiple form fields on one page', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.textField('name', 72, 750, 200, 20);
      await page.textField('email', 72, 720, 200, 20);
      await page.checkbox('terms', 72, 690, 20, 20);
      await page.comboBox('role', 72, 660, 200, 20, ['Admin', 'User']);
      await page.pushButton('submit', 72, 620, 100, 30, 'Submit');
      await page.signatureField('sig', 72, 560, 200, 50);
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final doc = await pdf.open(sourceOf(bytes));
      expect(doc.pageCount, 1);
    });
  });
}
