# pdf_manipulator

[![pub package](https://img.shields.io/pub/v/pdf_manipulator.svg)](https://pub.dev/packages/pdf_manipulator)
[![likes](https://img.shields.io/pub/likes/pdf_manipulator)](https://pub.dev/packages/pdf_manipulator/score)
[![pub points](https://img.shields.io/pub/points/pdf_manipulator)](https://pub.dev/packages/pdf_manipulator/score)
[![GitHub stars](https://img.shields.io/github/stars/whuppi/pdf_manipulator?style=flat&logo=github)](https://github.com/whuppi/pdf_manipulator)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Cross-platform PDF manipulation for Dart & Flutter. Merge, split, render, extract, search, sign, encrypt, validate, convert, build from scratch. Native and web. Every operation runs off the main thread, streams at constant memory, can be cancelled mid-flight, and dies instantly on dispose.

> Try it — and if it helps, a [⭐ star](https://github.com/whuppi/pdf_manipulator) or [👍 like](https://pub.dev/packages/pdf_manipulator) keeps it going. [Bugs & features →](https://github.com/whuppi/pdf_manipulator/issues)

> **Coming from the old Android-only package?** See the [migration guide](docs/MIGRATION.md).

---

## Contents

- [Install](#install)
  - [Add the dependency](#add-the-dependency)
  - [Native targets](#native-platforms)
  - [Web](#web)
- [Quick start](#quick-start)
  - [Sources & sinks](#sources--sinks)
- [Usage](#usage)
  - [Combine & split](#combine--split)
  - [Read & query](#read--query)
  - [Edit & transform](#edit--transform)
  - [Security & signing](#security--signing)
  - [Convert](#convert)
  - [Create from scratch](#create-from-scratch)
  - [Batch editing](#batch-editing)
- [Error handling](#error-handling)
- [Targets](#targets)
  - [How binaries are resolved](#how-binaries-are-resolved)
  - [Browser support](#browser-support)
  - [Web I/O modes](#web-io-modes)
- [When NOT to use pdf_manipulator](#when-not-to-use-pdf_manipulator)
- [Docs](#docs)

---

## Install

### Add the dependency

```yaml
dependencies:
  pdf_manipulator:
```

### Native targets

iOS, Android, macOS, Windows, Linux — nothing extra. The build hook downloads the correct binary automatically.

### Web

Run setup once after install, and again after each `pub upgrade`:

```sh
flutter pub run pdf_manipulator:setup
```

Hard-pin the version to avoid silent upgrades breaking your web build:

```yaml
pdf_manipulator: X.Y.Z  # exact version — upgrade intentionally
```

<details>
<summary>All setup commands</summary>

```sh
flutter pub run pdf_manipulator:setup                  # web (default)
flutter pub run pdf_manipulator:setup <target>         # web|android|ios|macos|linux|windows
flutter pub run pdf_manipulator:setup --force <target> # re-resolve (debugging)
```

</details>

<details>
<summary>Why does web need a setup step?</summary>

Flutter's build system automatically downloads native binaries for
iOS, Android, etc. — but it doesn't support web assets (WASM, JS)
yet. The setup command fills that gap: it downloads the pre-built
WASM engine, or compiles it from the vendored Rust source if the
download isn't available.

This will go away when Dart/Flutter adds WASM/JS asset support to
build hooks. Tracking: [dart-lang/native#988](https://github.com/dart-lang/native/issues/988)

</details>

---

## Quick start

```dart
import 'package:pdf_manipulator/pdf_manipulator.dart';

final pdf = Pdf();

// Operations run in parallel across isolated lanes, off the main
// thread. Cap concurrent lanes per instance if you want — defaults
// to half the cores, minimum 2:
//   final pdf = Pdf(config: PdfConfig(maxLanes: 8));

// Open a PDF from bytes in memory
final source = MemorySource(pdfBytes);      // your Uint8List
final doc = await pdf.open(source);
print('${doc.pageCount} pages');
await doc.dispose();

// Merge two PDFs into one
final output = MemorySink();
await pdf.merge([sourceA, sourceB], output);
final mergedBytes = output.takeBytes();

// Always dispose when done
await pdf.dispose();
```

That's it. Every operation follows the same pattern: **source in, sink out**.

### Cancellation

Every engine method returns a `PdfTask<T>` — a `Future` you can also
cancel. Existing `await` code works unchanged; cancellation is one
extra verb when you want it:

```dart
final task = pdf.merge(sources, output);   // starts immediately

// User navigated away — abort just this operation.
task.cancel();                              // idempotent, instant

try {
  await task;
} on PdfCancelled {
  // The op was cancelled; the Pdf instance and every other
  // handle keep working.
}
```

`pdf.dispose()` is the bigger hammer: it cancels everything on the
instance and returns in the same event-loop turn — no waiting for
in-flight work to drain.

### Sources & sinks

A `DataSource` is where the PDF bytes come from. A `DataSink` is where the output goes. Two tiny interfaces:

```dart
abstract interface class DataSource {
  int get length;
  FutureOr<Uint8List> readAt(int offset, int count);
}

abstract interface class DataSink {
  FutureOr<void> write(Uint8List chunk);
}
```

The simplest implementations — good for getting started:

```dart
class MemorySource implements DataSource {
  MemorySource(this._data);
  final Uint8List _data;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) =>
      Uint8List.sublistView(_data, offset, (offset + count).clamp(0, _data.length));
}

class MemorySink implements DataSink {
  final _buf = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _buf.add(chunk);

  Uint8List takeBytes() => _buf.takeBytes();
}
```

Wrap whatever you have — `Uint8List` for memory, `RandomAccessFile` for disk, `Blob.slice` for web file pickers, HTTP Range requests for remote files. The engine reads at most 64KB per call, never the whole file. Constant memory regardless of file size.

<details>
<summary>More implementations: file, HTTP, web blob</summary>

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

**Note:** `DataSource` is random-access — the engine jumps to arbitrary positions in the file. Forward-only streams (like a network socket or stdin) need to be buffered into memory or disk first.

---

## Usage

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

### Read & query

Open a PDF once, run any number of queries, dispose when done:

```dart
final doc = await pdf.open(source);
print('${doc.pageCount} pages, v${doc.version}');
print('encrypted: ${doc.isEncrypted}, tagged: ${doc.isTagged}');
print('title: ${doc.title}, author: ${doc.author}');
```

**Extract text** — plain, markdown, or html:

```dart
final text = await doc.extract(pages: PdfPages.all());
final md = await doc.extract(
    pages: PdfPages.single(0), format: PdfExtractionFormat.markdown);
final html = await doc.extract(
    pages: PdfPages.single(0), format: PdfExtractionFormat.html);
```

**Search** with bounding rectangles:

```dart
final hits = await doc.search(query: 'revenue', pages: PdfPages.all());
for (final hit in hits) {
  print('p${hit.page}: "${hit.text}" at (${hit.rect.x}, ${hit.rect.y})');
}
```

**Render** to images — streams one page at a time, constant memory:

```dart
await for (final page in doc.render(
    pages: PdfPages.all(), size: PdfRenderSize.thumbnail(200))) {
  // page.width, page.height, page.data (RGBA Uint8List)
}
```

**Extract embedded images:**

```dart
await for (final img in doc.extractImages(pages: PdfPages.single(0))) {
  print('${img.width}×${img.height} ${img.format}');
}
```

**Validate, classify, inspect:**

```dart
// PDF/A and PDF/UA compliance
final pdfA = await doc.validatePdfA();
print('PDF/A: ${pdfA.compliant} (${pdfA.errors} errors, ${pdfA.warnings} warnings)');
final accessible = await doc.validatePdfUa();

// Auto-detect page/document type
final pageType = await doc.classifyPage(0);
final docType = await doc.classifyDocument();

// Digital signatures
final sigs = await doc.getSignatures();
final valid = await doc.verifySignatures();

// Bookmark structure
final segments = await doc.planSplitByBookmarks();

await doc.dispose();
```

### Edit & transform

```dart
// Rotate
await pdf.rotatePages(source, sink, pages: {0: 90, 2: 180});
await pdf.rotateAllPages(source, sink, degrees: 90);

// Watermark — centered (default), tiled, corner, or exact position
await pdf.watermark(source, sink,
    text: 'CONFIDENTIAL',
    style: PdfWatermarkStyle(opacity: 0.2, fontSize: 60, rotation: 45));

// Tiled watermark behind content
await pdf.watermark(source, sink,
    text: 'DRAFT',
    position: PdfWatermarkPosition.tiled(columns: 3, rows: 4),
    layer: PdfWatermarkLayer.background);

// Corner watermark
await pdf.watermark(source, sink,
    text: 'SAMPLE',
    position: PdfWatermarkPosition.corner(PdfCorner.topRight));

// Stamps
await pdf.addStamp(source, sink,
    page: 0, type: PdfStampType.approved,
    rect: PdfRect(x: 100, y: 100, width: 200, height: 50));
await pdf.addImageStamp(source, sink,
    page: 0, imageData: imageSource,
    rect: PdfRect(x: 100, y: 100, width: 150, height: 150));

// Compress
await pdf.compress(source, sink, imageQuality: 75);

// Flatten forms / redactions
await pdf.flattenForms(source, sink);
await pdf.applyRedactions(source, sink);

// Embed file / erase regions
await pdf.embedFile(source, sink, name: 'data.csv', fileData: csvSource);
await pdf.eraseRegions(source, sink,
    page: 0, regions: [PdfRect(x: 50, y: 700, width: 200, height: 30)]);

// Convert to PDF/A
await pdf.convertToPdfA(source, sink);

// Images to PDF
await pdf.imagesToPdf([img1, img2, img3], sink);
```

For multiple edits on the same PDF, use the [batch editor](#batch-editing) — parse once, mutate many, save once.

### Security & signing

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

// Sign (PKCS#12)
await pdf.sign(source, sink,
    credentials: PdfSigningCredentials.pkcs12(certBytes, 'cert-pw'),
    reason: 'Approved', location: 'HQ');

// Sign (PEM)
await pdf.sign(source, sink,
    credentials: PdfSigningCredentials.pem(certPem, keyPem));
```

### Convert

```dart
// PDF → Office
await pdf.convertTo(source, sink, format: PdfDocumentFormat.docx);
await pdf.convertTo(source, sink, format: PdfDocumentFormat.pptx);
await pdf.convertTo(source, sink, format: PdfDocumentFormat.xlsx);

// Office → PDF
await pdf.convertToPdf(docxSource, sink, format: PdfDocumentFormat.docx);
```

### Create from scratch

```dart
final builder = await pdf.build();
await builder.setTitle('Invoice #1042');
await builder.setAuthor('Acme Corp');

final page = await builder.addA4Page();
await page.heading(1, 'Invoice');
await page.paragraph('Thank you for your purchase.');
await page.space(20);
await page.textField('notes', PdfRect(x: 50, y: 400, width: 300, height: 100));
await page.checkbox('agree', PdfRect(x: 50, y: 370, width: 14, height: 14));
await page.radioGroup('plan', [
  (value: 'monthly', rect: PdfRect(x: 50, y: 340, width: 14, height: 14)),
  (value: 'yearly', rect: PdfRect(x: 50, y: 320, width: 14, height: 14)),
]);

// Form-field JavaScript (Acrobat-style actions on the field above)
await page.fieldFormat('AFNumber_Format(2, 0, 0, 0, "\$", true);');

await page.linkUrl('https://example.com');
await page.linkPage(2);                 // jump to another page in this doc
await page.footnote('1', 'Terms apply.');
await page.done();

await builder.save(sink);
await builder.dispose();
```

Text, headings, paragraphs, images, form fields (text, checkbox, radio group, combo box, push button, signature), links (URL or page), footnotes, columns, watermarks — all from Dart. Page sizes: A4, Letter, or custom dimensions.

Form fields can carry JavaScript actions — `fieldKeystroke`, `fieldFormat`, `fieldValidate`, `fieldCalculate` — that conforming viewers run as the user types, on display, on commit, and when other fields change.

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

await editor.save(sink, options: PdfSaveOptions.incremental());
await editor.dispose();
```

Save options:
- `PdfSaveOptions.fullRewrite()` — default. Recompresses, garbage-collects unused objects.
- `PdfSaveOptions.fullRewrite(encryption: PdfEncryption.config(...))` — encrypt on save.
- `PdfSaveOptions.fullRewrite(encryption: PdfEncryption.remove())` — strip encryption.
- `PdfSaveOptions.incremental()` — appends changes without rewriting. Faster, larger file.

**Redaction** is a three-step lifecycle on the editor — mark regions, optionally count what's pending, then permanently remove the content:

```dart
final editor = await pdf.edit(source);
editor.addRedaction(0, PdfRect(x: 72, y: 700, width: 200, height: 20));
editor.addRedaction(1, PdfRect(x: 72, y: 680, width: 150, height: 20));
print(await editor.redactionCount(0));   // pending marks on page 0
await editor.applyRedactions();          // content is gone, not hidden
await editor.save(sink);
await editor.dispose();
```

Every operation from the sections above is also available on the editor: rotate, stamp, flatten, crop, resize images, embed files, set form field values, scrub metadata, and more.

---

## Error handling

```dart
try {
  await pdf.open(source);
} on PdfPasswordRequired {
  // needs a password — retry with pdf.open(source, password: '...')
} on PdfCorrupted catch (e) {
  print('Bad PDF: ${e.message}');
} on PdfIoError catch (e) {
  print('I/O problem: ${e.message}');
}
```

Every error is a typed subclass of `PdfError`. No string matching. No `PlatformException`.

---

## Targets

| Target | Architectures | Minimum version | Engine |
|---|---|---|---|
| Android | arm64, arm, x64, x86 | API 21 (Android 5.0) | Native (Rust) |
| iOS | arm64 device, arm64 + x64 simulator | 13.0 | Native (Rust) |
| macOS | arm64, x64 | 10.15 (Catalina) | Native (Rust) |
| Linux | x64, arm64 | glibc 2.31+ (Ubuntu 20.04+) | Native (Rust) |
| Windows | x64, arm64 | Windows 10 | Native (Rust) |
| Web | All modern browsers | See [browser support](#browser-support) | WASM |

### How binaries are resolved

The build hook (native) and setup command (web) resolve assets through the same pipeline:

| Priority | Method | When | Requires |
|:---:|---|---|---|
| 1 | **Cached** | File exists + SHA-256 hash matches | Nothing |
| 2 | **Download** | Fetch pre-built from GitHub Releases | Internet |
| 3 | **Source compile** | Binary unavailable, vendor source on disk | [Rust](https://rustup.rs) |
| 4 | **Submodule init** | Git dep `ref: dev` (no vendor dir) | [Rust](https://rustup.rs) + git |
| 5 | **Error** | Nothing worked | A clear message listing your options |

The vendored Rust source ships in both the pub.dev tarball and git tags. If the repo disappears, published versions still compile from source.

### Browser support

| Browser | Version | Released |
|---|---|---|
| Chrome / Edge | 102+ | May 2022 |
| Firefox | 111+ | Mar 2023 |
| Safari / Safari iOS | 15.2+ | Dec 2021 |
| Chrome Android | 102+ | May 2022 |
| Samsung Internet | 21+ | 2023 |

The engine compiles to WASM and runs in isolated Web Workers. Your UI thread never does PDF work.

### Web I/O modes

Three modes, auto-detected (best first). No code changes between them:

| Mode | How it works | Streaming | Requires |
|---|---|:---:|---|
| **JSPI** | WASM promise suspension | ✅ | Chrome 137+ · Firefox 139+ |
| **Atomics** | SharedArrayBuffer blocking | ✅ | COOP/COEP headers |
| **OPFS** | Pre-copy to disk, then process | ❌ | All modern browsers |

Force a mode or check which was selected:

```dart
// Force the web I/O mode (native ignores it)
final pdf = Pdf(config: PdfConfig(webIoMode: PdfIoMode.atomics));

// Check
final mode = await pdf.ensureInitialized();
if (mode == PdfIoMode.opfs) {
  // OPFS: pre-copies each source to disk before processing.
  // Slower first byte + uses disk quota vs streaming modes.
  // To get streaming: deploy with COOP/COEP headers (Atomics)
  // or target Chrome 137+ / Firefox 139+ (JSPI auto-detected).
}
```

<details>
<summary>Advanced: COOP/COEP headers for Atomics on older browsers</summary>

By default on browsers without JSPI support, the package copies your PDF to temporary disk storage (OPFS) before processing — works everywhere, no server config needed.

On Chrome 137+ and Firefox 139+, JSPI mode is auto-detected and gives true streaming without any server config. This is the best mode and requires no action from you.

For **older browsers** that have `SharedArrayBuffer` but not JSPI, adding two server headers enables Atomics mode — direct memory reads, no disk copy, lower latency:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

**⚠️ These headers have side effects.** `require-corp` blocks loading ANY cross-origin resource (images, fonts, scripts, iframes) that doesn't explicitly opt in via `Cross-Origin-Resource-Policy` or CORS headers. Google Fonts, CDN images, analytics scripts, OAuth popups, embedded videos — all break unless their servers also send the right headers. Only add these if your app controls all its resource origins or you've tested thoroughly.

With these headers, browser support for streaming goes further back:

| Browser | Version | Released |
|---|---|---|
| Chrome / Edge | 68+ | Jul 2018 |
| Firefox | 79+ | Jul 2020 |
| Safari / Safari iOS | 15.2+ | Dec 2021 |

For development:

```sh
flutter run -d chrome --cross-origin-isolation
```

This adds the COOP/COEP headers to Flutter's dev server automatically.

</details>

---

## When NOT to use pdf_manipulator

- **You only need to display PDFs.** Use [`pdfx`](https://pub.dev/packages/pdfx) or [`flutter_pdfview`](https://pub.dev/packages/flutter_pdfview).
- **Server-side batch processing.** This package is for client-side use. For thousands of PDFs per second, use qpdf or poppler.
- **OCR.** This package extracts text already in the PDF. For scanned images, you need Tesseract or similar.

---

## Docs

| Doc | What's inside |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | How it's built — layers, streaming I/O, three web modes |
| [Capabilities](docs/CAPABILITY_ROADMAP.md) | What's shipped, what's planned |
| [Updating](docs/UPDATING.md) | Maintaining the vendored Rust engine |
| [Migration](docs/MIGRATION.md) | Upgrading from the old Android-only version |
| [Contributing](CONTRIBUTING.md) | Setup, PR workflow, adding operations |

---

## License

MIT. See [LICENSE](LICENSE).
