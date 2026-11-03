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
- [I want to...](#i-want-to)
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
  pdf_manipulator:
```

### Native

Nothing to do. On iOS, Android, macOS, Windows, and Linux, the build hook downloads the right binary on first build.

### Web

Web can't auto-download native assets, so run setup once. It fetches the prebuilt WASM engine. Run it again after any `pub upgrade`, because the asset is tied to the package version:

```sh
flutter pub run pdf_manipulator:setup
```

Pin the version, too, so a `pub upgrade` can't bump it behind your back and leave that fetched asset stale:

```yaml
pdf_manipulator: X.Y.Z  # exact version
```

<details>
<summary><b>🧰 all the setup commands</b></summary>

```sh
flutter pub run pdf_manipulator:setup                  # web (default)
flutter pub run pdf_manipulator:setup <target>         # web|android|ios|macos|linux|windows
flutter pub run pdf_manipulator:setup --force <target> # re-resolve (debugging)
```

The command never carries config flags. `keep` and `build` live in your
pubspec (see The engine binary) — web reads the same block native does.

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

## I want to...

`doc` is an open document (`await pdf.open(source)`); `e` is the editor inside `pdf.rewrite(source, output, (e) async { ... })`.

| I want to | Do this |
|---|---|
| read a page's size, rotation and label | `await doc.pageInfo()` |
| extract the text | `await doc.extractText(sink)` |
| search the text | `await doc.search('invoice')` |
| render a page to an image | `await doc.render((page) => sink)` |
| extract the images of a page | `await doc.extractImages((image) => sink)` |
| list the signatures | `await doc.signatures()` |
| verify the signatures | `await doc.verifySignatures()` |
| validate PDF/A | `await doc.validatePdfA()` |
| validate PDF/UA | `await doc.validatePdfUa()` |
| read the bookmarks' page ranges | `await doc.bookmarkRanges()` |
| classify the pages | `await doc.classifyPages()` |
| classify the whole document | `await doc.classifyDocument()` |
| read the form fields | `await doc.formFields()` |
| export the form values | `await doc.exportFormData(sink)` |
| read the XFA packet | `await doc.xfa()` |
| list the embedded files | `await doc.attachments()` |
| list a page's annotations | `await doc.annotations(0)` |
| extract an embedded file | `await doc.extractAttachment('notes.txt', sink)` |
| watermark every page | `await pdf.rewrite(a, b, (e) => e.addWatermark('DRAFT'))` |
| watermark some pages | `await pdf.rewrite(a, b, (e) => e.addWatermark('DRAFT', pages: PdfPages.list([0, 2])))` |
| rotate pages | `await pdf.rewrite(a, b, (e) => e.rotatePages(PdfPages.single(0), by: .r90))` |
| set the rotation of pages | `await pdf.rewrite(a, b, (e) => e.setRotation(PdfPages.all(), .r0))` |
| delete pages | `await pdf.rewrite(a, b, (e) => e.deletePages(PdfPages.list([1, 3])))` |
| keep pages, in this order | `await pdf.rewrite(a, b, (e) => e.keepPages([2, 0, 1]))` |
| move a page | `await pdf.rewrite(a, b, (e) => e.movePage(from: 3, to: 0))` |
| set the title and other metadata | `await pdf.rewrite(a, b, (e) => e.setMetadata(const PdfMetadata(title: 'Q4 Report')))` |
| resize the pages | `await pdf.rewrite(a, b, (e) => e.setMediaBox(PdfPages.all(), PdfRect(x: 0, y: 0, width: 595, height: 842)))` |
| crop the pages, or remove the crop | `await pdf.rewrite(a, b, (e) => e.setCropBox(PdfPages.all(), null))` |
| trim margins off every page | `await pdf.rewrite(a, b, (e) => e.cropMargins(top: 36, bottom: 36))` |
| stamp pages | `await pdf.rewrite(a, b, (e) => e.addStamp(PdfPages.all(), type: .draft, rect: PdfRect(x: 50, y: 700, width: 120, height: 40)))` |
| stamp pages with an image | `await pdf.rewrite(a, b, (e) => e.addImageStamp(PdfPages.single(0), logo, rect: PdfRect(x: 50, y: 700, width: 80, height: 40)))` |
| flatten the form | `await pdf.rewrite(a, b, (e) => e.flattenForms())` |
| flatten the form on some pages | `await pdf.rewrite(a, b, (e) => e.flattenForms(pages: PdfPages.single(0)))` |
| flatten the annotations | `await pdf.rewrite(a, b, (e) => e.flattenAnnotations())` |
| redact a region for good | `await pdf.rewrite(a, b, (e) => e.redact(0, [PdfRect(x: 72, y: 700, width: 200, height: 20)]))` |
| erase a region, drawing no box | `await pdf.rewrite(a, b, (e) => e.redact(0, [region], overlay: .none()))` |
| embed a file | `await pdf.rewrite(a, b, (e) => e.embedFile('notes.txt', notes))` |
| convert to PDF/A | `await pdf.rewrite(a, b, (e) => e.convertToPdfA())` |
| shrink the images | `await pdf.rewrite(a, b, (e) => e.reduceImages(PdfImagePolicy.ebook))` |
| move or resize an image on a page | `await pdf.rewrite(a, b, (e) => e.setImageBounds(0, 'Im0', PdfRect(x: 0, y: 0, width: 200, height: 100)))` |
| remove embedded copies of the standard fonts | `await pdf.rewrite(a, b, (e) => e.unembedStandardFonts())` |
| remove the metadata | `await pdf.rewrite(a, b, (e) => e.scrubMetadata())` |
| remove the metadata, scripts or attachments | `await pdf.rewrite(a, b, (e) => e.sanitize(const PdfSanitizeOptions(embeddedFiles: true)))` |

Every read and every edit also has a one-shot: `pdf.reads(source)` opens the document, runs the one call and closes it, and `pdf.edits(source, output)` does the same for one edit.

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
final firstPdf = await FileSource.open(File('first.pdf'));
final secondPdf = await FileSource.open(File('second.pdf'));

// write the result straight to a file
final output = await FileSink.open(File('merged.pdf'));

// the merge call is identical
await pdf.merge([firstPdf, secondPdf], output);

// whoever opens a file closes it, then dispose the instance
await firstPdf.close();
await secondPdf.close();
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

- **The `cancel()` call never throws** — it sends a stop request and returns. No `try/catch` needed; it's safe to call twice, and a no-op after the task is done.
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

A **sink** is the mirror:

```dart
abstract interface class DataSink {
  FutureOr<void> write(Uint8List chunk); // here's a chunk of output
  FutureOr<void> abort(Object reason); // the operation ended cancelled or failed: discard it all
  FutureOr<void> close(); // release what you hold
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

Same idea for a custom `DataSink`: implement `write(chunk)` for an upload stream, a database column, wherever the bytes should go, and `abort(reason)` to take them back (cancel the upload, delete the row) when the operation is cancelled or fails, so a stopped operation leaves no partial output.

</details>

---

## Usage

Everything goes through one of four doors. Pick the one that fits what you're doing. Highlights below; every method and full signature lives in the [API reference](https://pub.dev/documentation/pdf_manipulator/latest/).

### One-shot operations

A single change: call it straight on `pdf`, source in, sink out:

```dart
await pdf.edits(source, output).addWatermark('CONFIDENTIAL',
    style: PdfWatermarkStyle(opacity: 0.2, fontSize: 60));

await pdf.compress(source, output); // images to 150 ppi (PdfImagePolicy.ebook), streams deflated, unused objects dropped
await pdf.compress(source, output, images: PdfImagePolicy.screen); // 72 ppi

await pdf.resave(source, output,
    options: PdfSaveOptions.fullRewrite(
      encryption: PdfEncryption.config(
        ownerPassword: 'secret',
        algorithm: PdfEncryptionAlgorithm.aes256,
      ),
    ));

final parts = await pdf.split(source, (part) => MemorySink(),
    by: PdfSplitRule.every(5)); // or .maxBytes(5000000), or .bookmarks()
```

`split` asks `sinkFor` for each part's sink before the part is written and closes it after, and returns what each part holds (`index`, `start` and `end` page, the bookmark's `title`). `merge` opens a password-protected source with its entry of `passwords: {source: 'secret'}`.

The one-shots that are not an edit or a read: `merge`, `split`, `compress`, `sign`, `convertTo` (DOCX/PPTX/XLSX), `convertToPdf`, `convertXfaToAcroForm`, `imagesToPdf`. Every edit is a call on `pdf.edits(source, output)` and every read a call on `pdf.reads(source)`; the table at the top of this page lists them.

> Doing several of these to the *same* PDF? Use the editor (below); it parses once instead of re-parsing per call.

### Read a document

`pdf.open` gives you a document to query as much as you like, then dispose:

```dart
final doc = await pdf.open(source);
print('${doc.pageCount} pages · encrypted: ${doc.encryption != null}');

// The text is written to a sink as the engine produces it; the sink is
// yours, so the library never closes it.
final text = MemorySink();
await doc.extractText(text);
print(text.takeString());

final hits = await doc.search('revenue');
for (final field in await doc.formFields()) {
  print('${field.name}: ${field.type.name} = ${field.value}');
}

// Each page is written to the sink the factory returns for it, row by row; the
// library closes that sink. `size` defaults to `PdfRenderSize.dpi(150)`.
final pages = await doc.render(
  (page) => FileSink.open(File('page-${page.page}.png')),
  size: PdfRenderSize.thumbnail(200),
);
// pages[i].width, pages[i].height — the pixel size of each file written

await doc.dispose();
```

The document's own facts are sync fields: `pageCount`, `version`, `isTagged`, `metadata` (title, author, subject, keywords, creator, producer, creation date) and `encryption` (the algorithm, the permissions and whether a password is needed; null when the document is not encrypted). Everything per page is a read of its own: `pageInfo` (size, rotation, label), `extractText` (plain / markdown / html), `search`, `render`, `extractImages`, `classifyPages` / `classifyDocument`, `bookmarkRanges`, `annotations(page)`, `signatures` / `verifySignatures` (digest and signer key; certificate trust is not checked), `validatePdfA` / `validatePdfUa`, `formFields`, `exportFormData` (FDF / XFDF), `xfa`, `attachments` / `extractAttachment`.

### Edit a document

`pdf.edit` is for *many* changes to one PDF. It parses once, applies everything in memory, and writes once on save:

```dart
final editor = await pdf.edit(source);

await editor.setMetadata(const PdfMetadata(title: 'Q4 Report'));
await editor.mergeFrom(appendix);
await editor.deletePages(PdfPages.single(4));
await editor.addWatermark(
  'FINAL',
  pages: PdfPages.single(0),
  style: PdfWatermarkStyle(opacity: 0.15),
);
final report = await editor.reduceImages(PdfImagePolicy.ebook); // 150 ppi, JPEG q75
print('${report.changed} images, ${report.bytesBefore} → ${report.bytesAfter} bytes');

await editor.save(output); // see save options below
await editor.dispose();
```

Also on the editor: `mergeFrom` (all pages, or `pages:`; `password:` opens an encrypted source), `keepPages`, `rotatePages` / `setRotation` (absolute), `deletePages`, `addStamp` / `addImageStamp` (over a `PdfPages` set), `embedFile` (with description, mime type and relationship), `cropMargins`, `pageImages` / `setImageBounds`, `pageInfo` (media box and crop box) / `setMediaBox` / `setCropBox`, `flattenForms` / `flattenAnnotations` (each takes `pages:`, every page by default), `unembedStandardFonts`, `convertToPdfA`, `scrubMetadata` / `sanitize`, `metadata` / `setMetadata`, and the form fields below.

Every edit that takes pages checks all of them before it changes one: a page the document does not have, a page listed twice or an empty list throws `PdfInvalidArgument` and the document stays as it was.

A form field is edited through `editor.field(name)`:

```dart
final f = editor.field('country');
await f.setValue(const PdfFormFieldValue.choice('DE')); // a combo box or list box
await editor.field('tags').setValue(const PdfFormFieldValue.choices(['a', 'c'])); // a multi-select list box
await editor.field('name').setValue(const PdfFormFieldValue.text('Ada'));
await editor.field('agree').setValue(const PdfFormFieldValue.checked(true));
await editor.field('name').setValue(const PdfFormFieldValue.none()); // clear it
```

The other edits of a field are `setReadOnly`, `setRequired`, `setFlags`, `setTooltip`, `setBounds`, `setMaxLength`, `setAlignment`, `setBackgroundColor`, `setBorderColor`, `setBorderWidth`, `setAppearance` and `remove`. `setTooltip`, `setMaxLength`, `setBackgroundColor` and `setBorderColor` take null to remove what they set. A choice that is not an option of the field (an editable combo box takes any text) throws `PdfInvalidArgument` and changes nothing.

Save options:

- `PdfSaveOptions.fullRewrite()` — default; recompresses and drops unused objects.
- `PdfSaveOptions.fullRewrite(encryption: ...)` — encrypt on save (or `PdfEncryption.remove()` to strip it).
- `PdfSaveOptions.incremental()` — keeps the original bytes and appends the changes, so an existing signature stays valid. An edit an append cannot carry (a removed page, a redaction, a content edit) throws `PdfIncrementalRefused` and writes nothing; save it with `fullRewrite()`.

Redaction removes the content, not just hides it. `redact` marks and applies the regions you give it in one step; `markRedaction` queues a mark and `applyRedactions` applies the queued ones:

```dart
final once = await editor.redact(0, [PdfRect(x: 72, y: 700, width: 200, height: 20)]);

await editor.markRedaction(0, PdfRect(x: 72, y: 650, width: 200, height: 20));
print(await editor.redactionCount(0)); // pending marks on this page
final report = await editor.applyRedactions(); // gone for good
print('${report.glyphsRemoved} glyphs and ${report.imagesRemoved} images removed');
await editor.clearRedactions(); // or drop every queued mark instead
```

`redact` applies only its own regions: marks queued earlier stay queued, and a later `applyRedactions` keeps what `redact` removed. `overlay:` is what is drawn over a region, a black box by default: `PdfRedactionOverlay.fill(PdfColor.red)` for another colour, `PdfRedactionOverlay.none()` for no box at all. An image under a region keeps a black patch where its pixels were removed whatever the overlay.

### Build from scratch

`pdf.build` hands you an empty PDF; add pages, then content:

```dart
final builder = await pdf.build();
await builder.setMetadata(const PdfMetadata(title: 'Invoice #1042'));

final page = await builder.addPage();
await page.heading(1, 'Invoice');
await page.paragraph('Thank you for your purchase.');
await page.text('Paid on time.', style: const PdfTextStyle(bold: true));
await page.textField('notes', PdfRect(x: 50, y: 400, width: 300, height: 100));
await page.linkUrl('https://example.com');

await builder.save(output);
await builder.dispose();
```

Pages: `addPage()` (A4), `addPage(PdfPageSize.letter())`, `addPage(PdfPageSize.custom(400, 300))`, and `addImagePage` for a page that holds one picture — it takes the picture's own size, one point per pixel, unless you name one. Photos become a PDF with `pdf.imagesToPdf([a, b], output)`.

Content: `text` (with a `PdfTextStyle` for the font, size, weight, slant and colour), `heading`, `paragraph`, `space`, `image`, `rect` (filled and/or stroked), `line`, `markup` (highlight, underline, strikeout, squiggly), `table`, `stickyNote`, `columns`, `footnote`, `watermark`; form fields (`textField`, `checkbox`, `radioGroup`, `comboBox`, `pushButton`, `signatureField`) with Acrobat JS actions (`fieldFormat`, `fieldValidate`, `fieldCalculate`, `fieldKeystroke`); links (`linkUrl`, `linkPage`).

```dart
await page.rect(
  const PdfRect(x: 50, y: 300, width: 200, height: 80),
  fill: PdfColor.red,
  stroke: const PdfStroke(color: PdfColor.black, width: 2),
);
await page.line((50, 300), (250, 380), stroke: const PdfStroke(dash: [3, 2]));
await page.markup(PdfMarkupKind.highlight);
await page.table(
  const [
    PdfTableColumn('SKU', width: 100),
    PdfTableColumn('Qty', width: 60, align: PdfAlignment.right),
  ],
  const [
    [PdfTableCell('A-1'), PdfTableCell('12')],
  ],
);
```

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
    PdfFallbackFontKind.cjk, MemorySource(fontBytes.buffer.asUint8List()));
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

