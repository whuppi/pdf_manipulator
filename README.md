# pdf_manipulator

Cross-platform PDF manipulation for Dart & Flutter. Powered by [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) (Rust, MIT/Apache-2.0). Every operation runs off the main thread — worker isolate on native, Web Worker on web.

> **Upgrading from the old Android-only version?** See the [migration guide](docs/MIGRATION.md).

---

## Install

```yaml
dependencies:
  pdf_manipulator: ^1.0.0
```

That's it. Works on all platforms.

For web targets, also run once:

```sh
dart run pdf_manipulator:setup
```

---

## The 30-second version

`PdfSource` and `PdfSink` are interfaces *you* implement. The package never touches `dart:io` — you bring whatever backing store fits your platform (file, network, database, memory).

```dart
import 'dart:typed_data';
import 'package:pdf_manipulator/pdf_manipulator.dart';

// In-memory — good for tests and quick scripts
class MemorySource implements PdfSource {
  MemorySource(this._data);
  final Uint8List _data;
  @override int get length => _data.length;
  @override Uint8List readAt(int offset, int count) =>
      Uint8List.sublistView(_data, offset, (offset + count).clamp(0, _data.length));
}

class MemorySink implements PdfSink {
  final _buf = BytesBuilder(copy: false);
  @override void write(Uint8List chunk) => _buf.add(chunk);
  Uint8List takeBytes() => _buf.takeBytes();
}

final pdf = Pdf();

final source = MemorySource(pdfBytes);
final doc = await pdf.open(source);
print('${doc.pageCount} pages');

final sink = MemorySink();
await pdf.merge([source, otherSource], sink);

final text = await pdf.extractText(source);

pdf.dispose(); // done — release the worker
```

Each `Pdf()` creates its own background worker. `dispose()` releases the worker and instantly cancels all pending operations. `PdfSource` in, `PdfSink` out. No file paths, no `dart:io`. Same code on every platform.

---

## What you can do

### Open and inspect

You picked a PDF. What's inside?

```dart
final pdf = Pdf();
final doc = await pdf.open(source);
print('${doc.pageCount} pages, version ${doc.version}');
print('Title: ${doc.title}');
print('Author: ${doc.author}');
print('Tagged: ${doc.isTagged}');

for (final page in doc.pages) {
  print('Page ${page.index + 1}: ${page.effectiveWidth} × ${page.effectiveHeight} pt'
      '${page.rotation != 0 ? ", rotated ${page.rotation}°" : ""}');
}
```

Don't need the full parse? `probe` is faster:

```dart
final info = await pdf.probe(source);
// info.isValid, info.pageCount, info.isEncrypted, info.version
```

### Merge

Two board decks into one? A cover page onto a report?

```dart
await pdf.merge([coverSource, reportSource, appendixSource], outputSink);
```

Page order follows the list order.

### Split

Break a big PDF into smaller ones:

```dart
// Every 5 pages
final sinks = <MemorySink>[];
await pdf.split(source, (i) { final s = MemorySink(); sinks.add(s); return s; }, every: 5);
// → sinks[0] = pages 1-5, sinks[1] = pages 6-10, sinks[2] = pages 11-13

// By file size (max 500KB each) — returns the number of parts
final partSinks = <MemorySink>[];
final count = await pdf.splitBySize(source, (i) { final s = MemorySink(); partSinks.add(s); return s; }, maxBytes: 500000);
```

### Extract, delete, reorder, move

Pull pages out, throw pages away, shuffle them around:

```dart
// Grab pages 0 and 2 as a new PDF
await pdf.extractPages(source, outputSink, pages: [0, 2]);

// Delete page 3
await pdf.deletePages(source, outputSink, pages: [3]);

// Reverse the entire document
await pdf.reorderPages(source, outputSink,
    order: [4, 3, 2, 1, 0]);

// Move the last page to the front
await pdf.movePage(source, outputSink,
    from: 9, to: 0);
```

### Rotate

```dart
// Rotate every page 90° clockwise
await pdf.rotateAllPages(source, outputSink, degrees: 90);

// Rotate specific pages — page 0 by 180°, page 2 by 270°
await pdf.rotatePages(source, outputSink, pages: {0: 180, 2: 270});
```

### Compress

Three levels of compression in one call — stream recompression, garbage collection, and image optimization. Non-JPEG images get converted to JPEG only if the result is smaller. Resolution is preserved.

```dart
await pdf.compress(source, outputSink, imageQuality: 75);
```

### Watermark

Stamp text across every page — or just the pages you pick:

