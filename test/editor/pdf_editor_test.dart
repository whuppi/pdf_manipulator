import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

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
      final editor = await Pdf.edit(minimalPdf);
      expect(await editor.pageCount, equals(1));
      editor.dispose();
    });

    test('reads version in major.minor format', () async {
      final editor = await Pdf.edit(minimalPdf);
      expect(await editor.version, contains('.'));
      final parts = (await editor.version).split('.');
      expect(parts.length, equals(2));
      editor.dispose();
    });

    test('isModified is false on fresh open', () async {
      final editor = await Pdf.edit(minimalPdf);
      expect(await editor.isModified, isFalse);
      editor.dispose();
    });

    test('throws on garbage bytes', () async {
      expect(() => Pdf.edit(garbageBytes), throwsA(isA<PdfError>()));
    });

    test('throws on empty bytes', () async {
      expect(() => Pdf.edit(emptyBytes), throwsA(isA<PdfError>()));
    });
  });

  group('PdfEditor metadata', () {
    test('set and get title round-trips', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setTitle('Test Title');
      expect(await editor.getTitle(), equals('Test Title'));
      editor.dispose();
    });

    test('set and get author round-trips', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setAuthor('DC');
      expect(await editor.getAuthor(), equals('DC'));
      editor.dispose();
    });

    test('set and get subject round-trips', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setSubject('Testing');
      expect(await editor.getSubject(), equals('Testing'));
      editor.dispose();
    });

    test('set and get keywords round-trips', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setKeywords('dart, pdf, test');
      expect(await editor.getKeywords(), equals('dart, pdf, test'));
      editor.dispose();
    });

    test('metadata changes mark document as modified', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setTitle('Changed');
      expect(await editor.isModified, isTrue);
      editor.dispose();
    });

    test('unicode title round-trips', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setTitle('日本語タイトル');
      expect(await editor.getTitle(), equals('日本語タイトル'));
      editor.dispose();
    });

    test('metadata persists through save → reopen', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setTitle('Persisted');
      await editor.setAuthor('Miko');
      final saved = await editor.save();
      editor.dispose();

      final doc = await pdf.open(saved);
      expect(doc.title, equals('Persisted'));
      expect(doc.author, equals('Miko'));
    });
  });

  group('PdfEditor page manipulation', () {
    test('rotate page changes rotation value', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.rotatePage(0, degrees: 90);
      final saved = await editor.save();
      editor.dispose();

      final doc = await pdf.open(saved);
      expect(doc.pages[0].rotation, equals(90));
    });

    test('rotate marks as modified', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.rotatePage(0, degrees: 90);
      expect(await editor.isModified, isTrue);
      editor.dispose();
    });

    test('get page media box returns correct A4 dimensions', () async {
      final editor = await Pdf.edit(minimalPdf);
      final box = await editor.getPageMediaBox(0);
      expect(box.width, closeTo(595, 1));
      expect(box.height, closeTo(842, 1));
      editor.dispose();
    });
  });

  group('PdfEditor merge', () {
    test('merge adds pages — count increases', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.mergeFrom(minimalPdf);
      expect(await editor.pageCount, equals(2));
      editor.dispose();
    });

    test('merge 3 PDFs — count is 3', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.mergeFrom(minimalPdf);
      await editor.mergeFrom(minimalPdf);
      expect(await editor.pageCount, equals(3));
      editor.dispose();
    });

    test('merge preserves mixed page dimensions', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.mergeFrom(letterPdf);
      final saved = await editor.save();
      editor.dispose();

      final doc = await pdf.open(saved);
      expect(doc.pages[0].width, closeTo(595, 1)); // A4
      expect(doc.pages[1].width, closeTo(612, 1)); // Letter
    });
  });

  group('PdfEditor delete/move/extract', () {
    test('delete page reduces count', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(threePage);
      expect(await editor.pageCount, equals(3));
      await editor.deletePage(1);
      expect(await editor.pageCount, equals(2));
      editor.dispose();
    });

    test('delete first page — Letter becomes first', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(threePage);
      await editor.deletePage(0);
      final saved = await editor.save();
      editor.dispose();

      final doc = await pdf.open(saved);
      expect(doc.pages[0].width, closeTo(612, 1)); // Letter
    });

    test('move page preserves count', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(threePage);
      await editor.movePage(from: 0, to: 2);
      expect(await editor.pageCount, equals(3));
      editor.dispose();
    });

    test('extract pages returns correct count', () async {
      final threePage = await buildThreePagePdf();
      final editor = await Pdf.edit(threePage);
      final extracted = await editor.extractPages([0, 2]);
      editor.dispose();

      final doc = await pdf.open(extracted);
      expect(doc.pageCount, equals(2));
    });
  });

  group('PdfEditor save', () {
    test('save preserves page count', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.setTitle('Saved');
      final saved = await editor.save();
      editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(1));
    });

    test('save with options preserves page count', () async {
      final editor = await Pdf.edit(minimalPdf);
      final saved = await editor.saveWithOptions(compress: true, garbageCollect: true);
      editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(1));
    });

    test('save after merge preserves combined pages', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.mergeFrom(minimalPdf);
      final saved = await editor.save();
      editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(2));
    });
  });

  group('PdfEditor lifecycle', () {
    test('throws StateError after dispose on pageCount', () async {
      final editor = await Pdf.edit(minimalPdf);
      editor.dispose();
      expect(() => editor.pageCount, throwsStateError);
    });

    test('throws StateError after dispose on title', () async {
      final editor = await Pdf.edit(minimalPdf);
      editor.dispose();
      expect(() => editor.getTitle(), throwsStateError);
    });

    test('throws StateError after dispose on save', () async {
      final editor = await Pdf.edit(minimalPdf);
      editor.dispose();
      expect(() => editor.save(), throwsStateError);
    });

    test('double dispose is safe', () async {
      final editor = await Pdf.edit(minimalPdf);
      editor.dispose();
      editor.dispose();
    });
  });

  group('PdfEditor forms', () {
    test('flattenForms on no-form PDF preserves page count', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.flattenForms();
      final saved = await editor.save();
      editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(1));
    });
  });

  group('PdfEditor redaction', () {
    test('applyAllRedactions on no-redaction PDF preserves page count', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.applyAllRedactions();
      final saved = await editor.save();
      editor.dispose();
      final doc = await pdf.open(saved);
      expect(doc.pageCount, equals(1));
    });
  });

  group('PdfEditor watermark', () {
    test('watermark increases output size', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.addWatermark(0, 'DRAFT');
      final saved = await editor.save();
      expect(saved.length, greaterThan(minimalPdf.length));
      editor.dispose();
    });

    test('watermark marks as modified', () async {
      final editor = await Pdf.edit(minimalPdf);
      expect(await editor.isModified, isFalse);
      await editor.addWatermark(0, 'MOD');
      expect(await editor.isModified, isTrue);
      editor.dispose();
    });
  });

  group('PdfEditor encrypted save', () {
    test('encrypted save produces larger output than normal save', () async {
      final editor = await Pdf.edit(minimalPdf);
      final normal = await editor.save();
      // Re-open to get a fresh editor for encrypted save
      final editor2 = await Pdf.edit(minimalPdf);
      final encrypted = await editor2.saveEncrypted(ownerPassword: 'pw');
      expect(encrypted.length, greaterThan(normal.length));
      editor.dispose();
      editor2.dispose();
    });

    test('encrypted save with modifications preserves page count', () async {
      final editor = await Pdf.edit(minimalPdf);
      await editor.mergeFrom(minimalPdf);
      final encrypted = await editor.saveEncrypted(ownerPassword: 'pw', userPassword: 'user');
      editor.dispose();

      final doc = await pdf.open(encrypted, password: 'pw');
      expect(doc.pageCount, equals(2));
    });
  });
}
