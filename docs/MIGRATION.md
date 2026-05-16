# Migrating from v0 to v1

The v1 rewrite changes every API surface. This guide maps every old call to its new equivalent.

---

## The big picture

| v0 | v1 |
|---|---|
| Android only | iOS, Android, macOS, Windows, Linux, web |
| File paths in, file paths out | `Uint8List` in, `Uint8List` out |
| `PdfManipulator()` instance + params objects | `Pdf()` instance methods with named args |
| No cleanup required | Call `pdf.dispose()` when done |
| `PlatformException` on error | Typed `PdfError` sealed class |
| Method channel (Kotlin ↔ Dart) | FFI (native) / WASM (web), off main thread |

### The instance-based API

v1 uses an instance-based API. Each `Pdf()` instance owns its own worker (isolate on native, Web Worker on web). You create an instance, call methods on it, and dispose it when done.

```dart
final pdf = Pdf();
try {
  final merged = await pdf.merge([bytesA, bytesB]);
  await File('/path/merged.pdf').writeAsBytes(merged);
} finally {
  pdf.dispose();
}
```

Call `pdf.dispose()` when you're done — in a `finally` block, in a widget's `dispose()`, or wherever cleanup belongs. Each instance is independent; you can run multiple instances in parallel for concurrent work.

### The I/O change

v0 worked with file paths — you passed a path, the plugin wrote a temp file, returned the path. v1 works with bytes — you read the file yourself, pass the bytes, get bytes back, write them yourself.

```dart
// v0 — paths
final resultPath = await PdfManipulator().mergePDFs(
  params: PDFMergerParams(pdfsPaths: ['/path/a.pdf', '/path/b.pdf']),
);

// v1 — bytes
final pdf = Pdf();
final bytesA = await File('/path/a.pdf').readAsBytes();
final bytesB = await File('/path/b.pdf').readAsBytes();
final merged = await pdf.merge([bytesA, bytesB]);
await File('/path/merged.pdf').writeAsBytes(merged);
pdf.dispose();
```

This means v1 works on web (no file system) and gives you full control over where files come from and go.

---

## Method-by-method migration

### Merge

```dart
// v0
final plugin = PdfManipulator();
final path = await plugin.mergePDFs(
  params: PDFMergerParams(pdfsPaths: [pathA, pathB]),
);

// v1
final pdf = Pdf();
final merged = await pdf.merge([bytesA, bytesB]);
pdf.dispose();
```

### Split

```dart
// v0 — by page count
final paths = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, pageCount: 2),
);

// v1 — by page count
final pdf = Pdf();
final chunks = await pdf.split(bytes, every: 2);

// v0 — by byte size
final paths = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, byteSize: 500000),
);

// v1 — by byte size
final chunks = await pdf.splitBySize(bytes, maxBytes: 500000);

// v0 — by page numbers
final paths = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, pageNumbers: [3, 7]),
);

// v1 — extract specific pages (0-based)
final subset = await pdf.extractPages(bytes, pages: [0, 1, 2]);
pdf.dispose();
```

### Delete pages

```dart
// v0 — 1-based page numbers
final path = await plugin.pdfPageDeleter(
  params: PDFPageDeleterParams(pdfPath: path, pageNumbers: [2, 4]),
);

// v1 — 0-based indices
final pdf = Pdf();
final result = await pdf.deletePages(bytes, pages: [1, 3]);
pdf.dispose();
```

**Note:** v0 used 1-based page numbers. v1 uses 0-based indices throughout.

### Reorder pages

```dart
// v0 — 1-based page numbers in desired order
final path = await plugin.pdfPageReorder(
  params: PDFPageReorderParams(
    pdfPath: path,
    pageNumbers: [3, 1, 2],
  ),
);

// v1 — 0-based indices in desired order
final pdf = Pdf();
final result = await pdf.reorderPages(bytes, order: [2, 0, 1]);
pdf.dispose();
```

### Rotate pages

