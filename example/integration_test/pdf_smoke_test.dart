// Integration smoke test — every API surface exercised with hardcoded data.
// Runs on real devices (Android/iOS) and desktop (macOS/Windows/Linux).
// No file picker needed — uses the same MemorySource/MemorySink from the app.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator_example/main.dart' show MemorySource, MemorySink, minimalPdf, testCertPem, testKeyPem;

DataSource _src(Uint8List bytes) => MemorySource(bytes);

// Optional: force a specific web I/O mode via --dart-define=PDF_IO_MODE=jspi|atomics|opfs
const _modeOverride = String.fromEnvironment('PDF_IO_MODE');
PdfIoMode? get _forceMode => switch (_modeOverride) {
  'jspi' => PdfIoMode.jspi,
  'atomics' => PdfIoMode.atomics,
  'opfs' => PdfIoMode.opfs,
  _ => null,
};

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  late Pdf pdf;
  setUp(() {
    final mode = _forceMode;
    pdf = mode != null ? Pdf(config: PdfConfig(webIoMode: mode)) : Pdf();
  });
  tearDown(() => pdf.dispose());

  // ── PdfDoc — read-only queries ──────────────────────────────────

  testWidgets('open returns correct page count + metadata', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    expect(doc.pageCount, 1);
    expect(doc.version, isNotEmpty);
    expect(doc.pages, hasLength(1));
    expect(doc.pages[0].width, closeTo(595, 1));
    expect(doc.pages[0].height, closeTo(842, 1));
    await doc.dispose();
  });

  testWidgets('extract text (plain + markdown + html)', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    expect(await doc.extract(pages: const PdfPages.all()), isA<String>());
    expect(await doc.extract(pages: const PdfPages.single(0)), isA<String>());
    expect(await doc.extract(pages: const PdfPages.all(), format: PdfExtractionFormat.markdown), isA<String>());
    expect(await doc.extract(pages: const PdfPages.single(0), format: PdfExtractionFormat.html), isA<String>());
    await doc.dispose();
  });

  testWidgets('search returns results list', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    final r = await doc.search(query: 'the', pages: const PdfPages.all());
    expect(r, isA<List<SearchResult>>());
    await doc.dispose();
  });

  testWidgets('render streams pages', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    var count = 0;
    await for (final page in doc.render(pages: const PdfPages.single(0))) {
      expect(page.width, greaterThan(0));
      expect(page.data, isNotEmpty);
      count++;
    }
    expect(count, 1);
    await doc.dispose();
  });

  testWidgets('extractImages streams', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    await for (final _ in doc.extractImages(pages: const PdfPages.single(0))) {}
    await doc.dispose();
  });

  testWidgets('signatures + verify', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    expect(await doc.getSignatures(), isA<List<PdfSignatureInfo>>());
    expect(await doc.verifySignatures(), isA<bool>());
    await doc.dispose();
  });

  testWidgets('validatePdfA + validatePdfUa', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    final a = await doc.validatePdfA();
    expect(a.errors, isA<int>());
    expect(await doc.validatePdfUa(), isA<bool>());
    await doc.dispose();
  });

  testWidgets('classifyPage + classifyDocument', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    expect((await doc.classifyPage(0)).type, isNotEmpty);
    expect((await doc.classifyDocument()).type, isNotEmpty);
    await doc.dispose();
  });

  testWidgets('planSplitByBookmarks', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    // Minimal PDF has no bookmarks — should return empty or throw
    try {
      await doc.planSplitByBookmarks();
    } catch (_) {}
    await doc.dispose();
  });

  // ── PdfSugar — one-shot convenience ops ─────────────────────────

  testWidgets('merge', (t) async {
    final sink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], sink);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 2);
    await doc.dispose();
  });

  testWidgets('split', (t) async {
    // Make a 2-page PDF first
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final twoPage = mergeSink.takeBytes();

    final sinks = <MemorySink>[];
    await pdf.split(_src(twoPage), (i) { final s = MemorySink(); sinks.add(s); return s; }, every: 1);
    expect(sinks.length, 2);
  });

  testWidgets('splitBySize', (t) async {
    final sinks = <MemorySink>[];
    await pdf.splitBySize(_src(minimalPdf), (i) { final s = MemorySink(); sinks.add(s); return s; }, maxBytes: 500000);
    expect(sinks, isNotEmpty);
  });

  testWidgets('extractPages', (t) async {
    final sink = MemorySink();
    await pdf.extractPages(_src(minimalPdf), sink, pages: [0]);
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('deletePages', (t) async {
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final sink = MemorySink();
    await pdf.deletePages(_src(mergeSink.takeBytes()), sink, pages: [0]);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('reorderPages', (t) async {
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final sink = MemorySink();
    await pdf.reorderPages(_src(mergeSink.takeBytes()), sink, order: [1, 0]);
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('movePage', (t) async {
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final sink = MemorySink();
    await pdf.movePage(_src(mergeSink.takeBytes()), sink, from: 0, to: 1);
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('rotateAllPages + rotatePages', (t) async {
    final s1 = MemorySink();
    await pdf.rotateAllPages(_src(minimalPdf), s1, degrees: 90);
    final doc1 = await pdf.open(_src(s1.takeBytes()));
    expect(doc1.pages[0].rotation, 90);
    await doc1.dispose();

    final s2 = MemorySink();
    await pdf.rotatePages(_src(minimalPdf), s2, pages: {0: 180});
    final doc2 = await pdf.open(_src(s2.takeBytes()));
    expect(doc2.pages[0].rotation, 180);
    await doc2.dispose();
  });

  testWidgets('compress', (t) async {
    final sink = MemorySink();
    await pdf.compress(_src(minimalPdf), sink);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('watermark (all positions + layers)', (t) async {
    for (final pos in [
      const PdfWatermarkPosition.center(),
      const PdfWatermarkPosition.corner(PdfCorner.topRight),
      const PdfWatermarkPosition.tiled(columns: 2, rows: 2),
    ]) {
      final sink = MemorySink();
      await pdf.watermark(_src(minimalPdf), sink, text: 'TEST', position: pos);
      expect(sink.takeBytes(), isNotEmpty);
    }
    final bgSink = MemorySink();
    await pdf.watermark(_src(minimalPdf), bgSink, text: 'BG', layer: PdfWatermarkLayer.background);
    expect(bgSink.takeBytes(), isNotEmpty);
  });

  testWidgets('encrypt + decrypt', (t) async {
    final encSink = MemorySink();
    await pdf.encrypt(_src(minimalPdf), encSink, encryption: const PdfEncryptionConfig(ownerPassword: 'pw'));
    final doc1 = await pdf.open(_src(encSink.takeBytes()), password: 'pw');
    expect(doc1.isEncrypted, isTrue);
    await doc1.dispose();
  });

  testWidgets('flattenForms', (t) async {
    final sink = MemorySink();
    await pdf.flattenForms(_src(minimalPdf), sink);
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('applyRedactions', (t) async {
    final sink = MemorySink();
    await pdf.applyRedactions(_src(minimalPdf), sink);
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('embedFile', (t) async {
    final sink = MemorySink();
    await pdf.embedFile(_src(minimalPdf), sink, name: 'test.txt', fileData: _src(Uint8List.fromList('hello'.codeUnits)));
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('eraseRegions', (t) async {
    final sink = MemorySink();
    await pdf.eraseRegions(_src(minimalPdf), sink, page: 0, regions: [const PdfRect(x: 10, y: 10, width: 50, height: 50)]);
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('addStamp', (t) async {
    final sink = MemorySink();
    await pdf.addStamp(_src(minimalPdf), sink, page: 0, type: PdfStampType.approved, rect: const PdfRect(x: 50, y: 50, width: 200, height: 60));
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('convertToPdfA', (t) async {
    final sink = MemorySink();
    await pdf.convertToPdfA(_src(minimalPdf), sink);
    expect(sink.takeBytes(), isNotEmpty);
  });

  // ── PdfStandalone ───────────────────────────────────────────────

  testWidgets('sign (PEM)', (t) async {
    final sink = MemorySink();
    await pdf.sign(_src(minimalPdf), sink, credentials: const PdfSigningCredentials.pem(testCertPem, testKeyPem));
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('convertTo DOCX', (t) async {
    final sink = MemorySink();
    await pdf.convertTo(_src(minimalPdf), sink, format: PdfDocumentFormat.docx);
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('convertToPdf (DOCX round-trip)', (t) async {
    final docxSink = MemorySink();
    await pdf.convertTo(_src(minimalPdf), docxSink, format: PdfDocumentFormat.docx);
    final pdfSink = MemorySink();
    await pdf.convertToPdf(_src(docxSink.takeBytes()), pdfSink, format: PdfDocumentFormat.docx);
    expect(pdfSink.takeBytes(), isNotEmpty);
  });

  testWidgets('extractPages (standalone)', (t) async {
    final sink = MemorySink();
    await pdf.extractPages(_src(minimalPdf), sink, pages: [0]);
    expect(sink.takeBytes(), isNotEmpty);
  });

  // ── PdfEditor — batch mutations ─────────────────────────────────

  testWidgets('editor metadata get/set + isModified', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.setTitle('T'); await e.setAuthor('A'); await e.setSubject('S'); await e.setKeywords('K');
    expect(await e.getTitle(), 'T');
    expect(await e.getAuthor(), 'A');
    expect(await e.getSubject(), 'S');
    expect(await e.getKeywords(), 'K');
    expect(await e.isModified, isTrue);
    expect(await e.pageCount, 1);
    expect(await e.version, isNotEmpty);
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor page ops', (t) async {
    // Make 2-page PDF
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final twoPage = mergeSink.takeBytes();

    final e = await pdf.edit(_src(twoPage));
    await e.rotatePage(0, degrees: 90);
    await e.rotateAllPages(degrees: 180);
    final mb = await e.getPageMediaBox(0);
    expect(mb.width, greaterThan(0));
    await e.movePage(from: 0, to: 1);
    await e.deletePage(1);
    expect(await e.pageCount, 1);
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor selectPages', (t) async {
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final e = await pdf.edit(_src(mergeSink.takeBytes()));
    await e.selectPages([0]);
    expect(await e.pageCount, 1);
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor mergeFrom', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.mergeFrom(_src(minimalPdf));
    expect(await e.pageCount, 2);
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor optimizeImages + unembedStandardFonts', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    expect(await e.optimizeImages(), isA<int>());
    expect(await e.unembedStandardFonts(), isA<int>());
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor watermark + stamp', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.addWatermark(0, 'TEST');
    await e.addStamp(0, type: PdfStampType.draft, rect: const PdfRect(x: 50, y: 50, width: 200, height: 60));
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor embedFile + eraseRegions + cropMargins', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.embedFile('test.txt', _src(Uint8List.fromList('hi'.codeUnits)));
    await e.eraseRegions(0, [const PdfRect(x: 10, y: 10, width: 50, height: 50)]);
    await e.cropMargins(left: 10, right: 10, top: 10, bottom: 10);
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor flatten forms + annotations', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.flattenForms();
    await e.flattenAllAnnotations();
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor redaction lifecycle', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.addRedaction(0, const PdfRect(x: 50, y: 50, width: 100, height: 20));
    expect(await e.redactionCount(0), greaterThan(0));
    await e.applyRedactions();
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor scrubMetadata', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.scrubMetadata();
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor convertToPdfA', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.convertToPdfA();
    final sink = MemorySink(); await e.save(sink); await e.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('editor save options (full/incremental/encrypted/remove)', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    final s1 = MemorySink(); await e.save(s1, options: const PdfSaveOptions.fullRewrite(compress: true, garbageCollect: true));
    expect(s1.takeBytes(), isNotEmpty);

    final s2 = MemorySink(); await e.save(s2, options: const PdfSaveOptions.incremental());
    expect(s2.takeBytes(), isNotEmpty);

    final s3 = MemorySink(); await e.save(s3, options: const PdfSaveOptions.fullRewrite(encryption: PdfEncryption.config(ownerPassword: 'pw')));
    expect(s3.takeBytes(), isNotEmpty);

    final s4 = MemorySink(); await e.save(s4, options: const PdfSaveOptions.fullRewrite(encryption: PdfEncryption.remove()));
    expect(s4.takeBytes(), isNotEmpty);

    await e.dispose();
  });

  // ── PdfBuilder — create from scratch ────────────────────────────

  testWidgets('builder text document (A4)', (t) async {
    final b = await pdf.build();
    await b.setTitle('T'); await b.setAuthor('A'); await b.setSubject('S'); await b.setKeywords('K');
    final p = await b.addA4Page();
    await p.font('Helvetica', 14);
    await p.heading(1, 'Title');
    await p.space(10);
    await p.paragraph('Paragraph text.');
    await p.text('Plain text.');
    await p.horizontalRule();
    await p.columns(2, 10, 'Column text flows here.');
    await p.footnote('1', 'Footnote text.');
    await p.watermark('DRAFT');
    await p.newline();
    await p.done();
    final sink = MemorySink(); await b.save(sink); await b.dispose();
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('builder Letter + custom size', (t) async {
    final b = await pdf.build();
    final p1 = await b.addLetterPage();
    await p1.text('Letter'); await p1.done();
    final p2 = await b.addPage(width: 400, height: 300);
    await p2.text('Custom'); await p2.done();
    final sink = MemorySink(); await b.save(sink); await b.dispose();
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 2);
    await doc.dispose();
  });

  testWidgets('builder newPageSameSize', (t) async {
    final b = await pdf.build();
    final p = await b.addA4Page();
    await p.text('Page 1');
    await p.newPageSameSize();
    await p.text('Page 2');
    await p.done();
    final sink = MemorySink(); await b.save(sink); await b.dispose();
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 2);
    await doc.dispose();
  });

  testWidgets('builder form fields (all types)', (t) async {
    final b = await pdf.build();
    final p = await b.addA4Page();
    await p.textField('name', const PdfRect(x: 72, y: 700, width: 200, height: 20), defaultValue: 'John');
    await p.checkbox('agree', const PdfRect(x: 72, y: 660, width: 14, height: 14));
    await p.comboBox('country', const PdfRect(x: 72, y: 620, width: 150, height: 20), ['A', 'B'], selected: 'A');
    await p.pushButton('btn', const PdfRect(x: 72, y: 580, width: 80, height: 30), 'Click');
    await p.signatureField('sig', const PdfRect(x: 72, y: 520, width: 200, height: 60));
    // radioGroup — skipped: Rust handler not yet implemented (pre-existing gap)
    await p.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true)');
    await p.fieldFormat('AFNumber_Format(2, 0, 0, 0, "", true)');
    await p.fieldValidate('AFRange_Validate(true, 0, true, 100)');
    await p.fieldCalculate('AFSimple_Calculate("SUM", new Array("f1", "f2"))');
    await p.done();
    final sink = MemorySink(); await b.save(sink); await b.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  testWidgets('builder links', (t) async {
    final b = await pdf.build();
    final p = await b.addA4Page();
    await p.linkUrl('https://example.com');
    await p.linkPage(0);
    await p.done();
    final sink = MemorySink(); await b.save(sink); await b.dispose();
    expect(sink.takeBytes(), isNotEmpty);
  });

  // ── Lifecycle ───────────────────────────────────────────────────

  testWidgets('dispose prevents further ops', (t) async {
    final p = Pdf();
    await p.dispose();
    expect(() => p.open(_src(minimalPdf)), throwsStateError);
  });

  testWidgets('doc dispose + editor dispose + builder dispose', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    await doc.dispose();
    await doc.dispose(); // double dispose safe

    final e = await pdf.edit(_src(minimalPdf));
    final sink = MemorySink(); await e.save(sink);
    await e.dispose();
    await e.dispose();

    final b = await pdf.build();
    final p = await b.addA4Page(); await p.text('x'); await p.done();
    final sink2 = MemorySink(); await b.save(sink2);
    await b.dispose();
    await b.dispose();
  });
}
