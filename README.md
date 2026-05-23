# pdf_manipulator

Cross-platform PDF manipulation for Dart & Flutter. Powered by [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) (Rust, MIT/Apache-2.0). Every operation runs off the main thread — Rust thread pool on native, Web Worker pool on web. Zero full-file buffers. Zero UI jank.

> **Upgrading from the old Android-only version?** See the [migration guide](docs/MIGRATION.md).

---

## Install

```yaml
dependencies:
  pdf_manipulator: ^1.0.0
```

For web targets, also run once:

```sh
dart run pdf_manipulator:setup
```

---

## The 30-second version

You implement two interfaces. The package calls them when it needs data.

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
```

```dart
final pdf = Pdf();

// Inspect
final doc = await pdf.open(MemorySource(pdfBytes));
print('${doc.pageCount} pages, version ${doc.version}');

// Merge
final sink = MemorySink();
await pdf.merge([MemorySource(pdfA), MemorySource(pdfB)], sink);

// Extract text from page 0
final text = await pdf.extract(MemorySource(pdfBytes),
    pages: PdfPages.single(0), format: PdfExtractionFormat.text);

// Render all pages as thumbnails
await for (final page in pdf.render(MemorySource(pdfBytes),
    pages: PdfPages.all(), size: PdfRenderSize.thumbnail(150))) {
  // page.width, page.height, page.data (RGBA pixels)
}

pdf.dispose();
```

`PdfSource` in, `PdfSink` out. No `dart:io`. No file paths. Same code on every platform.

---

## What you can do

### Merge

```dart
await pdf.merge([sourceA, sourceB, sourceC], outputSink);
```

### Split

```dart
// Every 5 pages
final sinks = <MemorySink>[];
await pdf.split(source, (i) { final s = MemorySink(); sinks.add(s); return s; }, every: 5);

// By file size (max 500KB each)
final count = await pdf.splitBySize(source, (i) => MemorySink(), maxBytes: 500000);
```

### Extract, delete, reorder, rotate

```dart
await pdf.extractPages(source, sink, pages: [0, 2]);
await pdf.deletePages(source, sink, pages: [3]);
await pdf.reorderPages(source, sink, order: [4, 3, 2, 1, 0]);
await pdf.rotateAllPages(source, sink, degrees: 90);
await pdf.rotatePages(source, sink, pages: {0: 180, 2: 270});
```

### Compress

```dart
await pdf.compress(source, sink, imageQuality: 75);
```

### Watermark

```dart
await pdf.watermark(source, sink,
    text: 'CONFIDENTIAL',
    pages: PdfPages.all(),
    style: PdfWatermarkStyle(opacity: 0.2, fontSize: 60));
```

### Encrypt and decrypt

```dart
await pdf.encrypt(source, sink,
    encryption: PdfEncryptionConfig(
      ownerPassword: 'secret',
      algorithm: PdfEncryptionAlgorithm.aes256,
      permissions: PdfPermissions(copy: false, modify: false),
    ));

await pdf.decrypt(source, sink, password: 'secret');
```

### Extract text / markdown

```dart
final text = await pdf.extract(source,
    pages: PdfPages.all(), format: PdfExtractionFormat.text);

final markdown = await pdf.extract(source,
    pages: PdfPages.single(0), format: PdfExtractionFormat.markdown);
```

### Search

```dart
final hits = await pdf.search(source,
    query: 'revenue', pages: PdfPages.all());
for (final hit in hits) {
  print('Page ${hit.page}: "${hit.text}" at (${hit.rect.x}, ${hit.rect.y})');
}
```

### Render pages

```dart
// Single page, native resolution
final page = await pdf.render(source, pages: PdfPages.single(0)).first;
// page.width, page.height, page.data (Uint8List of RGBA pixels)

// All pages as thumbnails
await for (final p in pdf.render(source,
    pages: PdfPages.all(), size: PdfRenderSize.thumbnail(150))) {
  saveImage(p);
}
```

### Extract images

```dart
await for (final img in pdf.extractImages(source, pages: PdfPages.single(0))) {
  print('${img.width}×${img.height} ${img.format}');
}
```

### Sign

```dart
await pdf.sign(source, sink,
    certificate: p12Bytes,
    certificatePassword: 'cert-pw',
    reason: 'Approved',
    location: 'HQ');
```

### Validate

```dart
final pdfA = await pdf.validatePdfA(source);
print('Compliant: ${pdfA.compliant}, errors: ${pdfA.errors}');
```

---

## PdfEditor — parse once, mutate many, save once

```dart
final editor = await pdf.edit(source);

await editor.setTitle('Q4 Report');
await editor.setAuthor('Finance');
await editor.mergeFrom(appendixSource);
await editor.deletePage(4);
await editor.addWatermark(0, 'FINAL',
    style: PdfWatermarkStyle(opacity: 0.15));
await editor.optimizeImages(quality: 70);

final resultSink = MemorySink();
await editor.save(resultSink, options: PdfSaveOptions(
    compress: true, garbageCollect: true));
editor.dispose();
```

---

## PdfBuilder — create from scratch

```dart
final builder = await pdf.build();
await builder.setTitle('Meeting Notes');

final page = await builder.addA4Page();
await page.heading(1, 'Q4 Planning');
await page.paragraph('We discussed the roadmap.');
await page.done();

final resultSink = MemorySink();
await builder.save(resultSink);
builder.dispose();
```

---

## Error handling

Every error is a typed subclass of `PdfError`:

```dart
try {
  await pdf.open(source);
} on PdfPasswordRequired {
  // prompt the user
} on PdfCorrupted catch (e) {
  print('Nope: ${e.message}');
}
```

---

## Platform support

| Platform | Engine | Status |
|---|---|---|
| macOS (arm64, x64) | FFI → Rust | ✓ |
| iOS (arm64, simulator) | FFI → Rust | ✓ |
| Android (arm64, arm, x86_64, x86) | FFI → Rust | ✓ |
| Linux (x64, arm64) | FFI → Rust | ✓ |
| Windows (x64) | FFI → Rust | ✓ |
| Web (Chrome, Firefox, Safari, Edge) | WASM + OPFS | ✓ |

---

## Docs

| Doc | What it covers |
|---|---|
| [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Three-layer overview, data flows, memory model |
| [`BRIDGE_ARCHITECTURE.md`](docs/BRIDGE_ARCHITECTURE.md) | Full bridge internals: thread pools, condvars, arena, OPFS, cancel, dispose |
| [`API_GOLD.md`](docs/API_GOLD.md) | Complete public API: every type, method, parameter |
| [`CAPABILITY_ROADMAP.md`](docs/CAPABILITY_ROADMAP.md) | What's shipped, what's planned |
| [`MIGRATION.md`](docs/MIGRATION.md) | v0 → v1 migration guide |
| [`UPDATING.md`](docs/UPDATING.md) | How to update the vendored pdf_oxide |
