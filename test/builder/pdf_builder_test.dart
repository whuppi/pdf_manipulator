import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('PdfBuilder', () {
    test('creates a blank A4 PDF', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      expect(bytes.length, greaterThan(0));
      final doc = await pdf.open(bytes);
      expect(doc.pageCount, 1);
    });

    test('creates a blank Letter PDF', () async {
      final builder = await Pdf.build();
      final page = await builder.addLetterPage();
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      final doc = await pdf.open(bytes);
      expect(doc.pageCount, 1);
      // Letter is 612x792
      expect(doc.pages[0].width, closeTo(612, 1));
    });

    test('creates a custom-sized page', () async {
      final builder = await Pdf.build();
      final page = await builder.addPage(width: 400, height: 600);
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      final doc = await pdf.open(bytes);
      expect(doc.pages[0].width, closeTo(400, 1));
      expect(doc.pages[0].height, closeTo(600, 1));
    });

    test('creates multi-page PDF', () async {
      final builder = await Pdf.build();
      for (var i = 0; i < 5; i++) {
        final page = await builder.addA4Page();
        await page.done();
      }
      final bytes = await builder.save();
      builder.dispose();

      final doc = await pdf.open(bytes);
      expect(doc.pageCount, 5);
    });

    test('sets metadata', () async {
      final builder = await Pdf.build();
      await builder.setTitle('Test Title');
      await builder.setAuthor('Test Author');
      await builder.setSubject('Test Subject');
      await builder.setKeywords('test, builder');
      final page = await builder.addA4Page();
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      final doc = await pdf.open(bytes);
      expect(doc.title, 'Test Title');
      expect(doc.author, 'Test Author');
    });

    test('adds text to page', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.font('Helvetica', 14);
      await page.at(72, 750);
      await page.text('Hello, world!');
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      final text = await pdf.extractText(bytes);
      expect(text, contains('Hello, world!'));
    });

    test('adds paragraph to page', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.paragraph('This is a paragraph of text that should wrap.');
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      final text = await pdf.extractText(bytes);
      expect(text, contains('paragraph'));
    });

    test('adds heading to page', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.heading(1, 'Chapter One');
      await page.paragraph('Content here.');
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      final text = await pdf.extractText(bytes);
      expect(text, contains('Chapter One'));
    });

    test('adds watermark to builder page', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.watermark('DRAFT');
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      expect(bytes.length, greaterThan(0));
      final info = await pdf.probe(bytes);
      expect(info.isValid, isTrue);
    });

    test('builds encrypted PDF larger than unencrypted', () async {
      final builder1 = await Pdf.build();
      final page1 = await builder1.addA4Page();
      await page1.paragraph('Secret content');
      await page1.done();
      final plain = await builder1.save();
      builder1.dispose();

      final builder2 = await Pdf.build();
      final page2 = await builder2.addA4Page();
      await page2.paragraph('Secret content');
      await page2.done();
      final encrypted = await builder2.saveEncrypted(ownerPassword: 'test123');
      builder2.dispose();

      expect(encrypted.length, greaterThan(plain.length));
    });

    test('output is valid PDF', () async {
      final builder = await Pdf.build();
      await builder.setTitle('Validation Test');
      final page = await builder.addA4Page();
      await page.heading(1, 'Title');
      await page.paragraph('Body text');
      await page.space(10);
      await page.horizontalRule();
      await page.paragraph('After the rule');
      await page.done();
      final bytes = await builder.save();
      builder.dispose();

      final info = await pdf.probe(bytes);
      expect(info.isValid, isTrue);
      expect(info.pageCount, 1);
    });

    test('double dispose is safe', () async {
      final builder = await Pdf.build();
      final page = await builder.addA4Page();
      await page.done();
      await builder.save();
      builder.dispose();
      builder.dispose();
    });

    test('throws after dispose', () async {
      final builder = await Pdf.build();
      builder.dispose();
      expect(() => builder.addA4Page(), throwsA(anything));
    });
  });
}