```dart
await pdf.watermark(source, outputSink,
    text: 'CONFIDENTIAL', opacity: 0.2, fontSize: 60, rotation: 45);

// Just page 0
await pdf.watermark(source, outputSink,
    text: 'DRAFT', pages: [0]);

// Positioned — exact coordinates, custom font
await pdf.watermarkPositioned(source, outputSink,
    text: 'INTERNAL',
    x: 100, y: 50, width: 400, height: 100,
    fontName: 'Courier', fontSize: 36, opacity: 0.15);
```

### Stamp annotations

```dart
await pdf.addStamp(source, outputSink,
    page: 0,
    stampType: 0,  // 0=Approved, 12=Draft, 6=Confidential
    x: 50, y: 700, width: 200, height: 50);
```

### Image stamp

Stamp an image onto a page — logos, signatures, approval seals:

```dart
await pdf.addImageStamp(source, outputSink,
    page: 0,
    imageBytes: logoPng,
    x: 50, y: 700, width: 150, height: 50);
```

### Encrypt and decrypt

```dart
// Simple encryption (AES-256, all permissions)
await pdf.encrypt(source, encryptedSink, ownerPassword: 'secret');
await pdf.decrypt(encryptedSource, decryptedSink, password: 'secret');

// Full control — algorithm + permissions
await pdf.encryptFull(source, outputSink,
    ownerPassword: 'owner',
    userPassword: 'user',
    algorithm: 2,  // 0=RC4-40, 1=RC4-128, 2=AES-128, 3=AES-256
    allowPrint: true,
    allowCopy: false,
    allowModify: false,
);
```

### Extract text

Pull text out of any PDF — all pages or just one:

```dart
final everything = await pdf.extractText(source);
final page3only = await pdf.extractText(source, page: 2);
```

### Convert to Markdown, HTML, plain text

```dart
final md = await pdf.toMarkdown(source);
final html = await pdf.toHtml(source, page: 0);
final plain = await pdf.toPlainText(source, page: 0);
```

### Search

Find text with page numbers and position rectangles:

```dart
final hits = await pdf.searchAll(source, query: 'revenue');
for (final hit in hits) {
  print('Page ${hit.page}: "${hit.text}" at (${hit.rect.x}, ${hit.rect.y})');
}

// Search one page
final pageHits = await pdf.searchPage(source, page: 0, query: 'total');
```

### Render pages to images

Turn PDF pages into raw RGBA pixels — for thumbnails, previews, or image pipelines:

```dart
final full = await pdf.renderPage(source, 0);
// full.width, full.height, full.data (Uint8List of RGBA pixels)

final fitted = await pdf.renderPageFit(source, 0, width: 800, height: 600);
final thumb = await pdf.renderPageThumbnail(source, 0, size: 150);

// Stream — one page at a time, constant memory
await for (final page in pdf.renderAllPages(source, width: 400, height: 600)) {
  // process page.data
}
```

### Extract embedded images

Pull images out of PDF pages:

```dart
// Stream — one image at a time
await for (final img in pdf.extractImages(source, 0)) {
  print('${img.width}×${img.height} ${img.format} — ${img.data.length} bytes');
}

await for (final img in pdf.extractAllImages(source)) {
  // every image from every page
}
```

### Images to PDF

Turn a stack of images into a PDF:

```dart
await pdf.imagesToPdf([jpeg1, jpeg2, png3], outputSink);
```

Each image becomes one A4 page.

### Digital signatures

Inspect, verify, and sign:

```dart
final count = await pdf.getSignatureCount(source);
final sigs = await pdf.getSignatures(source);
final allValid = await pdf.verifySignatures(source);

await pdf.sign(source, signedSink,
    certificate: p12Bytes,
    certificatePassword: 'cert-pw',
    reason: 'Approved',
    location: 'HQ');
```

### Read encryption info

```dart
final perms = await pdf.getPermissions(source);
print('Can print: ${perms.print}, can copy: ${perms.copy}');

final algo = await pdf.getEncryptionAlgorithm(source);
// -1=not encrypted, 0=RC4-40, 1=RC4-128, 2=AES-128, 3=AES-256
```

### Compliance validation

```dart
final pdfA = await pdf.validatePdfA(source);
print('Compliant: ${pdfA.compliant}, errors: ${pdfA.errors}');

final accessible = await pdf.validatePdfUa(source);
```

### Forms, annotations, redactions

```dart
await pdf.flattenForms(source, outputSink);
await pdf.applyRedactions(source, outputSink);
```

### Embed files, erase regions

```dart
await pdf.embedFile(source, outputSink,
    name: 'data.csv', fileData: csvBytes);

await pdf.eraseRegions(source, outputSink,
    page: 0,
    regions: [PdfRect(x: 100, y: 100, width: 200, height: 50)]);
```

