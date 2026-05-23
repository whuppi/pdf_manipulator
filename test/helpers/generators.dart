// Rich PDF generators — create test fixtures at runtime using PdfBuilder.
// No file I/O, no platform dependency. Works on native and web.

import 'dart:math';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/transport/bridge.dart';

import 'test_source_sink.dart';

/// Generate a unique tiny PNG for each page (different color per page).
Uint8List _generateColorPng(int seed) {
  final r = (seed * 37 + 50) % 256;
  final g = (seed * 73 + 100) % 256;
  final b = (seed * 113 + 150) % 256;
  // 1x1 pixel PNG — smallest valid PNG with a specific color.
  // IHDR: 1x1, 8-bit RGB. IDAT: zlib-compressed single pixel.
  final raw = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
    // IHDR chunk
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE,
    // IDAT chunk (zlib-compressed 1px RGB)
    0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54,
    0x08, 0xD7, 0x63, 0x60 + (r >> 6), (r & 0x3F) << 2,
    (g & 0xFF), (b & 0xFF), 0x00, 0x00, 0x00, 0x04,
    0x00, 0x01,
    // CRC placeholder
    0x00, 0x00, 0x00, 0x00,
    // IEND
    0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
    0xAE, 0x42, 0x60, 0x82,
  ];
  return Uint8List.fromList(raw);
}

/// Build a PDF with [pageCount] pages, each containing text + a unique image.
Future<Uint8List> buildLargePdf(PdfBridge Function() b, {int pageCount = 100}) async {
  final bridge = b();
  final builder = await bridge.createBuilder();
  await builder.setTitle('Stress Test — $pageCount pages');
  await builder.setAuthor('pdf_manipulator test');

  for (var i = 0; i < pageCount; i++) {
    final page = await builder.addPage(width: 612, height: 792);
    await page.font('Helvetica', 14);
    await page.heading(1, 'Page ${i + 1} of $pageCount');
    await page.paragraph(
      'Lorem ipsum dolor sit amet, consectetur adipiscing elit. '
      'Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. '
      'Page seed: ${i * 7 + 42}. Random: ${Random(i).nextInt(99999)}.');
    await page.space(20);
    await page.paragraph(
      'Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris '
      'nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in '
      'reprehenderit in voluptate velit esse cillum dolore eu fugiat.');
    await page.done();
  }

  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

/// Build a PDF with [pageCount] pages containing images.
Future<Uint8List> buildImagePdf(PdfBridge Function() b, {int pageCount = 20}) async {
  final bridge = b();
  final builder = await bridge.createBuilder();
  await builder.setTitle('Image Test — $pageCount pages');

  final img = _generateColorPng(0);

  for (var i = 0; i < pageCount; i++) {
    final page = await builder.addPage(width: 612, height: 792);
    await page.font('Helvetica', 12);
    await page.text('Image page ${i + 1}');
    await page.image(img, const PdfRect(x: 50, y: 100, width: 200, height: 200));
    await page.done();
  }

  final sink = TestSink();
  await builder.save(sink);
  await builder.dispose();
  return sink.takeBytes();
}

/// Build a multi-chapter PDF with form fields.
Future<Uint8List> buildFormPdf(PdfBridge Function() b) async {
  final bridge = b();
  final builder = await bridge.createBuilder();
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