```dart
// v0
final path = await plugin.pdfPageRotator(
  params: PDFPageRotatorParams(
    pdfPath: path,
    pagesRotationInfo: [
      PageRotationInfo(pageNumber: 1, rotationAngle: 90),
      PageRotationInfo(pageNumber: 3, rotationAngle: 180),
    ],
  ),
);

// v1 — Map of 0-based index → degrees
final pdf = Pdf();
final result = await pdf.rotatePages(bytes, pages: {0: 90, 2: 180});

// v1 — rotate all pages at once
final result = await pdf.rotateAllPages(bytes, degrees: 90);
pdf.dispose();
```

### Rotate + delete + reorder (combined)

```dart
// v0 — one call did all three
final path = await plugin.pdfPageRotatorDeleterReorder(
  params: PDFPageRotatorDeleterReorderParams(
    pdfPath: path,
    pagesRotationInfo: [PageRotationInfo(pageNumber: 1, rotationAngle: 90)],
    pageNumbersForDeleter: [2, 4],
    pageNumbersForReorder: [3, 1, 2],
  ),
);

// v1 — chain separate calls, or use PdfEditor for one-parse-save
final pdf = Pdf();
final editor = await Pdf.edit(bytes);
await editor.rotatePage(0, degrees: 90);
await editor.deletePage(3);   // delete in descending order
await editor.deletePage(1);
// reorder via extractPages if needed
final result = await editor.save();
await editor.dispose();
pdf.dispose();
```

### Compress

```dart
// v0
final path = await plugin.pdfCompressor(
  params: PDFCompressorParams(
    pdfPath: path,
    imageQuality: 70,
    imageScale: 0.5,    // v1 does not scale — preserves resolution
    unEmbedFonts: true, // v1 has this via PdfEditor: editor.unembedStandardFonts()
  ),
);

// v1 — stream recompression + GC + image optimization
final pdf = Pdf();
final result = await pdf.compress(bytes, imageQuality: 70);
pdf.dispose();
```

**Changed params:** `imageScale` doesn't exist in v1 — pdf_oxide optimizes images by converting non-JPEG to JPEG when smaller, without reducing resolution. `unEmbedFonts` is now a separate method on `PdfEditor`:

```dart
final editor = await Pdf.edit(bytes);
final count = await editor.unembedStandardFonts();
final result = await editor.save();
await editor.dispose();
```

### Watermark

```dart
// v0
final path = await plugin.pdfWatermark(
  params: PDFWatermarkParams(
    pdfPath: path,
    text: 'DRAFT',
    fontSize: 80,
    watermarkLayer: WatermarkLayer.overContent,
    opacity: 0.3,
    positionType: PositionType.center,
    // customPosition for PositionType.custom
  ),
);

// v1
final pdf = Pdf();
final result = await pdf.watermark(
  bytes,
  text: 'DRAFT',
  fontSize: 80,
  opacity: 0.3,
  rotation: 45,
);
pdf.dispose();
```

**Replaced:** `positionType` and `customPosition` → use `pdf.watermarkPositioned(bytes, text: ..., x: ..., y: ..., width: ..., height: ...)` for exact positioning with `FixedPrint` annotation. `watermarkLayer` → watermarks are annotations (always visible).

### Encrypt

```dart
// v0
final path = await plugin.pdfEncryption(
  params: PDFEncryptionParams(
    pdfPath: path,
    userPassword: 'user',
    ownerPassword: 'owner',
    allowPrinting: true,
    allowModifyContents: false,
    allowCopy: true,
    allowModifyAnnotations: false,
    standardEncryptionAES40: false,
    standardEncryptionAES128: true,
    encryptionAES256: false,
  ),
);

// v1 — simple (AES-256, all permissions)
final pdf = Pdf();
final result = await pdf.encrypt(
  bytes,
  ownerPassword: 'owner',
  userPassword: 'user',
);

// v1 — full control (algorithm + permissions)
final result = await pdf.encryptFull(
  bytes,
  ownerPassword: 'owner',
  userPassword: 'user',
  algorithm: 2,  // 0=RC4-40, 1=RC4-128, 2=AES-128, 3=AES-256
  allowPrint: true,
  allowCopy: true,
  allowModify: false,
  allowAnnotate: false,
);
pdf.dispose();
```

All old permission flags and encryption algorithm choices are supported via `pdf.encryptFull`. v1 adds four additional permission flags not in v0: `allowPrintHq`, `allowFillForms`, `allowAccessibility`, `allowAssemble`.

