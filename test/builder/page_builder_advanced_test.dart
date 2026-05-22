import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('PdfPageBuilder advanced features', () {
    test('radioGroup creates a PDF with radio buttons', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.radioGroup(
        'color',
        ['red', 'green', 'blue'],
        [50, 50, 50],
        [700, 670, 640],
        [20, 20, 20],
        [20, 20, 20],
        selected: 'green',
      );
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
      expect(info.pageCount, 1);
    });

    test('fieldKeystroke + fieldFormat on a text field', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.textField('phone', 50, 700, 200, 30);
      await page.fieldKeystroke('AFNumber_Keystroke(0, 0, 0, 0, "", true);');
      await page.fieldFormat('AFNumber_Format(0, 0, 0, 0, "", true);');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('fieldValidate + fieldCalculate on a text field', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.textField('total', 50, 700, 200, 30);
      await page.fieldValidate('event.rc = (event.value >= 0);');
      await page.fieldCalculate('event.value = 42;');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('linkUrl attaches a URL link', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.font('Helvetica', 12);
      await page.paragraph('Click here');
      await page.linkUrl('https://example.com');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('linkPage attaches a page link', () async {
      final builder = await Pdf.build();
      final page1 = await builder.addA4Page();
      await page1.font('Helvetica', 12);
      await page1.paragraph('Go to page 2');
      await page1.linkPage(1);
      await page1.done();
      final page2 = await builder.addA4Page();
      await page2.paragraph('Page 2');
      await page2.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
      expect(info.pageCount, 2);
    });

    test('footnote adds a footnote', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.font('Helvetica', 12);
      await page.paragraph('See note below');
      await page.footnote('1', 'This is the footnote text.');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('columns lays out text in multiple columns', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.font('Helvetica', 10);
      await page.columns(2, 20.0,
          'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '
          'Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('newline inserts a line break', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.font('Helvetica', 12);
      await page.paragraph('Line one');
      await page.newline();
      await page.paragraph('Line two');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      expect(bytes.length, greaterThan(0));
    });

    test('newPageSameSize adds a continuation page', () async {
      final builder = await Pdf.build();
      final page = await builder.addPage(width: 400, height: 300);
      await page.font('Helvetica', 12);
      await page.paragraph('Page 1');
      await page.newPageSameSize();
      await page.paragraph('Page 2');
      await page.done();
      final sink = TestPdfSink();
      await builder.save(sink);
      builder.dispose();

      final bytes = sink.takeBytes();
      final info = await pdf.probe(sourceOf(bytes));
      expect(info.isValid, isTrue);
      expect(info.pageCount, 2);
    });
  });
}
