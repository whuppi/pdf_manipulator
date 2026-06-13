# Migrating from v0 to v1

The v1 rewrite changes every API surface. This guide maps old calls to new equivalents.

---

## The big picture

| v0 | v1 |
|---|---|
| Android only | iOS, Android, macOS, Windows, Linux, Web |
| File paths in, file paths out | `DataSource` in, `DataSink` out |
| `PdfManipulator()` + params objects | `Pdf()` instance with named args |
| No cleanup needed | Call `pdf.dispose()` when done |
| `PlatformException` on error | Typed `PdfError` sealed class |
| Method channel (Kotlin ↔ Dart) | FFI (native) / WASM (web), off main thread |
| 1-based page numbers | 0-based indices |

### The I/O change

v0 worked with file paths. v1 works with `DataSource` (you read bytes) and `DataSink` (you receive bytes). No file paths anywhere.

```dart
// v0
final resultPath = await PdfManipulator().mergePDFs(
  params: PDFMergerParams(pdfsPaths: ['/path/a.pdf', '/path/b.pdf']),
);

// v1
final pdf = Pdf();
final sink = MemorySink();
await pdf.merge([FileSource(File('a.pdf')), FileSource(File('b.pdf'))], sink);
await pdf.dispose();
```

You implement `DataSource` and `DataSink` for whatever backing store you have — file, memory, HTTP, web blob. See the [README](../README.md) for examples.

---

## Method-by-method

### Merge

```dart
// v0
final path = await plugin.mergePDFs(
  params: PDFMergerParams(pdfsPaths: [pathA, pathB]),
);

// v1
final pdf = Pdf();
await pdf.merge([sourceA, sourceB], outputSink);
await pdf.dispose();
```

### Split

```dart
// v0 — by page count
final paths = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, pageCount: 2),
);

// v1
await pdf.split(source, (i) => MemorySink(), every: 2);

// v0 — by byte size
final paths = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, byteSize: 500000),
);

// v1
await pdf.splitBySize(source, (i) => MemorySink(), maxBytes: 500000);

// v0 — specific pages
final paths = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, pageNumbers: [3, 7]),
);

// v1 — 0-based
await pdf.extractPages(source, sink, pages: [2, 6]);
```

### Delete pages

```dart
// v0 — 1-based
final path = await plugin.pdfPageDeleter(
  params: PDFPageDeleterParams(pdfPath: path, pageNumbers: [2, 4]),
);

// v1 — 0-based
await pdf.deletePages(source, sink, pages: [1, 3]);
```

### Reorder pages

```dart
// v0 — 1-based
final path = await plugin.pdfPageReorder(
  params: PDFPageReorderParams(pdfPath: path, pageNumbers: [3, 1, 2]),
);

// v1 — 0-based
await pdf.reorderPages(source, sink, order: [2, 0, 1]);
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

// v1 — 0-based, map of index → degrees
await pdf.rotatePages(source, sink, pages: {0: 90, 2: 180});

// v1 — all at once
await pdf.rotateAllPages(source, sink, degrees: 90);
```

### Combined rotate + delete + reorder

```dart
// v0 — one call
final path = await plugin.pdfPageRotatorDeleterReorder(
  params: PDFPageRotatorDeleterReorderParams(
    pdfPath: path,
    pagesRotationInfo: [...],
    pageNumbersForDeleter: [2, 4],
    pageNumbersForReorder: [3, 1, 2],
  ),
);

// v1 — use PdfEditor for multiple mutations
final editor = await pdf.edit(source);
await editor.rotatePage(0, degrees: 90);
await editor.deletePage(3);
await editor.deletePage(1);
await editor.save(sink);
await editor.dispose();
```

### Compress

```dart
// v0
final path = await plugin.pdfCompressor(
  params: PDFCompressorParams(
    pdfPath: path,
    imageQuality: 70,
    imageScale: 0.5,
    unEmbedFonts: true,
  ),
);

// v1
await pdf.compress(source, sink, imageQuality: 70);
```

`imageScale` is gone — v1 optimizes without reducing resolution. `unEmbedFonts` is now a separate editor method:

```dart
final editor = await pdf.edit(source);
await editor.unembedStandardFonts();
await editor.save(sink);
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
    opacity: 0.3,
    watermarkLayer: WatermarkLayer.overContent,
    positionType: PositionType.center,
  ),
);

// v1
await pdf.watermark(source, sink,
    text: 'DRAFT',
    style: PdfWatermarkStyle(fontSize: 80, opacity: 0.3, rotation: 45),
    position: PdfWatermarkPosition.center(),  // or .corner(), .tiled(), .exact()
    layer: PdfWatermarkLayer.foreground);      // or .background for behind content
```