### Decrypt

```dart
// v0
final path = await plugin.pdfDecryption(
  params: PDFDecryptionParams(pdfPath: path, password: 'pw'),
);

// v1
final pdf = Pdf();
final result = await pdf.decrypt(bytes, password: 'pw');
pdf.dispose();
```

### Images to PDF

```dart
// v0 — file paths, optional single/multi PDF
final paths = await plugin.imagesToPdfs(
  params: ImagesToPDFsParams(
    imagesPaths: ['/img1.jpg', '/img2.png'],
    createSinglePdf: true,
  ),
);

// v1 — bytes in, one PDF out (always single)
final pdf = Pdf();
final imageBytes1 = await File('/img1.jpg').readAsBytes();
final imageBytes2 = await File('/img2.png').readAsBytes();
final result = await pdf.imagesToPdf([imageBytes1, imageBytes2]);
pdf.dispose();
```

**Dropped param:** `createSinglePdf` — v1 always creates one PDF. To create separate PDFs per image, call `pdf.imagesToPdf` once per image.

### Page size info

```dart
// v0
final sizes = await plugin.pdfPagesSize(
  params: PDFPagesSizeParams(pdfPath: path),
);
for (final s in sizes!) {
  print('Page ${s.pageNumber}: ${s.widthOfPage} x ${s.heightOfPage}');
}

// v1
final pdf = Pdf();
final doc = await pdf.open(bytes);
for (final page in doc.pages) {
  print('Page ${page.index + 1}: ${page.effectiveWidth} x ${page.effectiveHeight}');
}
pdf.dispose();
```

**Type change:** `PageSizeInfo` → `PdfPageInfo`. Fields renamed: `pageNumber` → `index` (0-based), `widthOfPage` → `width`, `heightOfPage` → `height`. Added: `rotation`, `effectiveWidth`, `effectiveHeight` (which swap on 90°/270° rotation).

### Validity and protection info

```dart
// v0
final info = await plugin.pdfValidityAndProtection(
  params: PDFValidityAndProtectionParams(pdfPath: path),
);
print('Valid: ${info?.isPDFValid}');
print('Owner protected: ${info?.isOwnerPasswordProtected}');

// v1
final pdf = Pdf();
final info = await pdf.probe(bytes);
print('Valid: ${info.isValid}');
print('Encrypted: ${info.isEncrypted}');
print('Pages: ${info.pageCount}');
pdf.dispose();
```

**Type change:** `PdfValidityAndProtection` → `PdfInfo`. `probe` reports `isValid`, `pageCount`, `isEncrypted`, `version`, `isTagged`. For fine-grained permission flags, use `pdf.getPermissions(bytes)` which returns all 8 flags (print, copy, modify, annotate, fill-forms, accessibility, assemble, print-hq). For encryption algorithm, use `pdf.getEncryptionAlgorithm(bytes)`.

### Cancel manipulations

```dart
// v0
await plugin.cancelManipulations();

// v1 — dispose the instance's worker
final pdf = Pdf();
// ... do work ...
pdf.dispose();
// Each Pdf() instance owns its own worker. Disposing it frees the isolate/Web Worker.
```

---

## Error handling

```dart
// v0
try {
  await plugin.mergePDFs(params: params);
} on PlatformException catch (e) {
  print(e.message);
}

// v1
final pdf = Pdf();
try {
  await pdf.merge([bytesA, bytesB]);
} on PdfPasswordRequired {
  // prompt user
} on PdfCorrupted catch (e) {
  print(e.message);
} on PdfError catch (e) {
  print(e);
} finally {
  pdf.dispose();
}
```

---

## Types removed