The default binary carries every capability. If your app only uses some of them, tell the build which capabilities to `keep` and it deletes the rest at compile time. It follows the same safety rule as Dart's own tree shaking: when the build cannot prove that your app skips something, it keeps it. The worst case is a slightly bigger binary — never a missing feature.

Two knobs decide the size — **`keep`** (which capabilities go in) and **`build`** (`speed`, the default, or `size` for denser code, no features lost). Each row is one engine you can build; the columns are the two platforms:

| Engine | Native | Web — raw (gzipped) |
|---|---|---|
| Full · `speed` (default) | 22.7 MB | 18.7 MB (7.7 MB) |
| Full · `size` | 16.7 MB | 15.1 MB (6.6 MB) |
| Core only · `speed` | 6.7 MB | 5.6 MB (2.1 MB) |
| Core only · `size` | 5.4 MB | 4.7 MB (1.8 MB) |

The top row is the default; the bottom row is the smallest — core + `size`, about three quarters off. `keep` does the heavy lifting (full → core is about 71% of the native library gone); `build: size` shaves another ~20–25% on top. `size` trades a little runtime speed for the bytes: the cost lands on rendering and image work, and is negligible for parse, edit, and metadata.

A custom engine — only the capabilities you keep — compiles on your machine. There is nothing to set up in advance: if a piece is missing, the build stops with the exact instruction. What it will ask for:

