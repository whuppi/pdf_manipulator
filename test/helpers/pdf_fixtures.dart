import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';

/// Minimal valid PDF — one blank A4 page (595×842 points).
final Uint8List minimalPdf = _build(
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
  'startxref\n190\n%%EOF\n',
);

/// Minimal valid PDF — US Letter page (612×792 points).
final Uint8List letterPdf = _build(
  '%PDF-1.4\n'
  '1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n'
  '2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n'
  '3 0 obj\n<< /Type /Page /Parent 2 0 R '
  '/MediaBox [0 0 612 792] >>\nendobj\n'
  'xref\n0 4\n'
  '0000000000 65535 f \n'
  '0000000009 00000 n \n'
  '0000000058 00000 n \n'
  '0000000115 00000 n \n'
  'trailer\n<< /Size 4 /Root 1 0 R >>\n'
  'startxref\n190\n%%EOF\n',
);

/// Not a PDF — random bytes.
final Uint8List garbageBytes = Uint8List.fromList([1, 2, 3, 4, 5, 6, 7, 8]);

/// Empty bytes.
final Uint8List emptyBytes = Uint8List(0);

/// Almost a PDF — has header but broken structure.
final Uint8List brokenPdf = _build('%PDF-1.4\ngarbage after header\n');

Uint8List _build(String content) => Uint8List.fromList(content.codeUnits);

// ── Rich fixtures — built at test time via PdfEditor ──────────────

Future<Uint8List> buildThreePagePdf() async {
  final pdf = Pdf();
  final editor = PdfEditor(await pdf.openEditor(minimalPdf));
  await editor.mergeFrom(letterPdf);
  await editor.mergeFrom(minimalPdf);
  await editor.setTitle('Three Page Test');
  await editor.setAuthor('Test Suite');
  final result = await editor.save();
  await editor.dispose();
  pdf.kill();
  return result;
}

Future<Uint8List> buildMetadataPdf({
  String title = 'Test Title',
  String author = 'Test Author',
  String subject = 'Test Subject',
  String keywords = 'test, pdf, dart',
}) async {
  final pdf = Pdf();
  final editor = PdfEditor(await pdf.openEditor(minimalPdf));
  await editor.setTitle(title);
  await editor.setAuthor(author);
  await editor.setSubject(subject);
  await editor.setKeywords(keywords);
  final result = await editor.save();
  await editor.dispose();
  pdf.kill();
  return result;
}

Future<Uint8List> buildFivePagePdf() async {
  final pdf = Pdf();
  final editor = PdfEditor(await pdf.openEditor(minimalPdf));
  for (var i = 0; i < 4; i++) {
    await editor.mergeFrom(minimalPdf);
  }
  final result = await editor.save();
  await editor.dispose();
  pdf.kill();
  return result;
}
