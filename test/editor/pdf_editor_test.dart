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

  group('PdfEditor.open', () {
    test('opens valid PDF with correct page count', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      expect(await editor.pageCount, equals(1));
      editor.dispose();
    });

    test('reads version in major.minor format', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      expect(await editor.version, contains('.'));
      final parts = (await editor.version).split('.');
      expect(parts.length, equals(2));
      editor.dispose();
    });

    test('isModified is false on fresh open', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      expect(await editor.isModified, isFalse);
      editor.dispose();
    });

    test('throws on garbage bytes', () async {
      expect(() => Pdf.edit(sourceOf(garbageBytes)), throwsA(isA<PdfError>()));
    });

    test('throws on empty bytes', () async {
      expect(() => Pdf.edit(sourceOf(emptyBytes)), throwsA(isA<PdfError>()));
    });
  });

  group('PdfEditor metadata', () {
    test('set and get title round-trips', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setTitle('Test Title');
      expect(await editor.getTitle(), equals('Test Title'));
      editor.dispose();
    });

    test('set and get author round-trips', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setAuthor('DC');
      expect(await editor.getAuthor(), equals('DC'));
      editor.dispose();
    });

    test('set and get subject round-trips', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setSubject('Testing');
      expect(await editor.getSubject(), equals('Testing'));
      editor.dispose();
    });

    test('set and get keywords round-trips', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setKeywords('dart, pdf, test');
      expect(await editor.getKeywords(), equals('dart, pdf, test'));
      editor.dispose();
    });

    test('metadata changes mark document as modified', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setTitle('Changed');
      expect(await editor.isModified, isTrue);
      editor.dispose();
    });

    test('unicode title round-trips', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setTitle('日本語タイトル');
      expect(await editor.getTitle(), equals('日本語タイトル'));
      editor.dispose();
    });

    test('metadata persists through save → reopen', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setTitle('Persisted');
      await editor.setAuthor('Miko');
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.title, equals('Persisted'));
      expect(doc.author, equals('Miko'));
    });
  });

  group('PdfEditor page manipulation', () {
    test('rotate page changes rotation value', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.rotatePage(0, degrees: 90);
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pages[0].rotation, equals(90));
    });

    test('rotate marks as modified', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.rotatePage(0, degrees: 90);
      expect(await editor.isModified, isTrue);
      editor.dispose();
    });

    test('get page media box returns correct A4 dimensions', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final box = await editor.getPageMediaBox(0);
      expect(box.width, closeTo(595, 1));
      expect(box.height, closeTo(842, 1));
      editor.dispose();
    });
  });

  group('PdfEditor merge', () {
    test('merge adds pages — count increases', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(minimalPdf));
      expect(await editor.pageCount, equals(2));
      editor.dispose();
    });

    test('merge 3 PDFs — count is 3', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(minimalPdf));
      expect(await editor.pageCount, equals(3));
      editor.dispose();
    });

    test('merge preserves mixed page dimensions', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(letterPdf));
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pages[0].width, closeTo(595, 1)); // A4
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter
    });
  });

  group('PdfEditor delete/move/extract', () {
    test('delete page reduces count', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(sourceOf(threePage));
      expect(await editor.pageCount, equals(3));
      await editor.deletePage(1);
      expect(await editor.pageCount, equals(2));
      editor.dispose();
    });

    test('delete first page — Letter becomes first', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(sourceOf(threePage));
      await editor.deletePage(0);
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();

      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pages[0].width, closeTo(612, 1)); // Letter
    });

    test('move page preserves count', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(sourceOf(threePage));
      await editor.movePage(from: 0, to: 2);
      expect(await editor.pageCount, equals(3));
      editor.dispose();
    });

    test('extract pages returns correct count', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(sourceOf(threePage));
      final sink = TestPdfSink();
      await editor.extractPages([0, 2], sink);
      editor.dispose();

      final extracted = sink.takeBytes();
      final doc = await pdf.open(sourceOf(extracted));
      expect(doc.pageCount, equals(2));
    });
  });

  group('PdfEditor save', () {
    test('save preserves page count', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.setTitle('Saved');
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(1));
    });

    test('save with options preserves page count', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final sink = TestPdfSink();
      await editor.saveWithOptions(sink, compress: true, garbageCollect: true);
      editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(1));
    });

    test('save after merge preserves combined pages', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(minimalPdf));
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(2));
    });
  });

  group('PdfEditor lifecycle', () {
    test('throws StateError after dispose on pageCount', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      editor.dispose();
      expect(() => editor.pageCount, throwsStateError);
    });

    test('throws StateError after dispose on title', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      editor.dispose();
      expect(() => editor.getTitle(), throwsStateError);
    });

    test('throws StateError after dispose on save', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      editor.dispose();
      expect(() => editor.save(TestPdfSink()), throwsStateError);
    });

    test('double dispose is safe', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      editor.dispose();
      editor.dispose();
    });
  });

  group('PdfEditor forms', () {
    test('flattenForms on no-form PDF preserves page count', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.flattenForms();
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(1));
    });
  });

  group('PdfEditor redaction', () {
    test('applyAllRedactions on no-redaction PDF preserves page count', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.applyAllRedactions();
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(1));
    });
  });

  group('PdfEditor watermark', () {
    test('watermark increases output size', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.addWatermark(0, 'DRAFT');
      final sink = TestPdfSink();
      await editor.save(sink);
      final saved = sink.takeBytes();
      expect(saved.length, greaterThan(minimalPdf.length));
      editor.dispose();
    });

    test('watermark marks as modified', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      expect(await editor.isModified, isFalse);
      await editor.addWatermark(0, 'MOD');
      expect(await editor.isModified, isTrue);
      editor.dispose();
    });
  });

  group('PdfEditor encrypted save', () {
    test('encrypted save produces larger output than normal save', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      final normSink = TestPdfSink();
      await editor.save(normSink);
      final normal = normSink.takeBytes();
      // Re-open to get a fresh editor for encrypted save
      final editor2 = await Pdf.edit(sourceOf(minimalPdf));
      final encSink = TestPdfSink();
      await editor2.saveEncrypted(encSink, ownerPassword: 'pw');
      final encrypted = encSink.takeBytes();
      expect(encrypted.length, greaterThan(normal.length));
      editor.dispose();
      editor2.dispose();
    });

    test('encrypted save with modifications preserves page count', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      await editor.mergeFrom(sourceOf(minimalPdf));
      final sink = TestPdfSink();
      await editor.saveEncrypted(sink, ownerPassword: 'pw', userPassword: 'user');
      editor.dispose();

      final encrypted = sink.takeBytes();
      final doc = await pdf.open(sourceOf(encrypted), password: 'pw');
      expect(doc.pageCount, equals(2));
    });
  });
}