- **Native** (iOS, Android, macOS, Windows, Linux): [Rust](https://rustup.rs) — the same one-line installer on every OS.
- **Web**: [Rust](https://rustup.rs) too — nothing more. The web build makes its own helper tools the first time, which adds a few minutes once; every run after that reuses them.

```yaml
# pubspec.yaml of YOUR app
hooks:
  user_defines:
    pdf_manipulator:
      keep: auto              # a source scan decides what to keep
```

The scan reads your app's own `lib/` and `bin/`. Keeping source somewhere else? Point it there — paths are relative to your app, and they add to `lib/` and `bin/` rather than replacing them:

```yaml
      keep: auto
      scan-dirs: [tools, packages/shared/lib]
```

The one thing no scan can see is a call made *inside* a package you depend on. If that's your case, name those capabilities yourself with the manual form below — `keep:` takes one value, so it is `auto` or an explicit set, not both.

Prefer to say it yourself? The manual form keeps exactly these capabilities (plus the always-included core — parse, write, edit, forms, build):

```yaml
      keep: [render, signatures]
```

<details>
<summary><b>🧩 shrinking or debugging the engine — the <code>build:</code> knob</b></summary>

<br>

Alongside `keep` (which picks _which_ capabilities go in) there's `build` (what the engine is _optimized for_ — speed, size, or debugging). It lives in the same pubspec block and works the same on web and native:

```yaml
hooks:
  user_defines:
    pdf_manipulator:
      build: speed   # speed (default) | size | debug
```

- **`speed`** — the shipped default. The prebuilt binaries are this; nothing compiles.
- **`size`** — optimizes for size over speed (a smaller engine, somewhat slower). Compiles from source, like any `keep` subset.
- **`debug`** — keeps debug symbols so a native engine crash points to a file and line. For diagnosing an engine problem, not for shipping. Compiles from source.

Only `speed` has prebuilt binaries — `size` and `debug` always compile on your machine (so they need [Rust](https://rustup.rs), exactly like a `keep` subset). `build` and `keep` stack: `size` + a tight `keep:` is the smallest possible engine.

</details>


How do you know what to keep? Each capability covers a small set of methods. Core is always included:

| Capability | Keep it if you call | Also brings | Adds (native) |
|---|---|---|---|
| `core` | everything else — merge, split, forms, watermark, encrypt, build… | — | always included (~6.7 MB) |
| `render` | `doc.render()`, `editor.reduceImages()`, the `compress` one-shot, `editor.redact()`, `editor.markRedaction()`, `editor.applyRedactions()` | — | +4.8 MB |
| `signatures` | `sign()`, `doc.signatures()`, `doc.verifySignatures()` | — | +1.0 MB |
| `pdfa` | `doc.validatePdfA()`, `doc.validatePdfUa()`, `convertToPdfA` | — | +0.1 MB |
| `extract` | `doc.extractText()`, `doc.search()`, `doc.classifyPages()`, `doc.classifyDocument()` | — | +3.2 MB |
| `office` | `convertTo`, `convertToPdf` (DOCX / PPTX / XLSX) | `extract`, automatically | +2.9 MB on top of `extract` |

Dependencies are handled for you: `keep: [office]` switches on `extract` as well. Costs are measured one capability at a time, and capabilities share some code — so a combination can total a little less than the sum of its rows.

Not sure? Use `keep: auto` — the scan answers this for you. And if you ever guess wrong, the error message names the missing capability.

Want to see it live? [`example_trimmed/`](example_trimmed/) runs the full example app under a `keep: [render]` engine — its smoke test asserts that kept capabilities work and excluded ones answer the typed error.

On web, just re-run the setup after configuring pubspec — it reads the same
`keep:` block:

```bash
flutter pub run pdf_manipulator:setup
```

<details>
<summary><b>🧩 how keep works</b></summary>

- `auto` reads your app's code and keeps every capability it can reach. If any file can't be analyzed, you get the full binary and a warning — it never guesses.
- The custom engine compiles once and is cached — later builds reuse it.
- Call something you left out and you get a clear error saying what to add to `keep:`. No crashes, no silent misbehavior.
- A typo in `keep:` fails the build and prints the valid options.

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

Internally the modes differ in *how* the WASM engine gets your bytes: the top two stream them on demand, while the fallback copies the whole file into private browser storage first (works everywhere, just a slower first byte).

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
<summary><b>🧰 Advanced: force a mode, or enable streaming on older browsers</b></summary>

<br>

Pin a mode, or just check which one was chosen (native ignores this):

```dart
final pdf = Pdf(config: PdfConfig(webIoMode: PdfIoMode.atomics)); // pin a mode
final mode = await pdf.ensureInitialized(); // or just check
```

**Enable streaming on older browsers.** Chrome 137+ / Firefox 139+ already stream via JSPI with no setup. For older browsers that have `SharedArrayBuffer`, two server headers switch the fallback from disk-copy (OPFS) to streaming (Atomics):

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