| v0 type | v1 replacement |
|---|---|
| `PdfManipulator` (instance + params) | `Pdf()` instance with direct methods |
| `PDFMergerParams` | Direct args: `pdf.merge(inputs)` |
| `PDFSplitterParams` | `pdf.split(bytes, every:)` / `pdf.splitBySize(bytes, maxBytes:)` / `pdf.extractPages(bytes, pages:)` |
| `PDFPageDeleterParams` | `pdf.deletePages(bytes, pages:)` |
| `PDFPageReorderParams` | `pdf.reorderPages(bytes, order:)` |
| `PDFPageRotatorParams` | `pdf.rotatePages(bytes, pages:)` |
| `PageRotationInfo` | `Map<int, int>` (page index → degrees) |
| `PDFPageRotatorDeleterReorderParams` | `PdfEditor` chained calls (via `Pdf.edit(bytes)`) |
| `PDFCompressorParams` | `pdf.compress(bytes, imageQuality:)` |
| `PDFWatermarkParams` | `pdf.watermark(bytes, text:, opacity:, ...)` |
| `WatermarkLayer` | Removed — watermark is always an annotation |
| `PositionType` | Removed — use `pdf.watermarkPositioned` for exact coordinates |
| `PDFEncryptionParams` | `pdf.encrypt(bytes, ownerPassword:, userPassword:)` |
| `PDFDecryptionParams` | `pdf.decrypt(bytes, password:)` |
| `ImagesToPDFsParams` | `pdf.imagesToPdf(imageBytesList)` |
| `PDFPagesSizeParams` | `pdf.open(bytes)` → `doc.pages` |
| `PageSizeInfo` | `PdfPageInfo` |
| `PDFValidityAndProtectionParams` | `pdf.probe(bytes)` |
| `PdfValidityAndProtection` | `PdfInfo` |
| `PlatformException` | `PdfError` sealed class |
| `PdfEditor.open(bytes)` (static factory) | `await Pdf.edit(bytes)` (static factory, owns its own worker) |
| `PdfBuilder.create()` (static factory) | `await Pdf.build()` (static factory, owns its own worker) |

---

## Indexing change

v0 used **1-based** page numbers everywhere. v1 uses **0-based** indices everywhere. When migrating, subtract 1 from every page number.

```dart
// v0: page 1 = first page
PDFPageDeleterParams(pageNumbers: [1, 3])

// v1: page 0 = first page
pdf.deletePages(bytes, pages: [0, 2])
```

---

## Platform change

v0 was Android-only. v1 runs on every platform Flutter supports. If your app had platform checks like `if (Platform.isAndroid)` around PDF calls, remove them.

---

## New features with no v0 equivalent

These are all new in v1 — no migration needed, just start using them:

- `pdf.extractText`, `pdf.toMarkdown`, `pdf.toHtml`, `pdf.toPlainText`
- `pdf.searchPage`, `pdf.searchAll`
- `pdf.renderPage`, `pdf.renderPageFit`, `pdf.renderPageThumbnail`, `pdf.renderAllPages`
- `pdf.extractImages`, `pdf.extractAllImages`
- `pdf.getSignatureCount`, `pdf.getSignatures`, `pdf.verifySignatures`, `pdf.sign`
- `pdf.validatePdfA`, `pdf.validatePdfUa`
- `pdf.flattenForms`, `pdf.applyRedactions`
- `pdf.embedFile`, `pdf.eraseRegions`
- `pdf.getPermissions`, `pdf.getEncryptionAlgorithm` — read encryption info from existing PDFs
- `PdfEditor.unembedStandardFonts()` — remove embedded standard 14 fonts to reduce file size
- `pdf.watermarkPositioned` — watermark with exact coordinates, font name, and FixedPrint annotation
- `pdf.addImageStamp` / `PdfEditor.addImageStamp` — stamp images (logos, signatures) onto pages
- `PdfEditor.addStamp` — stamp annotations (Approved, Draft, Confidential, etc.)
- `PdfEditor.resizeImage` — resize images on page (DPI control)
- `PdfEditor` — batch mutations (parse once, save once), created via `Pdf.edit(bytes)`
- `PdfBuilder` — create PDFs from scratch with text, images, form fields, created via `Pdf.build()`
- `PdfPageBuilder.textField`, `.checkbox`, `.comboBox(options)`, `.pushButton(caption)`, `.signatureField` — interactive form creation
- `PdfPageBuilder.radioGroup` — radio button groups
- `PdfPageBuilder.fieldKeystroke`, `.fieldFormat`, `.fieldValidate`, `.fieldCalculate` — JavaScript actions on form fields
