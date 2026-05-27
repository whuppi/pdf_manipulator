// Rich PDF generators — create test fixtures at runtime using PdfBuilder.
// No file I/O, no platform dependency. Works on native and web.

import 'dart:math';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';

import 'photo_png.dart';
import 'test_source_sink.dart';

/// Build a PDF with [pageCount] pages using exponential merge.
Future<Uint8List> buildLargePdf(Pdf Function() createPdf, {int pageCount = 100}) async {
  final pdf = createPdf();

  final builder = await pdf.build();
  await builder.setTitle('Stress Test');
  for (var i = 0; i < 10; i++) {
    final page = await builder.addPage(width: 612, height: 792);
    await page.font('Helvetica', 14);
    await page.heading(1, 'Page ${i + 1}');
    await page.paragraph(
      'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '
      'Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. '
      'Page seed: ${i * 7 + 42}. Random: ${Random(i).nextInt(99999)}.');
    await page.paragraph(
      'Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris '
      'nisi ut aliquip ex ea commodo consequat.');
    await page.done();
  }
  final seedSink = TestSink();
  await builder.save(seedSink);
  await builder.dispose();
  var current = seedSink.takeBytes();

  var currentPages = 10;
  while (currentPages < pageCount) {
    final sink = TestSink();
    final copies = (pageCount / currentPages).ceil().clamp(2, 10);
    final sources = List.generate(copies, (_) => src(current));
    await pdf.merge(sources, sink);
    current = sink.takeBytes();
    currentPages *= copies;
  }

  if (currentPages > pageCount) {
    final sink = TestSink();
    await pdf.extractPages(src(current), sink,
        pages: List.generate(pageCount, (i) => i));
    current = sink.takeBytes();
  }

  return current;
}

/// Build a PDF with [pageCount] pages, each containing a 64x64 photo-like PNG.
/// Uses buildPhotoPng() which produces noisy gradient data — large enough that
/// JPEG compression at quality 50 beats the Flate-compressed PNG stream.
Future<Uint8List> buildImagePdf(Pdf Function() createPdf, {int pageCount = 20}) async {
  final pdf = createPdf();
  final builder = await pdf.build();
  await builder.setTitle('Image Test — $pageCount pages');
  for (var i = 0; i < pageCount; i++) {
    final page = await builder.addPage(width: 612, height: 792);
    await page.font('Helvetica', 12);
    await page.text('Image page ${i + 1}');
    await page.image(src(photoPng), PdfRect(
      x: 50, y: 100,
      width: 100 + (i % 5) * 40.0,
      height: 100 + (i % 3) * 60.0,
    ));
    await page.done();
  }

  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

/// Build a PDF with pages of varying sizes — some small (A5), some large (A3).
Future<Uint8List> buildVariedSizePdf(Pdf Function() createPdf, {int pageCount = 50}) async {
  final pdf = createPdf();
  final builder = await pdf.build();
  await builder.setTitle('Varied Size Test');

  for (var i = 0; i < pageCount; i++) {
    final isLarge = i % 3 == 0;
    final w = isLarge ? 841.89 : 420.94;
    final h = isLarge ? 1190.55 : 595.28;
    final page = await builder.addPage(width: w, height: h);
    await page.font('Helvetica', isLarge ? 18 : 10);
    await page.heading(1, '${isLarge ? "LARGE" : "small"} page ${i + 1}');
    if (isLarge) {
      for (var p = 0; p < 5; p++) {
        await page.paragraph(
          'Dense content block $p on large page $i. '
          'This adds significant byte weight to force splitBySize '
          'to create more chunks when pages vary in size. '
          'Repetition seed: ${i * 100 + p}.');
      }
    } else {
      await page.text('Minimal content on small page $i.');
    }
    await page.done();
  }

  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

/// Build a multi-chapter PDF with form fields.
Future<Uint8List> buildFormPdf(Pdf Function() createPdf) async {
  final pdf = createPdf();
  final builder = await pdf.build();
  await builder.setTitle('Form Test');

  final page = await builder.addPage(width: 612, height: 792);
  await page.font('Helvetica', 14);
  await page.heading(1, 'Application Form');
  await page.space(10);
  await page.text('Name:');
  await page.textField('name', const PdfRect(x: 100, y: 680, width: 200, height: 20));
  await page.text('Email:');
  await page.textField('email', const PdfRect(x: 100, y: 640, width: 200, height: 20));
  await page.text('Agree to terms:');
  await page.checkbox('agree', const PdfRect(x: 100, y: 600, width: 20, height: 20));
  await page.done();

  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}
