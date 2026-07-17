<!--
  Banner stays <picture> for GitHub's dark/light. pub.dev strips <picture>
  when sanitizing the README, so the publish step flattens it to the inner
  <img> via `tool/ci/release.sh --stamp-readme` (the repo copy is untouched).
  Drop both once pub.dev renders <picture>. Tracking:
  dart-lang/pub-dev#5923, dart-lang/pub-dev#6363, google/dart-neats#383.
-->
<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)"  srcset="assets/banner_dark-web-min.webp">
    <source media="(prefers-color-scheme: light)" srcset="assets/banner_light-web-min.webp">
    <img alt="pdf_manipulator — cross-platform PDF manipulation for Dart & Flutter"
         src="assets/banner_light-web-min.webp" width="100%">
  </picture>
</p>

<p align="center">
  <a href="https://pub.dev/packages/pdf_manipulator"><img src="https://img.shields.io/pub/v/pdf_manipulator.svg" alt="pub package"></a>
  <a href="https://pub.dev/packages/pdf_manipulator/score"><img src="https://img.shields.io/pub/likes/pdf_manipulator" alt="likes"></a>
  <a href="https://pub.dev/packages/pdf_manipulator/score"><img src="https://img.shields.io/pub/points/pdf_manipulator" alt="pub points"></a>
  <a href="https://github.com/whuppi/pdf_manipulator"><img src="https://img.shields.io/github/stars/whuppi/pdf_manipulator?style=flat&logo=github" alt="GitHub stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="license: MIT"></a>
</p>

Cross-platform PDF manipulation for Dart & Flutter. Merge, split, render, extract, search, sign, encrypt, validate, convert, or build from scratch. Every operation runs off the main thread and streams large files in chunks. Any of it can be cancelled mid-flight.

