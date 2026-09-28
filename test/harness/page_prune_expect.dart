import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../fixtures/handwritten.dart';
import 'test_source_sink.dart';

/// Proves that [out], made from [pagePrunePdf], holds the pages in [kept]
/// (in that order) and nothing of the others (#261).
///
/// A dropped page's content stream and field value are absent from the raw
/// bytes: the save copies an `ASCIIHexDecode` stream and a string byte for
/// byte, so their absence means the objects were not written. Each kept page
/// still shows its text and the media box it inherited from an inner
/// `/Pages` node, and its field is the only one left.
Future<void> expectOnlyPrunedPages(
  Pdf pdf,
  Uint8List out,
  List<int> kept,
) async {
  // bytegrep-exempt: a leak negative, the claim is about written bytes.
  final raw = String.fromCharCodes(out); // bytegrep-exempt
  final truth = pagePruneTruth;
  for (var i = 0; i < truth.pageCount; i++) {
    final keep = kept.contains(i);
    expect(
      raw.contains(truth.contentHex(i)),
      keep,
      reason:
          'page $i content stream ${keep ? 'kept' : 'written though dropped'}',
    );
    expect(
      raw.contains(truth.value(i)),
      keep,
      reason: 'page $i field ${keep ? 'kept' : 'written though dropped'}',
    );
  }

  final doc = await pdf.open(src(out));
  expect(doc.pageCount, kept.length);
  for (var p = 0; p < kept.length; p++) {
    final text = await doc.extract(pages: PdfPages.single(p));
    expect(text, contains(truth.marker(kept[p])));
  }
  final fields = (await doc.formFields).map((f) => f.name).toSet();
  expect(fields, {for (final i in kept) 'field_$i'});
  await doc.dispose();

  final editor = await pdf.edit(src(out));
  for (var p = 0; p < kept.length; p++) {
    final box = await editor.pageMediaBox(p);
    expect(
      (box.width, box.height),
      truth.mediaBox,
      reason: 'page $p lost the media box it inherited',
    );
  }
  await editor.dispose();
}