`positionType` is replaced by sealed `PdfWatermarkPosition`:
- `PdfWatermarkPosition.center()` — centered on page (default, same as v0's `PositionType.center`)
- `PdfWatermarkPosition.corner(PdfCorner.topRight)` — anchored to a corner with margin
- `PdfWatermarkPosition.tiled(columns: 3, rows: 4)` — repeated grid across the page
- `PdfWatermarkPosition.exact(x:, y:, width:, height:)` — caller-specified coordinates

The engine resolves named positions per-page using each page's media box — no pixel math needed for mixed-size PDFs.

`watermarkLayer` is replaced by `PdfWatermarkLayer`:
- `PdfWatermarkLayer.foreground` — annotation-based, renders on top of page content (default)
- `PdfWatermarkLayer.background` — content-stream watermark rendered behind page content

### Encrypt

```dart
// v0
final path = await plugin.pdfEncryption(
  params: PDFEncryptionParams(
    pdfPath: path,
    userPassword: 'user',
    ownerPassword: 'owner',
    allowPrinting: true,
    allowCopy: true,
    standardEncryptionAES128: true,
  ),
);

// v1
await pdf.encrypt(source, sink,
    encryption: PdfEncryptionConfig(
      ownerPassword: 'owner',
      userPassword: 'user',
      algorithm: PdfEncryptionAlgorithm.aes128,
      permissions: PdfPermissions(print: true, copy: true),
    ));
```

v1 adds four permission flags not in v0: `printHq`, `fillForms`, `accessibility`, `assemble`.

### Decrypt

```dart
// v0
final path = await plugin.pdfDecryption(
  params: PDFDecryptionParams(pdfPath: path, password: 'pw'),
);

// v1
await pdf.decrypt(source, sink, password: 'pw');
```

### Images to PDF

```dart
// v0
final paths = await plugin.imagesToPdfs(
  params: ImagesToPDFsParams(
    imagesPaths: ['/img1.jpg', '/img2.png'],
    createSinglePdf: true,
  ),
);

// v1
await pdf.imagesToPdf([imageSource1, imageSource2], sink);
```

`createSinglePdf` is gone — v1 always creates one PDF. For separate PDFs per image, call once per image.

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
final doc = await pdf.open(source);
for (final page in doc.pages) {
  print('Page ${page.index}: ${page.width} x ${page.height}');
}
```

### Cancel

```dart
// v0 — cancel everything
await plugin.cancelManipulations();

// v1 — every method returns a PdfTask: cancel just that operation
final task = pdf.merge(sources, sink);
task.cancel();                 // idempotent, instant

// v1 — or cancel everything on the instance
await pdf.dispose();
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
try {
  await pdf.merge([sourceA, sourceB], sink);
} on PdfPasswordRequired {
  // prompt user
} on PdfCorrupted catch (e) {
  print(e.message);
} on PdfError catch (e) {
  print(e);
}
```

---

## Indexing change

v0 used **1-based** page numbers. v1 uses **0-based** indices everywhere. Subtract 1 from every page number when migrating.

---

## Types removed

| v0 type | v1 replacement |
|---|---|
| `PdfManipulator` | `Pdf()` |
| `PDFMergerParams` | `pdf.merge(sources, sink)` |
| `PDFSplitterParams` | `pdf.split(...)` / `pdf.splitBySize(...)` / `pdf.extractPages(...)` |
| `PDFPageDeleterParams` | `pdf.deletePages(source, sink, pages:)` |
| `PDFPageReorderParams` | `pdf.reorderPages(source, sink, order:)` |
| `PDFPageRotatorParams` | `pdf.rotatePages(source, sink, pages:)` |
| `PageRotationInfo` | `Map<int, int>` (index → degrees) |
| `PDFPageRotatorDeleterReorderParams` | `PdfEditor` chained calls |
| `PDFCompressorParams` | `pdf.compress(source, sink)` |
| `PDFWatermarkParams` | `pdf.watermark(source, sink, text:, style:)` |
| `WatermarkLayer` | `PdfWatermarkLayer.foreground` / `.background` |
| `PositionType` | Sealed `PdfWatermarkPosition.center()` / `.corner()` / `.tiled()` / `.exact()` — engine resolves per-page |
| `PDFEncryptionParams` | `pdf.encrypt(source, sink, encryption:)` |
| `PDFDecryptionParams` | `pdf.decrypt(source, sink, password:)` |
| `ImagesToPDFsParams` | `pdf.imagesToPdf(sources, sink)` |
| `PDFPagesSizeParams` | `pdf.open(source)` → `doc.pages` |
| `PageSizeInfo` | `PdfPageInfo` |
| `PDFValidityAndProtectionParams` | `pdf.open(source)` → `doc.isEncrypted` etc. |
| `PlatformException` | `PdfError` sealed class |

---

## New in v1

No migration needed — just start using:

- Extract text, Markdown, HTML
- Search with bounding rectangles
- Render pages to images (streaming)
- Extract embedded images (streaming)
- Digital signatures (inspect, verify, sign)
- PDF/A and PDF/UA validation
- PDF/A conversion (one-shot or via editor)
- Convert to/from DOCX, PPTX, XLSX
- PdfEditor — batch mutations, incremental save, encrypted save
- PdfBuilder — create from scratch with form fields, links, columns, footnotes
- Stamp annotations (13 types + image stamps)
- Redaction, metadata scrub, crop margins
- Resource pruning (image optimization, font unembedding)
- Form field value setting
- Per-op cancellation (`PdfTask.cancel()`) and instant dispose