> like it? a [⭐ star](https://github.com/whuppi/pdf_manipulator) or [👍 like](https://pub.dev/packages/pdf_manipulator) is the entire marketing budget. [Bugs & features →](https://github.com/whuppi/pdf_manipulator/issues)

> **Coming from the old Android-only package?** everything changed, but the [migration guide](docs/MIGRATION.md) has the before/after for every call.

---

<details>
<summary><b>👀 Peek inside</b></summary>

- [Install](#install)
  - [Add the dependency](#add-the-dependency)
  - [Native](#native)
  - [Web](#web)
- [Quick start](#quick-start)
  - [Merge two PDFs](#merge-two-pdfs)
  - [Cancellation](#cancellation)
- [Sources & sinks](#sources--sinks)
- [Usage](#usage)
  - [One-shot operations](#one-shot-operations)
  - [Read a document](#read-a-document)
  - [Edit a document](#edit-a-document)
  - [Build from scratch](#build-from-scratch)
  - [CJK & emoji in form fields](#cjk--emoji-in-form-fields)
- [Error handling](#error-handling)
- [The engine binary](#the-engine-binary)
- [Platform support](#platform-support)
  - [Browser support](#browser-support)
  - [Web I/O modes](#web-io-modes)
- [Not in the box](#not-in-the-box)
- [Docs](#docs)

</details>

---

## Install

### Add the dependency

```yaml
dependencies:
  pdf_manipulator: ^2.2.0-dev.0
```

### Native

Nothing to do. On iOS, Android, macOS, Windows, and Linux, the build hook downloads the right binary on first build.

### Web

Web can't auto-download native assets, so run setup once. It fetches the prebuilt WASM engine. Run it again after any `pub upgrade`, since the asset is tied to the package version:

```sh
flutter pub run pdf_manipulator:setup
```

Pin the version, too, so a `pub upgrade` can't bump it behind your back and leave that fetched asset stale:

```yaml
pdf_manipulator: 2.2.0-dev.0  # exact version
```

<details>
<summary><b>🧰 all the setup commands</b></summary>

```sh
flutter pub run pdf_manipulator:setup                  # web (default)
flutter pub run pdf_manipulator:setup <target>         # web|android|ios|macos|linux|windows
flutter pub run pdf_manipulator:setup --force <target> # re-resolve (debugging)
flutter pub run pdf_manipulator:setup --trim           # trimmed engine (see The engine binary)
```

</details>

<details>
<summary><b>🧩 wait — why does web need a setup step?</b></summary>

<br>

Flutter's build system automatically downloads native binaries for
iOS, Android, etc., but it doesn't support web assets (WASM, JS)
yet. The setup command fills that gap: it downloads the pre-built
WASM engine, or compiles it from the vendored Rust source if the
download isn't available.

This will go away when Dart/Flutter adds WASM/JS asset support to
build hooks. Tracking: [dart-lang/native#988](https://github.com/dart-lang/native/issues/988)

</details>

That's it — every platform now runs the full engine. **Before you ship: it can be up to 70% smaller.** One pubspec entry keeps only the features your app uses — [The engine binary](#the-engine-binary).

---

## Quick start

### Merge two PDFs

Merge two PDFs, using bytes in memory (works on web too):

```dart
import 'package:pdf_manipulator/pdf_manipulator.dart';

// one Pdf instance, reused everywhere
final pdf = Pdf();

// your two PDFs, as sources
final firstPdf = MemorySource(firstPdfBytes);
final secondPdf = MemorySource(secondPdfBytes);

// where the merged PDF lands
final output = MemorySink();

// combine the two into the output
await pdf.merge([firstPdf, secondPdf], output);

// take the merged bytes, then dispose it
final merged = output.takeBytes();
await pdf.dispose();
```

On mobile or desktop, point the same program at files; only the sources, sink, and import change:

```dart
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/io.dart'; // adds FileSource / FileSink

final pdf = Pdf();

// read the two PDFs straight from disk
final firstPdf = FileSource(File('first.pdf'));
final secondPdf = FileSource(File('second.pdf'));

// write the result straight to a file
final output = await FileSink.create(File('merged.pdf'));

// the merge call is identical
await pdf.merge([firstPdf, secondPdf], output);

// flush the file, then dispose it
await output.close();
await pdf.dispose();
```

That's the shape of every **edit**: source in, sink out. Watermark, compress, split, sign are the same shape, a different verb. Reading is the other shape: `open` a document and query it, no sink needed. See [Usage](#usage) for the full menu of both.

### Cancellation

Long job the user no longer needs? You can stop it. Every operation is a **`PdfTask`**: an ordinary `Future` you `await`, plus a `cancel()` button.

Keep the task in a variable: `await` it as usual, and `cancel()` it to stop early, from a Cancel button or your widget's `dispose()`.

```dart
// run it and await it — your normal code
final task = pdf.merge([firstPdf, secondPdf], output); // starts running
try {
  await task;
  final merged = output.takeBytes(); // success
} on PdfCancelled {
  // cancelled — the Pdf and everything else keep working
}
```

```dart
// stop it — from a Cancel button, or your widget's dispose()
task.cancel(); // the await above now throws PdfCancelled
```

Three rules and you're safe:

- **The `cancel()` call never throws** — it sends a stop request and returns. No `try/catch` needed; it's safe to call twice, and a no-op once the task is done.
- **`PdfCancelled` only appears when you `await` the task** — never a half-finished result. That's the spot to wrap in `try/catch`.
- **Never await the task? Nothing to handle** — cancel it and move on, no error, no crash.

To stop *everything*, **`pdf.dispose()`** cancels every operation and returns immediately; it never waits for in-flight work to drain.

<details>
<summary><b>🧩 Advanced: when do i want more than one <code>Pdf</code>?</b></summary>

<br>

**Most apps need just one `Pdf` instance**: create it once, reuse it everywhere.

You'd want a second one when **two parts of your app should stop independently.** Say each screen runs its own PDF work: give each screen its own `Pdf`, then `dispose()` it when the user leaves, which cancels only *that* screen's jobs and leaves the rest running. (With one shared `Pdf`, `dispose()` would stop everything, everywhere.)

Each `Pdf` instance runs its own pool of background workers, so operations go in parallel. Want more or fewer at a time?

```dart
final pdf = Pdf(config: PdfConfig(maxLanes: 8)); // default: max(2, cores ÷ 2)
```

</details>

---

## Sources & sinks

Every operation reads from a **source** and writes to a **sink**. You've already used the built-ins: `MemorySource`/`MemorySink` for bytes, `FileSource`/`FileSink` for files. Write your own for anything else (a server, a database, a `Blob`).

<details>
<summary><b>🧩 what are sources & sinks, really? (+ rolling your own)</b></summary>

<br>

A **source** is how `pdf_manipulator` reads your PDF, never all at once, just small bites:

```dart
abstract interface class DataSource {
  int get length; // how big are you?
  FutureOr<Uint8List> readAt(int offset, int count); // count bytes from offset
}
```

It only nibbles — *"give me 64KB starting here"*, then the next bit, then the next. So you hand it a **reader**, not your whole file dumped into one giant `Uint8List`. (It hops around to any spot in the file, so a one-way stream like a live socket can't be a source; stash those in memory or a file first.)

A **sink** is the mirror, one method:

```dart
abstract interface class DataSink {
  FutureOr<void> write(Uint8List chunk); // here's a chunk of output
}
```

**Build your own** for any backing store. Here's a PDF on a server, streamed over HTTP, pulling only the byte ranges asked for, never the whole download:

```dart
import 'package:http/http.dart' as http;

class UrlSource implements DataSource {
  UrlSource(this.uri, this.length); // length from a HEAD request
  final Uri uri;
  @override
  final int length;

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    final res = await http.get(uri,
        headers: {'range': 'bytes=$offset-${offset + count - 1}'});
    return res.bodyBytes;
  }
}

// then use it exactly like any other source
final doc = await pdf.open(UrlSource(uri, contentLength));
```

Or a browser `Blob` from a file picker / drag-and-drop:

```dart
import 'package:web/web.dart' as web;

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

Same idea for a custom `DataSink`: implement `write(chunk)` for an upload stream, a database column, wherever the bytes should go.

</details>

---

## Usage

Everything goes through one of four doors. Pick the one that fits what you're doing. Highlights below; every method and full signature lives in the [API reference](https://pub.dev/documentation/pdf_manipulator/latest/).

### One-shot operations

A single change: call it straight on `pdf`, source in, sink out:

```dart
await pdf.watermark(source, output,
    text: 'CONFIDENTIAL',
    style: PdfWatermarkStyle(opacity: 0.2, fontSize: 60));

await pdf.compress(source, output, imageQuality: 75);

await pdf.encrypt(source, output,
    encryption: PdfEncryptionConfig(
      ownerPassword: 'secret',
      algorithm: PdfEncryptionAlgorithm.aes256,
    ));

await pdf.split(source, (i) => MemorySink(), every: 5);
```

The full one-shot set, all the same shape: `merge`, `split` / `splitBySize` / `splitByBookmarks`, `extractPages`, `deletePages`, `reorderPages`, `movePage`, `rotatePages` / `rotateAllPages`, `addStamp` / `addImageStamp`, `flattenForms`, `applyRedactions`, `embedFile`, `eraseRegions`, `decrypt`, `sign`, `convertTo` (DOCX/PPTX/XLSX), `convertToPdf`, `convertToPdfA`, `imagesToPdf`.

> Doing several of these to the *same* PDF? Use the editor (below); it parses once instead of re-parsing per call.

### Read a document

`pdf.open` gives you a document to query as much as you like, then dispose:

```dart
final doc = await pdf.open(source);
print('${doc.pageCount} pages · encrypted: ${doc.isEncrypted}');

final text = await doc.extract(pages: PdfPages.all());
final hits = await doc.search(query: 'revenue', pages: PdfPages.all());

await for (final page in doc.render(
    pages: PdfPages.all(), size: PdfRenderSize.thumbnail(200))) {
  // page.width, page.height, page.data — PNG-encoded bytes; decode to read pixels
}

await doc.dispose();
```

Also on the document: `extract` (plain / markdown / html), `extractImages`, `getSignatures` / `verifySignatures`, `validatePdfA` / `validatePdfUa`, `classifyPage` / `classifyDocument`, `planSplitByBookmarks`, plus metadata getters (`title`, `author`, `version`, `isTagged`).

### Edit a document

`pdf.edit` is for *many* changes to one PDF. It parses once, applies everything in memory, and writes once on save:

```dart
final editor = await pdf.edit(source);

await editor.setTitle('Q4 Report');
await editor.mergeFrom(appendix);
await editor.deletePage(4);
await editor.addWatermark(0, 'FINAL', style: PdfWatermarkStyle(opacity: 0.15));
await editor.optimizeImages(quality: 70);

await editor.save(output); // see save options below
await editor.dispose();
```

Also on the editor: `selectPages`, `rotatePage` / `rotateAllPages`, `addStamp` / `addImageStamp`, `embedFile`, `eraseRegions`, `cropMargins`, `resizeImage`, `flattenForms` / `flattenAllAnnotations`, `setFormFieldValue`, `unembedStandardFonts`, `convertToPdfA`, `scrubMetadata`, and metadata get/set.

Save options:

- `PdfSaveOptions.fullRewrite()` — default; recompresses and drops unused objects.
- `PdfSaveOptions.fullRewrite(encryption: ...)` — encrypt on save (or `PdfEncryption.remove()` to strip it).
- `PdfSaveOptions.incremental()` — appends changes; faster, larger file.

Redaction is a mark-then-apply lifecycle (the content is removed, not just hidden):

```dart
editor.addRedaction(0, PdfRect(x: 72, y: 700, width: 200, height: 20));
print(await editor.redactionCount(0)); // pending marks on this page
await editor.applyRedactions(); // gone for good
```

### Build from scratch

`pdf.build` hands you an empty PDF; add pages, then content:

```dart
final builder = await pdf.build();
await builder.setTitle('Invoice #1042');

final page = await builder.addA4Page();
await page.heading(1, 'Invoice');
await page.paragraph('Thank you for your purchase.');
await page.textField('notes', PdfRect(x: 50, y: 400, width: 300, height: 100));
await page.linkUrl('https://example.com');

await builder.save(output);
await builder.dispose();
```

Pages: `addA4Page` / `addLetterPage` / `addPage` (custom size). Content: `text`, `heading`, `paragraph`, `space`, `image`, `columns`, `footnote`, `watermark`; form fields (`textField`, `checkbox`, `radioGroup`, `comboBox`, `pushButton`, `signatureField`) with Acrobat JS actions (`fieldFormat`, `fieldValidate`, `fieldCalculate`, `fieldKeystroke`); links (`linkUrl`, `linkPage`).

---

### CJK & emoji in form fields

Filling a form with text the field's own font cannot draw (Japanese, Korean, Chinese, emoji) needs a fallback font. The binary doesn't bundle one (that's multiple MB most apps never use) — you register your own once, and every later fill uses it.

**1. Download a font.** Any complete `.ttf` or `.otf` file works:

- For Chinese, Japanese, or Korean: pick the Noto Sans font for your language — [Noto Sans SC](https://fonts.google.com/noto/specimen/Noto+Sans+SC) (Simplified Chinese), [Noto Sans TC](https://fonts.google.com/noto/specimen/Noto+Sans+TC) (Traditional Chinese), [Noto Sans JP](https://fonts.google.com/noto/specimen/Noto+Sans+JP) (Japanese), or [Noto Sans KR](https://fonts.google.com/noto/specimen/Noto+Sans+KR) (Korean). If your forms mix several of these languages, [notofonts/noto-cjk](https://github.com/notofonts/noto-cjk) has combined files that cover all of them at once — a bigger file, but one registration.
- For emoji: [Noto Emoji](https://fonts.google.com/noto/specimen/Noto+Emoji). Use this black-and-white font: baked emoji come out as black-and-white shapes, like printed text. Color is not possible here — PDF text is drawn from character shapes in one color, and no PDF tool can bake a color emoji font into text. (The stored value keeps the real emoji character either way.)

**2. Put the file in your app's assets** and declare it in `pubspec.yaml`:

```yaml
flutter:
  assets:
    - assets/fonts/NotoSansSC-Regular.ttf
```

**3. Register it once at startup**, before the first fill or flatten:

```dart
final fontBytes = await rootBundle.load('assets/fonts/NotoSansSC-Regular.ttf');
await pdf.registerFallbackFont(
    PdfFallbackFontKind.cjk, fontBytes.buffer.asUint8List());
```

`PdfFallbackFontKind.emoji` works the same way, with the emoji font. One registration covers the whole `Pdf` instance, on every platform.

The font stays in memory after you register it — that is what makes every later fill fast. On native the engine holds one shared copy; on web each engine worker holds its own copy (workers start only when needed). So when one language is enough, prefer the single-language file over the combined one.

Without a registered font the fill still succeeds — the value is stored correctly and readers with their own fonts display it; only the baked-in (flattened) appearance falls back to the field's font.

---

## Error handling

Every failure is a typed subclass of `PdfError`: no string matching, no `PlatformException`. Catch the cases you handle specially; let the rest fall to a catch-all. Each error carries a human-readable `message`.

```dart
try {
  final doc = await pdf.open(source);
  // ... use the document, then doc.dispose()
} on PdfPasswordRequired {
  // encrypted — retry with pdf.open(source, password: '...')
} on PdfWrongPassword {
  // wrong password — ask the user again
} on PdfCorrupted catch (e) {
  print('Not a valid PDF: ${e.message}');
} on PdfError catch (e) {
  // anything else — I/O failure, unsupported feature, page out of range...
  print(e.message);
}
```

`PdfError` is sealed, so you can also `catch (e)` once and `switch` over it, and the compiler flags any case you haven't handled. `PdfCancelled` is part of the same hierarchy (the one [Cancellation](#cancellation) throws).

---

## The engine binary

The default binary carries every capability. If your app only uses some of them, trim tells the build to keep what your code can reach and delete the rest at compile time. It follows the same safety rule as Dart's own tree shaking: when the build cannot prove that your app skips something, it keeps it. The worst case is a slightly bigger binary — never a missing feature.

What it's worth (measured):

| | Native library | Web wasm |
|---|---|---|
| **Full engine** (default) | ~21.1 MB | ~17.2 MB (~7.2 MB gzipped) |
| **Core only** (every capability trimmed; core always remains) | ~6.3 MB | ~5.2 MB (~1.9 MB gzipped) |

That's about 70% of the native library and almost three quarters of the web download gone. Real apps land between the rows — you pay only for what you keep.

One requirement: trimming compiles a custom engine on your machine, so it needs [Rust](https://rustup.rs) — the same one-line installer on macOS, Linux, and Windows. Skip it and the build tells you exactly this.

```yaml
# pubspec.yaml of YOUR app
hooks:
  user_defines:
    pdf_manipulator: ^2.2.0-dev.0
      trim: auto              # a source scan decides what to keep
```

Prefer to say it yourself? The manual form keeps exactly these capabilities (plus the always-included core — parse, write, edit, forms, build):

```yaml
      trim:
        keep: [render, signatures]
```

How do you know what to keep? Each capability covers a small set of methods. Core is always included:

| Capability | Keep it if you call | Also brings | Adds (native) |
|---|---|---|---|
| `core` | everything else — merge, split, forms, watermark, encrypt, build… | — | always included (~6.3 MB) |
| `render` | `doc.render()`, `editor.optimizeImages()`, the `compress` one-shot | — | +4.2 MB |
| `signatures` | `sign()`, `doc.getSignatures()`, `doc.verifySignatures()` | — | +0.9 MB |
| `pdfa` | `doc.validatePdfA()`, `doc.validatePdfUa()`, `convertToPdfA` | — | +0.1 MB |
| `extract` | `doc.extract()`, `doc.search()`, `doc.classifyPage()`, `doc.classifyDocument()` | — | +3.0 MB |
| `office` | `convertTo`, `convertToPdf` (DOCX / PPTX / XLSX) | `extract`, automatically | +2.5 MB on top of `extract` |

Dependencies are handled for you: `keep: [office]` switches on `extract` as well. Costs are measured one capability at a time, and capabilities share some code — so a combination can total a little less than the sum of its rows.

Not sure? Use `trim: auto` — the scan answers this for you. And if you ever guess wrong, the error message names the missing capability.

Want to see it live? [`example_trimmed/`](example_trimmed/) runs the full example app under a `keep: [render]` engine — its smoke test asserts that kept capabilities work and excluded ones answer the typed error.

On web, run the setup with the flag after configuring pubspec:

```bash
flutter pub run pdf_manipulator:setup --trim
```

<details>
<summary><b>🧩 how trim works</b></summary>

- `auto` reads your app's code and keeps every capability it can reach. If any file can't be analyzed, you get the full binary and a warning — it never guesses.
- The trimmed engine compiles once and is cached — later builds reuse it.
- Call something you trimmed away and you get a clear error saying what to add to `keep:`. No crashes, no silent misbehavior.
- A typo in `trim:` fails the build and prints the valid options.

</details>

<details>
<summary><b>🧩 where the binary comes from (the build-time steps)</b></summary>

<br>

You never call this; it runs at build time. For the curious (or when a build fails), here's the order it tries to get the binary, native and web alike:

| Priority | Method | When | Requires |
|:---:|---|---|---|
| 1 | **Cached** | File exists + SHA-256 hash matches | Nothing |
| 2 | **Download** | Fetch pre-built from GitHub Releases | Internet |
| 3 | **Source compile** | Binary unavailable, vendor source on disk | [Rust](https://rustup.rs) |
| 4 | **Submodule init** | Git dep `ref: dev` (no vendor dir) | [Rust](https://rustup.rs) + git |
| 5 | **Error** | Nothing worked | A clear message listing your options |

The vendored Rust source ships in both the pub.dev tarball and git tags. If the repo disappears, published versions still compile from source.

</details>

---

## Platform support

Every platform Flutter runs on, one API. Native platforms run a Rust core; web runs that same core compiled to WASM.

| Target | Architectures | Minimum version | Engine |
|---|---|---|---|
| Android | arm64, arm, x64, x86 | API 21 (Android 5.0) | Native (Rust) |
| iOS | arm64 device, arm64 + x64 simulator | 13.0 | Native (Rust) |
| macOS | arm64, x64 | 10.15 (Catalina) | Native (Rust) |
| Linux | x64, arm64 | glibc 2.31+ (Ubuntu 20.04+) | Native (Rust) |
| Windows | x64, arm64 | Windows 10 | Native (Rust) |
| Web | All modern browsers | See [browser support](#browser-support) | WASM |

### Browser support

Minimum versions, with no special setup:

| Browser | Version | Released |
|---|---|---|
| Chrome / Edge | 102+ | May 2022 |
| Firefox | 111+ | Mar 2023 |
| Safari / Safari iOS | 15.2+ | Dec 2021 |
| Chrome Android | 102+ | May 2022 |
| Samsung Internet | 21+ | 2023 |

The engine compiles to WASM and runs in isolated Web Workers, so your UI thread never touches PDF work. No jank, even on large files. (These floors can go further back with two extra headers; see [Web I/O modes](#web-io-modes).)

### Web I/O modes

On web you don't configure anything; the package auto-detects the best mode the browser supports and uses it. Everything works regardless of mode; the only thing that varies is how fast it reads large files.

Under the hood the modes differ in *how* the WASM engine gets your bytes: the top two stream them on demand, while the fallback copies the whole file into private browser storage first (works everywhere, just a slower first byte).

| Mode | What it means for you | Streams large files | Picked when |
|---|---|:---:|---|
| **JSPI** | Best: streams, zero setup | ✅ | Chrome 137+ · Firefox 139+ |
| **Atomics** | Streams, but needs two server headers | ✅ | COOP/COEP headers are set |
| **OPFS** | Always works; copies the file to disk first, then reads | ❌ | any modern browser (the fallback) |

<details>
<summary><b>🧩 ok but what do JSPI, Atomics, and OPFS actually mean?</b></summary>

<br>

All three solve the same puzzle (synchronous WASM code needs bytes that arrive asynchronously from Dart), just in different ways:

- **JSPI** (JavaScript Promise Integration) — the browser lets the WASM call pause and resume while it waits for the next chunk. Cleanest path, no setup; needs a recent browser. Used automatically where available.
- **Atomics** — the WASM side blocks on a `SharedArrayBuffer` while a worker fills it. Works on older browsers, but `SharedArrayBuffer` only switches on when your site sends two security headers (see below).
- **OPFS** (Origin Private File System) — when neither of the above is available, the package copies your file into the browser's private on-disk storage and reads from there. Works everywhere; the copy means a slower first byte and a little disk use.

You never choose; the package tries JSPI, then Atomics, then OPFS, and uses the first that works.

</details>

<details>
<summary><b>🧰 Advanced: force a mode, or unlock streaming on older browsers</b></summary>

<br>

Pin a mode, or just check which one was chosen (native ignores this):

```dart
final pdf = Pdf(config: PdfConfig(webIoMode: PdfIoMode.atomics)); // pin a mode
final mode = await pdf.ensureInitialized(); // or just check
```

**Unlock streaming on older browsers.** Chrome 137+ / Firefox 139+ already stream via JSPI with no setup. For older browsers that have `SharedArrayBuffer`, two server headers switch the fallback from disk-copy (OPFS) to streaming (Atomics):

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

**⚠️ These headers have side effects.** `require-corp` blocks *any* cross-origin resource (images, fonts, scripts, iframes) that doesn't opt in via `Cross-Origin-Resource-Policy` or CORS. Google Fonts, CDN images, analytics, OAuth popups, embedded video all break unless their servers send the right headers. Add them only if your app controls its resource origins, or you've tested thoroughly.

With the headers set, streaming reaches further back:

| Browser | Version | Released |
|---|---|---|
| Chrome / Edge | 68+ | Jul 2018 |
| Firefox | 79+ | Jul 2020 |
| Safari / Safari iOS | 15.2+ | Dec 2021 |

For local dev, Flutter adds the headers for you:

```sh
flutter run -d chrome --cross-origin-isolation
```

</details>

---

## Not in the box

A tiny wishlist — what the shipped package doesn't do *yet*, and what to grab in the meantime. For the full engine-vs-shipped picture, see the [capability roadmap](docs/CAPABILITY_ROADMAP.md).

- **A viewer to display PDFs.** This is a manipulation library, not a UI widget. To put a PDF on screen, use [`pdfx`](https://pub.dev/packages/pdfx) or [`flutter_pdfview`](https://pub.dev/packages/flutter_pdfview). It pairs well: pre-process here (merge, decrypt, watermark), display there. (It does render pages to image bytes via `doc.render(...)` if you'd rather draw your own surface.)
- **OCR and table extraction.** `extract` and `search` read the text already in a PDF, so a scanned page (just an image) comes back empty, and clean rows-and-columns is a separate problem. The engine *has* both (a PaddleOCR pipeline and ML table detection); the default build leaves them out so it doesn't pull the ONNX runtime and ~12 MB of models into every install. An opt-in build is on the [roadmap](docs/CAPABILITY_ROADMAP.md); until then, run them externally ([Tesseract](https://github.com/tesseract-ocr/tesseract), [Camelot](https://github.com/camelot-dev/camelot), or a cloud API) and feed the result back. Want them first-party? [Open an issue](https://github.com/whuppi/pdf_manipulator/issues); it's how we gauge demand.

---

## Docs

The README covers the everyday stuff. wanna go deeper?

| Doc | What's inside |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | How it's built: layers, streaming I/O, three web modes |
| [Capabilities](docs/CAPABILITY_ROADMAP.md) | What's shipped, what's planned |
| [Updating](docs/UPDATING.md) | Maintaining the vendored Rust engine |
| [Migration](docs/MIGRATION.md) | Upgrading from the old Android-only version |
| [Contributing](CONTRIBUTING.md) | Setup, PR workflow, adding operations |

---

## License

MIT. See [LICENSE](LICENSE).