---

## PdfEditor — parse once, mutate many, save once

When you're applying multiple changes, `PdfEditor` is more efficient — it parses the PDF once and saves once, no matter how many mutations you chain:

```dart
final editor = await Pdf.edit(source);

await editor.setTitle('Q4 Report');
await editor.setAuthor('Finance');
await editor.rotatePage(0, degrees: 90);
await editor.deletePage(4);
await editor.mergeFrom(appendixSource);
await editor.addWatermark(0, 'FINAL', opacity: 0.15);
await editor.optimizeImages(quality: 70);
await editor.flattenForms();

final resultSink = MemorySink();
await editor.saveWithOptions(resultSink, compress: true, garbageCollect: true);
editor.dispose();
```

Everything `pdf.*` can do, `PdfEditor` can do in a batch. Plus metadata setters, `cropMargins`, `convertToPdfA`, `flattenAllAnnotations`, `setFormFieldValue`, `embedFile`, `eraseRegions`, and `saveEncrypted`.

---

## PdfBuilder — create PDFs from scratch

Build new PDFs with text, headings, images, and watermarks:

```dart
final builder = await Pdf.build();
await builder.setTitle('Meeting Notes');

final page = await builder.addA4Page();
await page.heading(1, 'Q4 Planning');
await page.paragraph('We discussed the roadmap for next quarter.');
await page.space(12);
await page.horizontalRule();
await page.paragraph('Action items follow.');
await page.done();

final resultSink = MemorySink();
await builder.save(resultSink);
builder.dispose();
```

Custom sizes (`addPage(width: 400, height: 600)`), Letter pages (`addLetterPage()`), images (`page.image(pngBytes, x, y, w, h)`), watermarks (`page.watermark('DRAFT')`), and encrypted output (`builder.saveEncrypted(outputSink, ownerPassword: 'pw')`).

Form fields too:

```dart
final page = await builder.addA4Page();
await page.textField('name', 72, 700, 200, 20, defaultValue: 'Jane');
await page.checkbox('agree', 72, 660, 14, 14, checked: true);
await page.comboBox('color', 72, 620, 150, 20, ['Red', 'Green', 'Blue']);
await page.pushButton('submit', 72, 580, 80, 30, 'Submit');
await page.signatureField('sig', 72, 520, 200, 50);
await page.done();
```

---

## Error handling

Every error is a typed subclass of `PdfError`. Pattern-match, don't parse strings:

```dart
try {
  await pdf.open(source);
} on PdfPasswordRequired {
  // prompt the user
} on PdfCorrupted catch (e) {
  print('Nope: ${e.message}');
} on PdfPageRangeError catch (e) {
  print('Page ${e.page} doesn't exist (only ${e.pageCount} pages)');
}
```

11 error types: `PdfCorrupted`, `PdfPasswordRequired`, `PdfWrongPassword`, `PdfPageRangeError`, `PdfInvalidArgument`, `PdfIoError`, `PdfExtractionFailed`, `PdfUnsupported`, `PdfSearchError`, `PdfCryptoError`, `PdfEngineError`.

---

## Platform support

| Feature | Android | iOS | macOS | Windows | Linux | Web |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Open / inspect / probe | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Merge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Split (by count, size, pages) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Extract / delete / reorder pages | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Rotate pages | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Compress | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Watermark (text + positioned) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Stamp annotations (16 types + custom) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Image stamp (logo watermark) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Encrypt / decrypt (4 algorithms) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Extract text / Markdown / HTML | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Search text | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Render pages to images | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Extract embedded images | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Images to PDF | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Digital signatures | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| PdfEditor (batch mutations) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| PdfBuilder (create from scratch) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Form field creation | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| PDF/A + PDF/UA validation | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Font unembedding | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

Every feature works on every platform. Native binaries are downloaded automatically by the build hook. For web, run `dart run pdf_manipulator:setup` once.

---

## Docs

| Document | What's inside |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Source tree, three-layer design, conditional import dispatch, worker model, build hook |
| [`docs/CAPABILITY_ROADMAP.md`](docs/CAPABILITY_ROADMAP.md) | Every feature with shipped/planned status |
| [`docs/MIGRATION.md`](docs/MIGRATION.md) | Method-by-method mapping from the old Android-only package |
| [`docs/UPDATING.md`](docs/UPDATING.md) | Maintenance recipes — bump upstream, edit patches, rebuild |

---

## License

[MIT](LICENSE) — Dart code, build tooling, and package distribution.

pdf_oxide engine: [MIT/Apache-2.0](https://github.com/yfedoseev/pdf_oxide/blob/main/LICENSE-MIT).
