# pdf_manipulator

Cross-platform PDF manipulation for Dart & Flutter. Merge, split, render, extract, search, sign, encrypt, validate, build from scratch. Native and web. Off the main thread.

> **Coming from the old Android-only package?** See the [migration guide](docs/MIGRATION.md).

---

## Contents

- [Install](#install)
- [Quick start](#quick-start)
- [What you can do](#what-you-can-do)
  - [Combine & split](#combine--split)
  - [Read & extract](#read--extract)
  - [Edit & transform](#edit--transform)
  - [Security](#security)
  - [Create from scratch](#create-from-scratch)
  - [Batch editing](#batch-editing)
- [Error handling](#error-handling)
- [Platforms](#platforms) — [native](#native), [web](#web)
- [When NOT to use pdf_manipulator](#when-not-to-use-pdf_manipulator)
- [Docs](#docs)

---

## Install

```yaml
dependencies:
  pdf_manipulator: ^1.0.0
```

**Web only** — run once after install (and after each package update):

```sh
dart run pdf_manipulator:setup
```

Native platforms need nothing extra — the build hook handles everything.

---

## Quick start

```dart
import 'package:pdf_manipulator/pdf_manipulator.dart';

final pdf = Pdf();

// Open and inspect
final doc = await pdf.open(source);
print('${doc.pageCount} pages, v${doc.version}');

// Merge two PDFs
await pdf.merge([sourceA, sourceB], outputSink);

// Extract text
final text = await pdf.extract(source, pages: PdfPages.all());

pdf.dispose();
```

`source` is a `DataSource` — anything that can read bytes at an offset. `outputSink` is a `DataSink` — anything that can receive chunks. Two methods each:

```dart
abstract interface class DataSource {
  int get length;
  FutureOr<Uint8List> readAt(int offset, int count);
}

abstract interface class DataSink {
  FutureOr<void> write(Uint8List chunk);
}
```

Wrap whatever you have — `Uint8List` for memory, `RandomAccessFile` for disk, `Blob.slice` for web file pickers, HTTP Range requests for servers. The engine reads a few KB at a time, never the whole file.

<details>
<summary>Example implementations (memory, file, HTTP, web blob)</summary>

```dart
// Memory — tests, small files
class MemorySource implements DataSource {
  MemorySource(this._data);

  final Uint8List _data;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) =>
      Uint8List.sublistView(
        _data, offset, (offset + count).clamp(0, _data.length));
}

class MemorySink implements DataSink {
  final _buf = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _buf.add(chunk);

  Uint8List takeBytes() => _buf.takeBytes();
}
```

```dart
// File — mobile/desktop, constant memory for any size
class FileSource implements DataSource {
  FileSource(this._file) : length = _file.lengthSync();

  final File _file;

  @override
  final int length;

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    final raf = await _file.open();
    await raf.setPosition(offset);
    final bytes = await raf.read(count);
    await raf.close();
    return bytes;
  }
}
```

```dart
// HTTP — stream from server without downloading the whole file
class HttpSource implements DataSource {
  HttpSource(this._url, this.length);

  final Uri _url;

  @override
  final int length;

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    final req = await HttpClient().getUrl(_url);
    req.headers.set('Range', 'bytes=$offset-${offset + count - 1}');
    final res = await req.close();
    final builder = BytesBuilder();
    await for (final chunk in res) {
      builder.add(chunk);
    }
    return builder.takeBytes();
  }
}
```

```dart
// Web Blob — browser file picker or drag-and-drop (package:web)
class BlobSource implements DataSource {
  BlobSource(this._blob) : length = _blob.size;

  final web.Blob _blob;

  @override
  final int length;

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    final slice = _blob.slice(offset, offset + count);
    final bytes = await slice.arrayBuffer().toDart;
    return bytes.asUint8List();
  }
}
```

</details>

---

## What you can do

### Combine & split

```dart
// Merge
await pdf.merge([sourceA, sourceB, sourceC], outputSink);

// Split every N pages
await pdf.split(source, (i) => MemorySink(), every: 5);

// Split by file size
await pdf.splitBySize(source, (i) => MemorySink(), maxBytes: 500000);

// Split at bookmark boundaries
await pdf.splitByBookmarks(source, (i) => MemorySink());

// Pick specific pages
await pdf.extractPages(source, sink, pages: [0, 2, 5]);

// Remove pages
await pdf.deletePages(source, sink, pages: [3]);

// Reorder
await pdf.reorderPages(source, sink, order: [4, 3, 2, 1, 0]);

// Move one page
await pdf.movePage(source, sink, from: 0, to: 4);
```

### Read & extract

```dart
// Inspect
final doc = await pdf.open(source);
print('${doc.pageCount} pages, v${doc.version}, encrypted: ${doc.isEncrypted}');

// Extract text (plain, markdown, or html)
final text = await pdf.extract(source, pages: PdfPages.all());
final md = await pdf.extract(source,
    pages: PdfPages.single(0), format: PdfExtractionFormat.markdown);

// Search with positions
final hits = await pdf.search(source, query: 'revenue', pages: PdfPages.all());
for (final hit in hits) {
  print('p${hit.page}: "${hit.text}" at (${hit.rect.x}, ${hit.rect.y})');
}

// Render to images — one at a time, constant memory
await for (final page in pdf.render(source,
    pages: PdfPages.all(), size: PdfRenderSize.thumbnail(200))) {
  // page.width, page.height, page.data (RGBA Uint8List)
}

// Extract embedded images
await for (final img in pdf.extractImages(source, pages: PdfPages.single(0))) {
  print('${img.width}×${img.height} ${img.format}');
}

// Validate
final pdfA = await pdf.validatePdfA(source);
print('PDF/A compliant: ${pdfA.compliant}');

// Convert
await pdf.convertTo(source, sink, format: PdfDocumentFormat.docx);
await pdf.convertToPdf(docxSource, sink, format: PdfDocumentFormat.docx);
```

### Edit & transform

```dart
// Rotate
await pdf.rotatePages(source, sink, pages: {0: 90, 2: 180});
await pdf.rotateAllPages(source, sink, degrees: 90);

// Watermark — centered (default), tiled, or in a corner
await pdf.watermark(source, sink,
    text: 'CONFIDENTIAL',
    style: PdfWatermarkStyle(opacity: 0.2, fontSize: 60, rotation: 45));

// Tiled watermark behind content (background)
await pdf.watermark(source, sink,
    text: 'DRAFT',
    position: PdfWatermarkPosition.tiled(columns: 3, rows: 4),
    layer: PdfWatermarkLayer.background);

// Stamp
await pdf.addStamp(source, sink,
    page: 0, type: PdfStampType.approved,
    rect: PdfRect(x: 100, y: 100, width: 200, height: 50));

// Compress
await pdf.compress(source, sink);

// Flatten forms
await pdf.flattenForms(source, sink);
```

For multiple edits on the same PDF, use the [batch editor](#batch-editing) — open once, mutate many times, save once.

### Security

```dart
// Encrypt
await pdf.encrypt(source, sink,
    encryption: PdfEncryptionConfig(
      ownerPassword: 'secret',
      algorithm: PdfEncryptionAlgorithm.aes256,
      permissions: PdfPermissions(copy: false, modify: false),
    ));

// Decrypt
await pdf.decrypt(source, sink, password: 'secret');

// Sign
await pdf.sign(source, sink,
    credentials: PdfSigningCredentials.pkcs12(certBytes, password: 'cert-pw'),
    reason: 'Approved', location: 'HQ');

// Inspect signatures
final sigs = await pdf.getSignatures(source);
final valid = await pdf.verifySignatures(source);
```

### Create from scratch

```dart
final builder = await pdf.build();
await builder.setTitle('Invoice #1042');

final page = await builder.addA4Page();
await page.heading(1, 'Invoice');
await page.paragraph('Thank you for your purchase.');
await page.textField('notes', PdfRect(x: 50, y: 400, width: 300, height: 100));
await page.done();

await builder.save(sink);
await builder.dispose();
```

Text, headings, paragraphs, images, form fields (text, checkbox, combo box, radio group, push button, signature), links, footnotes, columns, barcodes, watermarks — all from Dart.

### Batch editing

When you need to do multiple things to the same PDF, open an editor. It parses the PDF once, applies all your mutations in memory, and writes once on save.

```dart
final editor = await pdf.edit(source);

await editor.setTitle('Q4 Report');
await editor.mergeFrom(appendixSource);
await editor.deletePage(4);
await editor.selectPages([0, 1, 2, 5, 6]);
await editor.addWatermark(0, 'FINAL', style: PdfWatermarkStyle(opacity: 0.15));
await editor.optimizeImages(quality: 70);
await editor.convertToPdfA();

await editor.save(sink,
    options: PdfSaveOptions(mode: PdfSaveMode.incremental)); // faster
await editor.dispose();
```

Every operation from the sections above is also available on the editor: rotate, watermark, stamp, encrypt, flatten, redact, crop, resize, embed files, set metadata, and more.

---

## Error handling

```dart
try {
  await pdf.open(source);
} on PdfPasswordRequired {
  // needs a password
} on PdfCorrupted catch (e) {
  print('Bad PDF: ${e.message}');
} on PdfIoError catch (e) {
  print('I/O problem: ${e.message}');
}
```

Every error is a typed subclass of `PdfError`. No string matching. No `PlatformException`.

---

## Platforms

### Native

| Platform | Architectures |
|---|---|
| macOS | arm64, x64 |
| iOS | arm64, simulator (arm64, x64) |
| Android | arm64, arm, x86_64, x86 |
| Linux | x64, arm64 |
| Windows | x64 |

The PDF engine is compiled Rust. For consumers (installed via pub.dev), the build hook downloads a pre-built binary from GitHub Releases automatically — no Rust toolchain needed. For contributors (cloned with `--recursive`), it compiles from the vendored source.

### Web

Works out of the box on all modern browsers:

| Browser | Version | Released |
|---|---|---|
| Chrome / Edge | 102+ | May 2022 |
| Firefox | 111+ | Mar 2023 |
| Safari / Safari iOS | 15.2+ | Dec 2021 |
| Chrome Android | 102+ | May 2022 |
| Samsung Internet | 21+ | 2023 |

Run once after install (and after each package update):

```sh
dart run pdf_manipulator:setup
```

The engine compiles to WASM and runs in a Web Worker pool. Your UI thread never does PDF work.

<details>
<summary>Advanced: faster web with COOP/COEP headers (has trade-offs)</summary>

By default, the package copies your PDF to temporary disk storage (OPFS) before processing — works everywhere, no server config needed.

Adding two server headers enables a faster mode using `SharedArrayBuffer` — direct memory reads, no disk copy, lower latency:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

**⚠️ These headers have side effects.** `require-corp` blocks loading ANY cross-origin resource (images, fonts, scripts, iframes) that doesn't explicitly opt in via `Cross-Origin-Resource-Policy` or CORS headers. Google Fonts, CDN images, analytics scripts, OAuth popups, embedded videos — all break unless their servers also send the right headers. Only add these if your app controls all its resource origins or you've tested thoroughly.

With these headers, browser support goes further back:

| Browser | Version | Released |
|---|---|---|
| Chrome / Edge | 68+ | Jul 2018 |
| Firefox | 79+ | Jul 2020 |
| Safari / Safari iOS | 15.2+ | Dec 2021 |

For development only:

```sh
flutter run -d chrome \
  --web-header=Cross-Origin-Opener-Policy=same-origin \
  --web-header=Cross-Origin-Embedder-Policy=require-corp
```

The package detects which mode is available and picks the best one automatically. No code changes either way — it's purely a server configuration choice.

</details>

---

## When NOT to use pdf_manipulator

- **You only need to display PDFs, not manipulate them.** Use [`pdfx`](https://pub.dev/packages/pdfx) or [`flutter_pdfview`](https://pub.dev/packages/flutter_pdfview) — they're built for viewing.
- **You're building a server that processes thousands of PDFs per second.** This package is optimized for client-side use (one PDF at a time, off the UI thread). For server-side batch processing, use a dedicated server tool like qpdf or poppler.
- **You need OCR.** This package extracts text that's already in the PDF. If the PDF is a scanned image with no text layer, you need an OCR engine like Tesseract — that's a different problem.

---

## Docs

| | |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | How it's built — layers, symmetry, streaming I/O |
| [Capabilities](docs/CAPABILITY_ROADMAP.md) | What's shipped, what's next |
| [Updating](docs/UPDATING.md) | Maintaining the vendored Rust engine |
| [Migration](docs/MIGRATION.md) | Upgrading from the old Android-only version |
| [Contributing](CONTRIBUTING.md) | Setup, PR workflow, adding operations |

---

## License

MIT. See [LICENSE](LICENSE).
