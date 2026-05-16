# pdf_manipulator

Cross-platform PDF manipulation for Dart & Flutter. Powered by [pdf_oxide](https://github.com/nickhimself/pdf_oxide) (Rust, MIT/Apache-2.0). Every operation runs off the main thread — worker isolate on native, Web Worker on web.

> **Upgrading from the old Android-only version?** See the [migration guide](docs/MIGRATION.md).

---

## Install

### Consumer (zero Rust needed)

```yaml
dependencies:
  pdf_manipulator: ^1.0.0
```

The build hook automatically downloads a pre-built binary from GitHub Releases for your platform. No Rust toolchain, no compilation — just `dart pub get` and go.

For web, also run once from your app directory:

```sh
dart run pdf_manipulator:setup
```

This copies the WASM binary + Web Worker into `web/pdf_manipulator/`.

### Contributor (needs Rust)

```bash
git clone --recursive https://github.com/whuppi/pdf_manipulator.git
cd pdf_manipulator
dart pub get
dart test  # build hook compiles from source automatically
```

Requires Rust ([rustup.rs](https://rustup.rs)). The build hook detects `vendor/pdf_oxide/` and runs `cargo build` — no manual compilation step.

---

## The 30-second version

```dart
import 'package:pdf_manipulator/pdf_manipulator.dart';

final pdf = Pdf();

final doc = await pdf.open(pdfBytes);
print('${doc.pageCount} pages');

final merged = await pdf.merge([pdfA, pdfB]);
final smaller = await pdf.compress(bytes, imageQuality: 75);
final text = await pdf.extractText(bytes);
final locked = await pdf.encrypt(bytes, ownerPassword: 'secret');

pdf.kill(); // done — release the worker
```

Each `Pdf()` creates its own background worker. `kill()` releases the worker and instantly cancels all pending operations. Bytes in, bytes out. No file paths, no `dart:io`. Same code on every platform.

---

## What you can do

### Open and inspect

You picked a PDF. What's inside?

```dart
final pdf = Pdf();
final doc = await pdf.open(bytes);
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
final info = await pdf.probe(bytes);
// info.isValid, info.pageCount, info.isEncrypted, info.version
```

### Merge

Two board decks into one? A cover page onto a report?

```dart
final merged = await pdf.merge([coverBytes, reportBytes, appendixBytes]);
```

Page order follows the list order.

### Split

Break a big PDF into smaller ones:

```dart
// Every 5 pages
final chunks = await pdf.split(bytes, every: 5);
// → [pages 1-5, pages 6-10, pages 11-13]

// By file size (max 500KB each)
final small = await pdf.splitBySize(bytes, maxBytes: 500000);
```

### Extract, delete, reorder, move

Pull pages out, throw pages away, shuffle them around:

```dart
// Grab pages 0 and 2 as a new PDF
final excerpt = await pdf.extractPages(bytes, pages: [0, 2]);

// Delete page 3
final trimmed = await pdf.deletePages(bytes, pages: [3]);

// Reverse the entire document
final backwards = await pdf.reorderPages(bytes,
    order: [4, 3, 2, 1, 0]);

// Move the last page to the front
final reshuffled = await pdf.movePage(bytes,
    from: 9, to: 0);
```

### Rotate

```dart
// Rotate every page 90° clockwise
final landscape = await pdf.rotateAllPages(bytes, degrees: 90);

// Rotate specific pages — page 0 by 180°, page 2 by 270°
final fixed = await pdf.rotatePages(bytes, pages: {0: 180, 2: 270});
```

### Compress

Three levels of compression in one call — stream recompression, garbage collection, and image optimization. Non-JPEG images get converted to JPEG only if the result is smaller. Resolution is preserved.

```dart
final smaller = await pdf.compress(bytes, imageQuality: 75);
print('${bytes.length} → ${smaller.length}');
```

### Watermark

Stamp text across every page — or just the pages you pick:

```dart
final stamped = await pdf.watermark(bytes,
    text: 'CONFIDENTIAL', opacity: 0.2, fontSize: 60, rotation: 45);

// Just page 0
final partial = await pdf.watermark(bytes,
    text: 'DRAFT', pages: [0]);

// Positioned — exact coordinates, custom font
final precise = await pdf.watermarkPositioned(bytes,
    text: 'INTERNAL',
    x: 100, y: 50, width: 400, height: 100,
    fontName: 'Courier', fontSize: 36, opacity: 0.15);
```

### Stamp annotations

```dart
final stamped = await pdf.addStamp(bytes,
    page: 0,
    stampType: 0,  // 0=Approved, 12=Draft, 6=Confidential
    x: 50, y: 700, width: 200, height: 50);
```

### Image stamp

Stamp an image onto a page — logos, signatures, approval seals:

```dart
final stamped = await pdf.addImageStamp(bytes,
    page: 0,
    imageBytes: logoPng,
    x: 50, y: 700, width: 150, height: 50);
```

### Encrypt and decrypt

```dart
// Simple encryption (AES-256, all permissions)
final locked = await pdf.encrypt(bytes, ownerPassword: 'secret');
final unlocked = await pdf.decrypt(locked, password: 'secret');

// Full control — algorithm + permissions
final restricted = await pdf.encryptFull(bytes,
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
final everything = await pdf.extractText(bytes);
final page3only = await pdf.extractText(bytes, page: 2);
```

### Convert to Markdown, HTML, plain text

```dart
final md = await pdf.toMarkdown(bytes);
final html = await pdf.toHtml(bytes, page: 0);
final plain = await pdf.toPlainText(bytes, page: 0);
```

### Search

Find text with page numbers and position rectangles:

```dart
final hits = await pdf.searchAll(bytes, query: 'revenue');
for (final hit in hits) {
  print('Page ${hit.page}: "${hit.text}" at (${hit.rect.x}, ${hit.rect.y})');
}

// Search one page
final pageHits = await pdf.searchPage(bytes, page: 0, query: 'total');
```

### Render pages to images

Turn PDF pages into raw RGBA pixels — for thumbnails, previews, or image pipelines:

```dart
final full = await pdf.renderPage(bytes, 0);
// full.width, full.height, full.data (Uint8List of RGBA pixels)

final fitted = await pdf.renderPageFit(bytes, 0, width: 800, height: 600);
final thumb = await pdf.renderPageThumbnail(bytes, 0, size: 150);
final all = await pdf.renderAllPages(bytes, width: 400, height: 600);
```

### Extract embedded images

Pull images out of PDF pages:

```dart
final images = await pdf.extractImages(bytes, 0);
for (final img in images) {
  print('${img.width}×${img.height} ${img.format} — ${img.data.length} bytes');
}

final allImages = await pdf.extractAllImages(bytes);
```

### Images to PDF

Turn a stack of images into a PDF:

```dart
final result = await pdf.imagesToPdf([jpeg1, jpeg2, png3]);
```

Each image becomes one A4 page.

### Digital signatures

Inspect, verify, and sign:

```dart
final count = await pdf.getSignatureCount(bytes);
final sigs = await pdf.getSignatures(bytes);
final allValid = await pdf.verifySignatures(bytes);

final signed = await pdf.sign(bytes,
    certificate: p12Bytes,
    certificatePassword: 'cert-pw',
    reason: 'Approved',
    location: 'HQ');
```

### Read encryption info

```dart
final perms = await pdf.getPermissions(bytes);
print('Can print: ${perms.print}, can copy: ${perms.copy}');

final algo = await pdf.getEncryptionAlgorithm(bytes);
// -1=not encrypted, 0=RC4-40, 1=RC4-128, 2=AES-128, 3=AES-256
```

### Compliance validation

```dart
final pdfA = await pdf.validatePdfA(bytes);
print('Compliant: ${pdfA.compliant}, errors: ${pdfA.errors}');

final accessible = await pdf.validatePdfUa(bytes);
```

### Forms, annotations, redactions

```dart
final flat = await pdf.flattenForms(bytes);
final redacted = await pdf.applyRedactions(bytes);
```

### Embed files, erase regions

```dart
final withAttachment = await pdf.embedFile(bytes,
    name: 'data.csv', fileData: csvBytes);

final erased = await pdf.eraseRegions(bytes,
    page: 0,
    regions: [PdfRect(x: 100, y: 100, width: 200, height: 50)]);
```

---

## PdfEditor — parse once, mutate many, save once

When you're applying multiple changes, `PdfEditor` is more efficient — it parses the PDF once and saves once, no matter how many mutations you chain:

```dart
final pdf = Pdf();
final editor = PdfEditor(await pdf.openEditor(bytes));

await editor.setTitle('Q4 Report');
await editor.setAuthor('Finance');
await editor.rotatePage(0, degrees: 90);
await editor.deletePage(4);
await editor.mergeFrom(appendixBytes);
await editor.addWatermark(0, 'FINAL', opacity: 0.15);
await editor.optimizeImages(quality: 70);
await editor.flattenForms();

final result = await editor.saveWithOptions(compress: true, garbageCollect: true);
await editor.dispose();
```

Everything `pdf.*` can do, `PdfEditor` can do in a batch. Plus metadata setters, `cropMargins`, `convertToPdfA`, `flattenAllAnnotations`, `setFormFieldValue`, `embedFile`, `eraseRegions`, and `saveEncrypted`.

---

## PdfBuilder — create PDFs from scratch

Build new PDFs with text, headings, images, and watermarks:

```dart
final pdf = Pdf();
final builder = PdfBuilder(await pdf.createBuilder());
await builder.setTitle('Meeting Notes');

final page = await builder.addA4Page();
await page.heading(1, 'Q4 Planning');
await page.paragraph('We discussed the roadmap for next quarter.');
await page.space(12);
await page.horizontalRule();
await page.paragraph('Action items follow.');
await page.done();

final result = await builder.build();
await builder.dispose();
```

Custom sizes (`addPage(width: 400, height: 600)`), Letter pages (`addLetterPage()`), images (`page.image(pngBytes, x, y, w, h)`), watermarks (`page.watermark('DRAFT')`), and encrypted output (`builder.buildEncrypted(ownerPassword: 'pw')`).

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
  await pdf.open(mysteryBytes);
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

| Platform | Status |
|---|---|
| macOS | Tested |
| iOS | Build ready |
| Android | Build ready |
| Windows | Build ready |
| Linux | Build ready |
| Web (Chrome) | Tested |

Native: the build hook downloads a pre-built binary from GitHub Releases (consumer path) or uses a locally compiled binary if present (contributor path). Web: WASM + Web Worker via `dart run pdf_manipulator:setup`.

---

## How it works

Each `Pdf()` instance spawns its own background worker — a worker isolate on native, a Web Worker on web. The Dart layer is a typed facade over pdf_oxide, a Rust PDF engine compiled to native libraries and WASM. The compiler picks the right implementation at build time via conditional imports. `dart:ffi` never touches the public API. `dart:io` is never imported.

`kill()` tears down the worker and instantly cancels all pending operations on that instance. After `kill()`, the instance throws on any further call.

Input and output are `Uint8List`. The consumer handles I/O; the package handles PDFs.

FFI bindings are generated by `ffigen` from pdf_oxide's C header (329 functions). 29 C-ABI + 8 Rust-level patches on the vendored fork add missing functions. Detailed architecture, capability roadmap, CI/CD setup, and maintenance recipes live in [`docs/`](docs/).

---

## License

MIT — package, Dart code, and build tooling.

pdf_oxide engine: MIT/Apache-2.0.
