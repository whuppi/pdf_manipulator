// Integration smoke test — every API surface exercised with hardcoded data.
// Runs on real devices (Android/iOS) and desktop (macOS/Windows/Linux).
// No file picker needed — uses the same MemorySource/MemorySink from the app.

import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator_example/main.dart' as app;
import 'package:pdf_manipulator_example/main.dart'
    show MemorySource, MemorySink, minimalPdf, testCertPem, testKeyPem;

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
    // The fixture is one BLANK page — the only correct extraction is
    // emptiness. isA<String>() would have accepted mojibake.
    expect((await doc.extract(pages: const PdfPages.all())).trim(), isEmpty);
    expect(
        (await doc.extract(pages: const PdfPages.single(0))).trim(), isEmpty);
    expect(
        (await doc.extract(
                pages: const PdfPages.all(),
                format: PdfExtractionFormat.markdown))
            .trim(),
        isEmpty);
    final html = await doc.extract(
        pages: const PdfPages.single(0), format: PdfExtractionFormat.html);
    expect(html.trim(), isEmpty,
        reason: 'a blank page yields empty output in every format');
    await doc.dispose();
  });

  testWidgets('search returns results list', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    final r = await doc.search(query: 'the', pages: const PdfPages.all());
    expect(r, isEmpty,
        reason: 'a blank page cannot contain matches — any hit here '
            'is an engine hallucination');
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
    var images = 0;
    await for (final _ in doc.extractImages(pages: const PdfPages.single(0))) {
      images++;
    }
    expect(images, 0, reason: 'the blank fixture embeds no images');
    await doc.dispose();
  });

  testWidgets('signatures + verify', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    expect(await doc.getSignatures(), isEmpty,
        reason: 'unsigned fixture — a phantom signature is worse '
            'than none');
    expect(await doc.verifySignatures(), isFalse);
    await doc.dispose();
  });

  testWidgets('validatePdfA + validatePdfUa', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    final a = await doc.validatePdfA();
    expect(a.compliant, isFalse,
        reason: 'the minimal fixture is NOT PDF/A — a pass means the '
            'validator is not validating');
    expect(a.errors, greaterThan(0));
    expect(await doc.validatePdfUa(), isFalse);
    await doc.dispose();
  });

  testWidgets('classifyPage + classifyDocument', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    final page = await doc.classifyPage(0);
    expect(page.type, isNotEmpty);
    expect(page.confidence, inInclusiveRange(0.0, 1.0));
    final whole = await doc.classifyDocument();
    expect(whole.type, isNotEmpty);
    expect(whole.confidence, inInclusiveRange(0.0, 1.0));
    await doc.dispose();
  });

  testWidgets('planSplitByBookmarks', (t) async {
    final doc = await pdf.open(_src(minimalPdf));
    await expectLater(
      doc.planSplitByBookmarks(),
      throwsA(isA<PdfEngineError>()
          .having((e) => e.message, 'message', contains('no bookmarks'))),
      reason: 'the engine refuses with an actionable message — '
          'inventing chapter splits would be worse',
    );
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
    await pdf.split(_src(twoPage), (i) {
      final s = MemorySink();
      sinks.add(s);
      return s;
    }, every: 1);
    expect(sinks.length, 2);
    for (final s in sinks) {
      final doc = await pdf.open(_src(s.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }
  });

  testWidgets('splitBySize', (t) async {
    final sinks = <MemorySink>[];
    await pdf.splitBySize(_src(minimalPdf), (i) {
      final s = MemorySink();
      sinks.add(s);
      return s;
    }, maxBytes: 500000);
    expect(sinks, hasLength(1),
        reason: 'the whole fixture fits the limit — one chunk');
    final chunk = await pdf.open(_src(sinks.single.takeBytes()));
    expect(chunk.pageCount, 1);
    await chunk.dispose();
  });

  testWidgets('extractPages', (t) async {
    final sink = MemorySink();
    await pdf.extractPages(_src(minimalPdf), sink, pages: [0]);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
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
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 2,
        reason: 'reorder must keep every page (both pages are '
            'identical here; order itself is proven in the batteries)');
    await doc.dispose();
  });

  testWidgets('movePage', (t) async {
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final sink = MemorySink();
    await pdf.movePage(_src(mergeSink.takeBytes()), sink, from: 0, to: 1);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 2);
    await doc.dispose();
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
      final out = sink.takeBytes();
      expect(String.fromCharCodes(out), contains('TEST'),
          reason: 'the watermark text must actually land in the output');
      final doc = await pdf.open(_src(out));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }
    final bgSink = MemorySink();
    await pdf.watermark(_src(minimalPdf), bgSink,
        text: 'BG', layer: PdfWatermarkLayer.background);
    expect(String.fromCharCodes(bgSink.takeBytes()), contains('BG'));
  });

  testWidgets('encrypt locks, correct password opens, decrypt unlocks',
      (t) async {
    final encSink = MemorySink();
    await pdf.encrypt(_src(minimalPdf), encSink,
        encryption: const PdfEncryptionConfig(
            ownerPassword: 'owner-pw', userPassword: 'user-pw'));
    final encBytes = encSink.takeBytes();

    // The lock is real: no password / wrong password must refuse.
    await expectLater(pdf.open(_src(encBytes)), throwsA(isA<PdfEngineError>()));
    await expectLater(pdf.open(_src(encBytes), password: 'wrong'),
        throwsA(isA<PdfEngineError>()));

    final doc1 = await pdf.open(_src(encBytes), password: 'user-pw');
    expect(doc1.isEncrypted, isTrue);
    expect(doc1.pageCount, 1);
    await doc1.dispose();

    final decSink = MemorySink();
    await pdf.decrypt(_src(encBytes), decSink, password: 'owner-pw');
    final doc2 = await pdf.open(_src(decSink.takeBytes()));
    expect(doc2.isEncrypted, isFalse);
    await doc2.dispose();
  });

  testWidgets('flattenForms', (t) async {
    final sink = MemorySink();
    await pdf.flattenForms(_src(minimalPdf), sink);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('applyRedactions', (t) async {
    final sink = MemorySink();
    await pdf.applyRedactions(_src(minimalPdf), sink);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('embedFile', (t) async {
    final sink = MemorySink();
    await pdf.embedFile(_src(minimalPdf), sink,
        name: 'test.txt',
        fileData: _src(Uint8List.fromList('hello'.codeUnits)));
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('eraseRegions', (t) async {
    final sink = MemorySink();
    await pdf.eraseRegions(_src(minimalPdf), sink,
        page: 0, regions: [const PdfRect(x: 10, y: 10, width: 50, height: 50)]);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('addStamp', (t) async {
    final sink = MemorySink();
    await pdf.addStamp(_src(minimalPdf), sink,
        page: 0,
        type: PdfStampType.approved,
        rect: const PdfRect(x: 50, y: 50, width: 200, height: 60));
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('convertToPdfA', (t) async {
    final sink = MemorySink();
    await pdf.convertToPdfA(_src(minimalPdf), sink);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  // ── PdfStandalone ───────────────────────────────────────────────

  testWidgets('sign (PEM)', (t) async {
    final sink = MemorySink();
    await pdf.sign(_src(minimalPdf), sink,
        credentials: const PdfSigningCredentials.pem(testCertPem, testKeyPem));
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(await doc.getSignatures(), isNotEmpty,
        reason: 'a signed PDF with no retrievable signature is not '
            'signed');
    await doc.dispose();
  });

  testWidgets('convertTo DOCX', (t) async {
    final sink = MemorySink();
    await pdf.convertTo(_src(minimalPdf), sink, format: PdfDocumentFormat.docx);
    final docx = sink.takeBytes();
    expect(docx.sublist(0, 2), [0x50, 0x4B],
        reason: 'DOCX is a ZIP — wrong magic means the converter '
            'wrote something else');
  });

  testWidgets('convertToPdf (DOCX round-trip)', (t) async {
    final docxSink = MemorySink();
    await pdf.convertTo(_src(minimalPdf), docxSink,
        format: PdfDocumentFormat.docx);
    final pdfSink = MemorySink();
    await pdf.convertToPdf(_src(docxSink.takeBytes()), pdfSink,
        format: PdfDocumentFormat.docx);
    final out = pdfSink.takeBytes();
    expect(String.fromCharCodes(out.sublist(0, 5)), '%PDF-');
    final doc = await pdf.open(_src(out));
    expect(doc.pageCount, greaterThanOrEqualTo(1));
    await doc.dispose();
  });

  testWidgets('extractPages (standalone)', (t) async {
    final sink = MemorySink();
    await pdf.extractPages(_src(minimalPdf), sink, pages: [0]);
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  // ── PdfEditor — batch mutations ─────────────────────────────────

  testWidgets('editor metadata get/set + isModified', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.setTitle('T');
    await e.setAuthor('A');
    await e.setSubject('S');
    await e.setKeywords('K');
    expect(await e.getTitle(), 'T');
    expect(await e.getAuthor(), 'A');
    expect(await e.getSubject(), 'S');
    expect(await e.getKeywords(), 'K');
    expect(await e.isModified, isTrue);
    expect(await e.pageCount, 1);
    expect(await e.version, isNotEmpty);
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
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
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pages[0].rotation, 180,
        reason: 'rotateAllPages(180) must survive save');
    await doc.dispose();
  });

  testWidgets('editor selectPages', (t) async {
    final mergeSink = MemorySink();
    await pdf.merge([_src(minimalPdf), _src(minimalPdf)], mergeSink);
    final e = await pdf.edit(_src(mergeSink.takeBytes()));
    await e.selectPages([0]);
    expect(await e.pageCount, 1);
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
  });

  testWidgets('editor mergeFrom', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.mergeFrom(_src(minimalPdf));
    expect(await e.pageCount, 2);
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 2);
    await saved.dispose();
  });

  testWidgets('editor optimizeImages + unembedStandardFonts', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    expect(await e.optimizeImages(), 0,
        reason: 'no images in the fixture — a nonzero count is '
            'invented work');
    expect(await e.unembedStandardFonts(), 0);
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
  });

  testWidgets('editor watermark survives save', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.addWatermark(0, 'WMTEXT');
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final out = sink.takeBytes();
    expect(String.fromCharCodes(out), contains('WMTEXT'),
        reason: 'the watermark text must land in the saved bytes');
    final doc = await pdf.open(_src(out));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('editor stamp after watermark keeps both', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.addWatermark(0, 'WMTEXT');
    await e.addStamp(0,
        type: PdfStampType.draft,
        rect: const PdfRect(x: 50, y: 50, width: 200, height: 60));
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final out = sink.takeBytes();
    expect(String.fromCharCodes(out), contains('WMTEXT'),
        reason: 'adding a stamp must never erase an earlier edit');
    final doc = await pdf.open(_src(out));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('editor embedFile + eraseRegions + cropMargins', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.embedFile('test.txt', _src(Uint8List.fromList('hi'.codeUnits)));
    await e
        .eraseRegions(0, [const PdfRect(x: 10, y: 10, width: 50, height: 50)]);
    await e.cropMargins(left: 10, right: 10, top: 10, bottom: 10);
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
  });

  testWidgets('editor flatten forms + annotations', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.flattenForms();
    await e.flattenAllAnnotations();
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
  });

  testWidgets('editor redaction lifecycle', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.addRedaction(
        0, const PdfRect(x: 50, y: 50, width: 100, height: 20));
    expect(await e.redactionCount(0), greaterThan(0));
    await e.applyRedactions();
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
  });

  testWidgets('editor scrubMetadata', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.scrubMetadata();
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
  });

  testWidgets('editor convertToPdfA', (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    await e.convertToPdfA();
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    final saved = await pdf.open(_src(sink.takeBytes()));
    expect(saved.pageCount, 1,
        reason: 'a save that cannot be re-opened saved nothing');
    await saved.dispose();
  });

  testWidgets('editor save options (full/incremental/encrypted/remove)',
      (t) async {
    final e = await pdf.edit(_src(minimalPdf));
    final s1 = MemorySink();
    await e.save(s1,
        options: const PdfSaveOptions.fullRewrite(
            compress: true, garbageCollect: true));
    final full = await pdf.open(_src(s1.takeBytes()));
    expect(full.pageCount, 1);
    await full.dispose();

    await e.setTitle('incremental edit');
    final s2 = MemorySink();
    await e.save(s2, options: const PdfSaveOptions.incremental());
    expect(s2.takeBytes().length, greaterThan(minimalPdf.length),
        reason: 'incremental save appends the modification after the '
            'original bytes — output must be strictly larger');

    final s3 = MemorySink();
    await e.save(s3,
        options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.config(ownerPassword: 'pw')));
    final enc = await pdf.open(_src(s3.takeBytes()));
    expect(enc.isEncrypted, isTrue,
        reason: 'the encrypted save mode must actually encrypt');
    await enc.dispose();

    final s4 = MemorySink();
    await e.save(s4,
        options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.remove()));
    final plain = await pdf.open(_src(s4.takeBytes()));
    expect(plain.isEncrypted, isFalse);
    await plain.dispose();

    await e.dispose();
  });

  // ── PdfBuilder — create from scratch ────────────────────────────

  testWidgets('builder text document (A4)', (t) async {
    final b = await pdf.build();
    await b.setTitle('T');
    await b.setAuthor('A');
    await b.setSubject('S');
    await b.setKeywords('K');
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
    final sink = MemorySink();
    await b.save(sink);
    await b.dispose();
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('builder Letter + custom size', (t) async {
    final b = await pdf.build();
    final p1 = await b.addLetterPage();
    await p1.text('Letter');
    await p1.done();
    final p2 = await b.addPage(width: 400, height: 300);
    await p2.text('Custom');
    await p2.done();
    final sink = MemorySink();
    await b.save(sink);
    await b.dispose();
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
    final sink = MemorySink();
    await b.save(sink);
    await b.dispose();
    final doc = await pdf.open(_src(sink.takeBytes()));
    expect(doc.pageCount, 2);
    await doc.dispose();
  });

  testWidgets('builder form fields (all types)', (t) async {
    final b = await pdf.build();
    final p = await b.addA4Page();
    await p.textField(
        'name', const PdfRect(x: 72, y: 700, width: 200, height: 20),
        defaultValue: 'John');
    await p.checkbox(
        'agree', const PdfRect(x: 72, y: 660, width: 14, height: 14));
    await p.comboBox('country',
        const PdfRect(x: 72, y: 620, width: 150, height: 20), ['A', 'B'],
        selected: 'A');
    await p.pushButton(
        'btn', const PdfRect(x: 72, y: 580, width: 80, height: 30), 'Click');
    await p.signatureField(
        'sig', const PdfRect(x: 72, y: 520, width: 200, height: 60));
    // radioGroup — skipped: Rust handler not yet implemented (pre-existing gap)
    await p.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true)');
    await p.fieldFormat('AFNumber_Format(2, 0, 0, 0, "", true)');
    await p.fieldValidate('AFRange_Validate(true, 0, true, 100)');
    await p.fieldCalculate('AFSimple_Calculate("SUM", new Array("f1", "f2"))');
    await p.done();
    final sink = MemorySink();
    await b.save(sink);
    await b.dispose();
    final out = String.fromCharCodes(sink.takeBytes());
    for (final field in ['name', 'agree', 'country', 'btn', 'sig']) {
      expect(out, contains(field),
          reason: 'every declared form field must exist in the file');
    }
  });

  testWidgets('builder links', (t) async {
    final b = await pdf.build();
    final p = await b.addA4Page();
    // Links anchor to the most recent text run — an empty page has
    // nothing to link.
    await p.text('Visit the site');
    await p.linkUrl('https://example.com');
    await p.text('Back to page one');
    await p.linkPage(0);
    await p.done();
    final sink = MemorySink();
    await b.save(sink);
    await b.dispose();
    expect(String.fromCharCodes(sink.takeBytes()), contains('example.com'));
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
    final sink = MemorySink();
    await e.save(sink);
    await e.dispose();
    await e.dispose();

    final b = await pdf.build();
    final p = await b.addA4Page();
    await p.text('x');
    await p.done();
    final sink2 = MemorySink();
    await b.save(sink2);
    await b.dispose();
    await b.dispose();

    // Disposed means refusing, not just surviving.
    expect(() => doc.extract(pages: const PdfPages.all()), throwsStateError);
    expect(() => e.setTitle('x'), throwsStateError);
    expect(() => b.save(MemorySink()), throwsStateError);
  });

  // ── Runtime — the lane architecture, proven on THIS device ──────

  testWidgets('task.cancel mid-op resolves PdfCancelled, instance survives',
      (t) async {
    final slow = _ParkedSource(minimalPdf);
    final task = pdf.open(slow);
    await slow.firstRead; // the engine is provably parked on the harness's read
    task.cancel();
    await expectLater(task, throwsA(isA<PdfCancelled>()));
    slow.release();
    // Only that job died — the instance keeps working.
    final doc = await pdf.open(_src(minimalPdf));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('cancel before the job starts resolves PdfCancelled', (t) async {
    final task = pdf.open(_src(minimalPdf));
    task.cancel(); // same tick — the job never reaches a lane
    await expectLater(task, throwsA(isA<PdfCancelled>()));
    final doc = await pdf.open(_src(minimalPdf));
    expect(doc.pageCount, 1);
    await doc.dispose();
  });

  testWidgets('cancel after completion is a no-op', (t) async {
    final task = pdf.open(_src(minimalPdf));
    final doc = await task;
    task.cancel();
    task.cancel(); // idempotent
    expect(doc.pageCount, 1); // handle unaffected
    await doc.dispose();
  });

  testWidgets('parallel opens land on independent lanes', (t) async {
    final docs =
        await Future.wait(List.generate(4, (_) => pdf.open(_src(minimalPdf))));
    for (final doc in docs) {
      expect(doc.pageCount, 1);
      await doc.dispose();
    }
  });

  testWidgets('instant dispose: in-flight op resolves PdfCancelled', (t) async {
    final lab = Pdf();
    final slow = _ParkedSource(minimalPdf);
    final inflight = lab.open(slow);
    await slow.firstRead;
    final sw = Stopwatch()..start();
    await lab.dispose();
    sw.stop();
    slow.release();
    await expectLater(inflight, throwsA(isA<PdfCancelled>()));
    // "Instant" = same event-loop turn; generous bound for slow CI.
    expect(sw.elapsedMilliseconds, lessThan(500),
        reason: 'dispose must not wait for in-flight work');
  });

  testWidgets('fresh instance works after an abrupt kill', (t) async {
    final lab = Pdf();
    unawaited(lab.open(_src(minimalPdf)));
    await lab.dispose(); // killed mid-open
    final lab2 = Pdf();
    final doc = await lab2.open(_src(minimalPdf));
    expect(doc.pageCount, 1);
    await doc.dispose();
    await lab2.dispose();
  });

  // ── UI journeys — the example app driven like a real user ───────
  //
  // The Runtime tab needs no file picker (a native dialog WidgetTester
  // cannot touch), so these flows are fully drivable end to end.

  testWidgets('UI: app boots and detects an I/O mode', (t) async {
    app.main();
    await t.pumpAndSettle();
    // The chip resolves from "…" to a real mode name.
    await _pumpUntil(t, () {
      final modes = ['NATIVE', 'JSPI', 'ATOMICS', 'OPFS'];
      return modes.any((m) => t.any(find.text(m)));
    });
  });

  testWidgets('UI: cancel-before-start demo reports PdfCancelled', (t) async {
    app.main();
    await t.pumpAndSettle();
    await _runDemo(t, 'Cancel before the job even starts');
    await _pumpUntil(t, () => t.any(find.textContaining('PdfCancelled')));
  });

  testWidgets('UI: parallel-opens demo reports 4 docs', (t) async {
    app.main();
    await t.pumpAndSettle();
    await _runDemo(t, 'Open 4 documents in parallel');
    await _pumpUntil(t, () => t.any(find.textContaining('4 docs open')));
  });

  testWidgets('UI: instant-dispose demo reports a measured kill', (t) async {
    app.main();
    await t.pumpAndSettle();
    await _runDemo(t, 'Dispose mid-flight — measure it');
    // Builds a 40-page sample first — give it room on slow devices.
    await _pumpUntil(t, () => t.any(find.textContaining('dispose() returned')),
        timeout: const Duration(minutes: 3));
  });

  testWidgets('UI: tab navigation shows each surface', (t) async {
    app.main();
    await t.pumpAndSettle();
    await _tapTab(t, 'Doc');
    expect(find.text('Open a PDF to query it'), findsOneWidget);
    await _tapTab(t, 'Merge');
    expect(find.textContaining('Pick 2+ PDFs'), findsOneWidget);
  });
}

/// Scrolls a Runtime-tab demo into view and taps its Run button.
///
/// The demo lists are lazy ListViews — a button below the fold isn't
/// built yet, so ensureVisible can't find it. scrollUntilVisible walks
/// the list exactly as a user's thumb would, building rows as it goes.
Future<void> _runDemo(WidgetTester t, String title) async {
  final run = find.byKey(ValueKey('run:$title'));
  // The Runtime tab body is the only ListView in the tree; the other
  // scrollables are the TabBar and the TabBarView pager (both
  // horizontal). scrollUntilVisible wants the Scrollable inside that
  // ListView, so the vertical scroll lands on the demo list.
  final list = find.descendant(
    of: find.byType(ListView),
    matching: find.byType(Scrollable),
  );
  await t.scrollUntilVisible(run, 200, scrollable: list);
  await t.tap(run);
}

/// Scrolls a tab into view, then taps it. The TabBar is scrollable, so
/// on a narrow screen the last tabs sit off the right edge — drag the
/// bar until the wanted tab is on-screen, exactly as a user would. The
/// TabBar is the first scrollable in the tree.
Future<void> _tapTab(WidgetTester t, String label) async {
  final tab = find.text(label);
  await t.scrollUntilVisible(tab, 120,
      scrollable: find.byType(Scrollable).first);
  await t.tap(tab);
  await t.pumpAndSettle();
}

/// Pumps frames until [condition] holds. pumpAndSettle would hang on
/// the status bar's spinner — poll explicitly instead.
Future<void> _pumpUntil(WidgetTester t, bool Function() condition,
    {Duration timeout = const Duration(seconds: 60)}) async {
  final end = DateTime.now().add(timeout);
  while (!condition()) {
    if (DateTime.now().isAfter(end)) {
      fail('condition not met within $timeout');
    }
    await t.pump(const Duration(milliseconds: 100));
  }
}

/// A DataSource whose first read parks until [release] — makes cancel
/// and dispose land deterministically mid-I/O (no timing luck).
class _ParkedSource implements DataSource {
  _ParkedSource(this._data);
  final Uint8List _data;
  final _firstRead = Completer<void>();
  final _gate = Completer<void>();

  Future<void> get firstRead => _firstRead.future;
  void release() {
    if (!_gate.isCompleted) _gate.complete();
  }

  @override
  int get length => _data.length;

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    if (!_firstRead.isCompleted) _firstRead.complete();
    await _gate.future;
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.sublistView(_data, offset, end);
  }
}
