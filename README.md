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

> **Upgrading from an earlier version?** The [migration guide](docs/MIGRATION.md) takes you from any release, the old Android-only plugin (0.x) included. Find your version in its first table and read the sections it names, in order. Each change shows the code before and after, and both are compiled against their versions, so you can also hand the guide to an AI assistant ([how](docs/MIGRATION.md#migrating-pdf_manipulator)).

| | What it does |
|:---|:---|
| **Read** | text (plain, Markdown, HTML), search, pages as PNG, the images inside, form fields, signatures, attachments, annotations, bookmarks |
| **Edit** | merge, split, remove, keep, reorder, move, rotate and crop pages; watermarks, stamps, attachments; fill and flatten forms; redact for good; metadata |
| **Secure** | encrypt, decrypt, sign, verify signatures, strip metadata and scripts |
| **Standards** | validate PDF/A and PDF/UA, convert to PDF/A |
| **Shrink** | compress, reduce images, drop embedded copies of the standard fonts |
| **Make** | pages with text, tables, shapes, images and form fields; photos to PDF |
| **Convert** | DOCX, PPTX and XLSX to PDF and back; XFA forms to AcroForm |

---

<details>
<summary><b>👀 Peek inside</b></summary>

- [Install](#install)
  - [Add the dependency](#add-the-dependency)
  - [Native](#native)
  - [Web](#web)
- [Quick start](#quick-start)
  - [Merge two PDFs](#merge-two-pdfs)
  - [One read, one edit](#one-read-one-edit)
  - [Cancellation](#cancellation)
- [Sources & sinks](#sources--sinks)
- [Usage](#usage)
  - [Read a document](#read-a-document)
  - [Edit a document](#edit-a-document)
  - [Build from scratch](#build-from-scratch)
  - [Whole-file jobs](#whole-file-jobs)
  - [I want to... (every call)](#i-want-to-every-call)
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

Setup writes into `web/pdf_manipulator/`: `executor_worker.js`, `pdf_oxide.js`, `pdf_oxide_manifest.json` and the wasm in parts of 16 MiB (`pdf_oxide_bg.0.wasm`, `pdf_oxide_bg.1.wasm`, and with `asyncify: true` the `pdf_oxide_asyncify_bg.<n>.wasm` ones).

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

// your two PDFs, as inputs
final firstPdf = PdfInput(MemorySource(firstPdfBytes));
final secondPdf = PdfInput(MemorySource(secondPdfBytes));

// where the merged PDF lands
final output = MemorySink();

// combine the two into the output
await pdf.merge([firstPdf, secondPdf], output);

// take the merged bytes, then dispose it
final merged = output.takeBytes();
await pdf.dispose();
```

A `MemorySource` is the bytes; `PdfInput` says "open these bytes as a PDF" (and carries the password, when there is one).

On mobile or desktop, point the same program at files; only the inputs, sink, and import change:

```dart
import 'dart:io';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/io.dart'; // adds PdfFile / FileSource / FileSink

final pdf = Pdf();

// read the two PDFs straight from disk; PdfFile opens and closes each file
final firstPdf = PdfFile(File('first.pdf'));
final secondPdf = PdfFile(File('second.pdf'));

// write the result straight to a file
final output = await FileSink.open(File('merged.pdf'));

// the merge call is identical
await pdf.merge([firstPdf, secondPdf], output);

// flush the file, then dispose it
await output.close();
await pdf.dispose();
```

That's the shape of every **edit**: input in, sink out. Reading is the other shape: open a document and ask it questions, no sink needed.

### One read, one edit

Most jobs are one thing on one file. `readOnce` asks one question; `editOnce` makes one change and saves it:

```dart
final text = await pdf.readOnce(PdfFile(File('report.pdf'))).text();

final output = await FileSink.open(File('draft.pdf'));
await pdf.editOnce(PdfFile(File('report.pdf')), output).addWatermark('DRAFT');
await output.close();
```

Each one opens the file, runs the call, saves when it is an edit, and closes the file. Every read and every edit in this README works this way. For several calls on one document, see [Usage](#usage).

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

- **The `cancel()` call never throws** — it sends a stop request and returns. No `try/catch` needed; it's safe to call twice, and it does nothing after the task is done.
- **`PdfCancelled` only appears when you `await` the task** — never a half-finished result. That's the spot to wrap in `try/catch`.
- **Never await the task? Nothing to handle** — cancel it and move on, no error, no crash.

A cancelled edit leaves its document as it was, and a sink it wrote to gets `abort()`, so no half-written output stays behind.

To stop *everything*, **`pdf.dispose()`** cancels every operation and returns at once; it never waits for work in progress to end.

<details>
<summary><b>🧩 Advanced: when do i want more than one <code>Pdf</code>?</b></summary>

<br>

**Most apps need just one `Pdf` instance**: create it once, reuse it everywhere.

You'd want a second one when **two parts of your app should stop independently.** Say each screen runs its own PDF work: give each screen its own `Pdf`, then `dispose()` it when the user leaves, which cancels only *that* screen's jobs and leaves the rest running. (With one shared `Pdf`, `dispose()` would stop everything, everywhere.)

Each `Pdf` instance runs its operations in parallel, on background workers it borrows from a shared pool. Want more or fewer at a time?

```dart
final pdf = Pdf(config: PdfConfig(concurrency: 8)); // default: max(2, cores ÷ 2)
```

</details>

---

## Sources & sinks

Every operation reads from a **source** and writes to a **sink**. The built-ins cover the usual places:

| Where | Read a PDF from it | Write the result to it |
|:---|:---|:---|
| Memory | `PdfInput(MemorySource(bytes))` | `MemorySink()`, then `takeBytes()` |
| A file (`io.dart`) | `PdfFile(file)` | `await FileSink.open(file)` |
| A picked browser file (`web.dart`) | `PdfInput(BlobSource(file))` | `MemorySink()` |
| A server, a database | your own `DataSource` | your own `DataSink` |

A `DataSource` (`MemorySource`, `FileSource`, `BlobSource`, or your own) is where the bytes are. A `PdfInput` is a PDF to open: a source plus its password. Calls that open a PDF take a `PdfInput`. Calls that take other bytes (an image, a font, a Word file, a signing certificate) take a plain `DataSource`.

**Passwords** go with the input: `PdfInput(source, password: 'secret')` or `PdfFile(file, password: 'secret')`. A PDF can have a user password and an owner password; either one opens it, so the input takes one. The permission flags of a PDF (print, copy, change) are requests to viewer apps: this library reads and changes any PDF it can open.

**Closing:** you close what you opened, like a `FileSink` or a `FileSource`. The library closes what it opened: the file of a `PdfFile`, and each sink your function returns to `split`, `render` or `extractImages`, also after an error or a cancel. Memory sources and sinks need no close.

<details>
<summary><b>🧩 what are sources & sinks, really? (+ rolling your own)</b></summary>

<br>

A **source** is how `pdf_manipulator` reads your PDF, never all at once, just small bites:

```dart
abstract interface class DataSource {
  int get length; // how big are you?
  FutureOr<Uint8List> readAt(int offset, int count); // count bytes from offset
  FutureOr<void> close(); // let go of what you hold
}
```

It only asks for small pieces — *"give me 64KB starting here"*, then the next bit, then the next. So you hand it a **reader**, not your whole file in one giant `Uint8List`. (It jumps to any spot in the file, so a one-way stream like a live socket can't be a source; keep those in memory or a file first.)

That is also why memory stays small. An operation holds at most 16 MiB over what the process held before it, whatever the file size. Reading the text of one page adds a fixed, tested number of bytes per piece of text on that page. Three things cost more: an image drawn at full size holds its pixels while it is decoded, `convertToPdfA` holds the one metadata packet it patches, and converting an Office file to PDF reads the whole Office file into memory. Nothing is skipped or refused at any size.

A **sink** is the mirror:

```dart
abstract interface class DataSink {
  FutureOr<void> write(Uint8List chunk); // here's a chunk of output
  FutureOr<void> abort(Object reason); // the job failed or was cancelled: throw it all away
  FutureOr<void> close(); // let go of what you hold
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

  @override
  void close() {} // nothing to let go of
}

// then use it exactly like any other source
final doc = await pdf.open(PdfInput(UrlSource(uri, contentLength)));
```

A browser `Blob` from a file picker or a drag-and-drop needs no code of its own: `BlobSource` ships in `package:pdf_manipulator/web.dart`, and the engine reads it in place, with no copy:

```dart
import 'package:pdf_manipulator/web.dart';

final doc = await pdf.open(PdfInput(BlobSource(file))); // file is a web.File
```

Same idea for a custom `DataSink`: implement `write(chunk)` for an upload stream, a database column, wherever the bytes should go, and `abort(reason)` to take them back (cancel the upload, delete the row), so a stopped job leaves nothing half-written.

A file that changes on disk while a job reads it fails that job with an I/O error; it never reads wrong bytes.

</details>

---

## Usage

There are three ways to call the library. They differ in one thing: who closes the document.

| | Read | Edit | Make | Closed by |
|:---|:---|:---|:---|:---|
| **One call** | `readOnce` | `editOnce` | `imagesToPdf` | the library |
| **A function** | `read` | `rewrite` | `buildTo` | the library |
| **A handle** | `open` | `edit` | `build` | you |

```dart
// one call: closed after the call
final text = await pdf.readOnce(input).text();
await pdf.editOnce(input, output).addWatermark('DRAFT');

// a function: one open for several calls, closed when the function returns
await pdf.rewrite(input, output, (editor) async {
  await editor.removePages(PdfPages.single(0));
  await editor.addWatermark('DRAFT');
});

// a handle: open as long as you need, closed by you
final doc = await pdf.open(input);
final cover = await doc.bitmap(0);
await doc.dispose();
```

Pick the smallest one that fits. One call is the shortest; a second call on the same `readOnce` or `editOnce` object throws `StateError`. A function opens the file once for many calls, and the library still closes it, also after an error or a cancel. A handle stays open as long as you need it, for example in a viewer.

Jobs on whole files are one call each, with inputs and sinks like everything else: `merge`, `split`, `compress`, `resave`, `info`, `sign`, `convertToOffice`, `convertFromOffice`, `convertXfaToAcroForm` ([Whole-file jobs](#whole-file-jobs)).

Highlights below; every method and full signature lives in the [API reference](https://pub.dev/documentation/pdf_manipulator/latest/).

### Read a document

`pdf.read` opens the document, hands it to your function, and closes it afterwards, also after an error or a cancel:

```dart
final hits = await pdf.read(input, (doc) async {
  print('${doc.pageCount} pages · encrypted: ${doc.encryption != null}');

  final text = await doc.text();
  for (final field in await doc.formFields()) {
    print('${field.name}: ${field.kind.name} = ${field.value}');
  }

  // every page as a PNG, each to the sink you return for it
  await doc.render((page) => FileSink.open(File('page-${page.page}.png')),
      size: PdfRenderSize.fit(maxWidth: 200, maxHeight: 200));

  return doc.search('revenue');
});
```

Need it open longer, say for a viewer? `pdf.open(input)` gives you the document itself; `dispose()` it when you're done.

What you can read:

| Kind | Calls |
|:---|:---|
| Text | `text` (a string), `extractText` (to a sink, page by page, for long documents), `search` (page, text and bounds of each hit); plain text, Markdown or HTML |
| Pictures | `bitmap` (one page, in memory), `render` (every page, each to its own sink; `PdfRenderSize.dpi(150)` by default), `extractImages` |
| Pages | `pageInfo` (size, crop box, rotation, label), `classifyPages`, `classifyDocument`, `bookmarkRanges`, `annotations` |
| Forms | `formFields`, `exportFormData` (FDF or XFDF), `xfa` |
| Signatures | `signatures`, `verifySignatures` (the digest and the signer key; the trust of the certificate is not checked) |
| Standards | `validatePdfA`, `validatePdfUa` |
| Attachments | `attachments`, `extractAttachment` |
| Facts | `pageCount`, `version`, `isTagged`, `metadata`, `encryption`: read when the document opens, so no `await` |

Pages are a `PdfPages`: `.all()`, `.single(3)`, `.range(0, 10)` (pages 0 to 9, like `List.sublist`), `.from(5)`, `.list([1, 4, 7])`. A list that is empty, negative or names a page twice throws `ArgumentError` when you build it.

### Edit a document

`pdf.rewrite` is for *many* changes to one PDF. It parses once, applies everything in memory, and writes once when your function returns. If your function throws, nothing is written:

```dart
await pdf.rewrite(input, output, (editor) async {
  await editor.setMetadata(const PdfMetadata(title: 'Q4 Report'));
  await editor.appendPages(PdfInput(appendix));
  await editor.removePages(PdfPages.single(4));
  await editor.addWatermark('FINAL',
      pages: PdfPages.single(0), style: PdfWatermarkStyle(opacity: 0.15));
  final report = await editor.reduceImages(PdfImagePolicy.ebook); // 150 ppi, JPEG q75
  print('${report.changed} images, ${report.bytesBefore} → ${report.bytesAfter} bytes');
});
```

Need the editor open longer? `pdf.edit(input)` gives it to you; `save(output)` and `dispose()` it yourself.

What you can edit:

| Kind | Calls |
|:---|:---|
| Pages | `removePages`, `keepPages` (in document order), `reorderPages` (a `PdfPageOrder`, in your order), `movePage`, `rotatePages` (relative), `setRotation` (absolute), `appendPages` ¹ |
| Page boxes | `setMediaBox`, `setCropBox` (`null` removes the crop), `cropMargins` |
| Marks | `addWatermark`, `addStamp`, `addImageStamp`, `addAttachment` (with description, mime type and relationship) |
| Forms | `editor.field(name)` ¹, `flattenForms`, `flattenAnnotations` (each takes `pages:`) |
| Images and fonts | `pageImages` ¹ with `setImageBounds`, `reduceImages`, `unembedStandardFonts` |
| Removal | `redact`, `markRedaction` / `applyRedactions` / `clearRedactions` ¹, `sanitize` (metadata, scripts, attached files) |
| Document | `setMetadata`, `convertToPdfA` |
| Read back ¹ | `pageCount`, `version`, `isModified`, `metadata`, `pageInfo` |

¹ On the editor of `rewrite` and `edit` only. Every other edit also works as one call: `pdf.editOnce(input, output).rotatePages(by: .r90)`.

An edit that takes pages checks all of them before it changes one: a page the document does not have fails with `PdfPageOutOfRange`, and the document stays as it was.

#### Form fields

A form field is edited through `editor.field(name)`:

```dart
await editor.field('name').setValue(const PdfFormFieldValue.text('Ada'));
await editor.field('agree').setValue(const PdfFormFieldValue.checked(true));
await editor.field('country').setValue(const PdfFormFieldValue.choice('DE'));
await editor.field('tags').setValue(const PdfFormFieldValue.choices(['a', 'c'])); // a multi-select list
await editor.field('name').setValue(const PdfFormFieldValue.none()); // clear it
```

A field also has `setReadOnly` / `setRequired` / `setFlags`, `setTooltip`, `setBounds`, `setMaxLength`, `setAlignment`, `setBackgroundColor` / `setBorderColor` / `setBorderWidth` / `setAppearance`, and `remove`. A choice that is not an option of the field throws `PdfEngineFailed` and changes nothing.

#### Save options

Passed as `save:` to `rewrite`, `editOnce`, `resave`, `merge` or `editor.save`:

- `PdfSave.full()` — default; writes the file again, deflates the streams at the best level and drops unused objects. Its other settings: `deflate` (`PdfDeflate.none`, `.fast`, `.best`), `garbageCollect`, `useObjectStreams`, `mergeDuplicates` (one copy of identical images, ICC profiles and fonts), `pruneResources` (drop page resources nothing draws).
- `PdfSave.full(encryption: ...)` — encrypt on save (or `PdfEncryption.remove()` to strip it; `PdfEncryption.keep()` is the default).
- `PdfSave.incremental()` — appends the changes to the original bytes, so an existing signature stays valid. An edit an append cannot carry (a removed page, a redaction) throws `PdfIncrementalRefused` and writes nothing.
- `PdfSave.incrementalOrRewrite()` — appends when it can, writes the file again when it can't. The rewrite breaks a signature, as any full save does.

#### Redaction

Redaction removes the content, it doesn't just hide it. `redact` does it in one step; `markRedaction` queues marks and `applyRedactions` removes them all at once:

```dart
await editor.redact(0, [PdfRect(x: 72, y: 700, width: 200, height: 20)]); // gone for good

await editor.markRedaction(0, [PdfRect(x: 72, y: 650, width: 200, height: 20)]);
print(await editor.redactionCount(0)); // pending marks on this page
final report = await editor.applyRedactions();
print('${report.glyphsRemoved} glyphs and ${report.imagesRemoved} images removed');
```

A black box is drawn over each region by default: `overlay: PdfRedactionOverlay.fill(PdfColor.red)` for another colour, `PdfRedactionOverlay.none()` for no box. An image under a region keeps a black patch where its pixels were removed, whatever the overlay.

### Build from scratch

`pdf.buildTo` hands your function an empty PDF; add pages, then content. The library saves it when your function returns:

```dart
await pdf.buildTo(output, (builder) async {
  await builder.setMetadata(const PdfMetadata(title: 'Invoice #1042'));

  final page = await builder.addPage(); // A4
  await page.heading(1, 'Invoice');
  await page.paragraph('Thank you for your purchase.');
  await page.table(
    const [PdfTableColumn('Item'), PdfTableColumn('Qty', align: PdfAlignment.right)],
    const [[PdfTableCell('A-1'), PdfTableCell('12')]],
  );
  await page.textField('notes', PdfRect(x: 50, y: 400, width: 300, height: 100));
  await page.text('example.com', link: const PdfLink.url('https://example.com'));
});
```

Need the builder open longer? `pdf.build()` gives it to you; `save(output)` and `dispose()` it yourself.

- **Pages:** `addPage()` (A4), `addPage(PdfPageSize.letter())`, `addPage(PdfPageSize.points(400, 300))`, and `addImagePage` for a page that is one picture, at the picture's size unless you name one. Photos to a PDF in one call: `pdf.imagesToPdf([a, b], output)`.
- **Text:** `text` / `appendText` (with a `PdfTextStyle`, a `PdfLink` or a `PdfMarkup`), `heading`, `paragraph`, `textIn`, `columns`, `footnote`, `space`, `newline`, `setFont`, `moveTo` (a `PdfPoint`).
- **Drawing:** `image`, `rect` (filled, stroked or both), `line`, `table`, `horizontalRule`, `stickyNote`, `freeText`, `stamp`, `watermark`.
- **Form fields:** `textField`, `checkbox`, `radioGroup`, `comboBox`, `pushButton`, `signatureField`, each with Acrobat JavaScript actions (`PdfFieldScripts`).
- **Page actions:** `onOpen`, `onClose`.

A page ends when the next one is added or the document is saved.

### Whole-file jobs

```dart
// smaller: streams deflated, unused objects dropped, images kept as they are
await pdf.compress(input, output);
await pdf.compress(input, output, images: PdfImagePolicy.ebook); // images to 150 ppi

// encrypt, or decrypt, with no other change
await pdf.resave(input, output,
    save: PdfSave.full(
      encryption: PdfEncryption.password(
        ownerPassword: 'secret',
        algorithm: PdfEncryptionAlgorithm.aesKey256,
      ),
    ));
await pdf.resave(PdfInput(source, password: 'secret'), output,
    save: PdfSave.full(encryption: PdfEncryption.remove()));

// one file per 5 pages; or .maxBytes(5000000), or .bookmarks()
final parts = await pdf.split(input, (part) => FileSink.open(File('part-${part.index}.pdf')),
    by: PdfSplitRule.every(5));

await pdf.sign(input, output,
    credentials: PdfSigningCredentials.pkcs12(certificate, 'p12-password'));

await pdf.convertToOffice(input, output, format: .docx); // or .pptx, .xlsx
await pdf.convertFromOffice(docx, output, format: .docx);

final info = await pdf.info(input); // page count, version, metadata, encryption
```

- `compress` never makes a file larger: if the result is not smaller, the output gets the input's bytes as they are. The image presets are `PdfImagePolicy.screen`, `.ebook`, `.print` and `.lossless` (the default).
- `resave` is an edit with no change: it saves the file again with the `PdfSave` you give it.
- `split` asks your function for a sink before each part and closes it after the part. It returns each part's `index`, `start` and `end` page and, for `.bookmarks()`, its `title`.
- `merge` appends the whole of each input to the first one. For only some pages, use `appendPages` in `rewrite`.
- `sign` keeps the earlier signatures and appends one update section. An encrypted input is refused with `PdfUnsupported`.

### I want to... (every call)

Every call, one line each, grouped like the sections above. In the table:

- `input` is a `PdfInput` and `output` a `DataSink`
- `doc` is the document of `pdf.read` or `pdf.open`
- `e` is the editor of `pdf.rewrite(input, output, (e) async { ... })`; for one edit, `pdf.editOnce(input, output)` works the same, except for `appendPages` and `field`
- `b` is the builder of `pdf.buildTo`

<details>
<summary><b>🧩 show the table</b></summary>

| I want to | Do this |
|:---|:---|
| **Open, use, close** | |
| run one read | `await pdf.readOnce(input).search('invoice')` |
| read a document and let the library close it | `await pdf.read(input, (doc) => doc.text())` |
| keep a document open for many reads | `final doc = await pdf.open(input)`, then `await doc.dispose()` |
| get the page count, version, metadata and encryption | `await pdf.info(input)` |
| make one edit | `await pdf.editOnce(input, output).addWatermark('DRAFT')` |
| make several edits and save once | `await pdf.rewrite(input, output, (e) async { ... })` |
| keep an editor open | `final editor = await pdf.edit(input)`, then `await editor.dispose()` |
| encrypt, decrypt or recompress with no edit | `await pdf.resave(input, output, save: PdfSave.full(encryption: PdfEncryption.remove()))` |
| build a document and let the library save it | `await pdf.buildTo(output, (b) async { ... })` |
| keep a builder open | `final builder = await pdf.build()`, then `await builder.dispose()` |
| **Whole files** | |
| merge PDFs | `await pdf.merge([PdfInput(a), PdfInput(b)], output)` |
| split a PDF | `await pdf.split(input, (part) => sink, by: PdfSplitRule.every(5))` |
| make a PDF smaller | `await pdf.compress(input, output)` |
| make a PDF from images | `await pdf.imagesToPdf([a, b], output)` |
| sign a PDF | `await pdf.sign(input, output, credentials: credentials)` |
| convert a PDF to DOCX, PPTX or XLSX | `await pdf.convertToOffice(input, output, format: .docx)` |
| convert a DOCX, PPTX or XLSX to PDF | `await pdf.convertFromOffice(docx, output, format: .docx)` |
| convert an XFA form to an AcroForm | `await pdf.convertXfaToAcroForm(input, output)` |
| **Text and pictures** | |
| get the text as a string | `await doc.text()` |
| write the text to a sink | `await doc.extractText(sink)` |
| search the text | `await doc.search('invoice')` |
| render one page to bytes | `await doc.bitmap(0)` |
| render pages to files | `await doc.render((page) => sinkFor(page))` |
| extract the images | `await doc.extractImages((image) => sinkFor(image))` |
| **Pages** | |
| read the size, rotation and label of the pages | `await doc.pageInfo()` |
| read the page range of each bookmark | `await doc.bookmarkRanges()` |
| classify the pages | `await doc.classifyPages()` |
| classify the whole document | `await doc.classifyDocument()` |
| append pages of another PDF | `await e.appendPages(PdfInput(other), pages: PdfPages.range(0, 2))` |
| remove pages | `await e.removePages(PdfPages.list([1, 3]))` |
| keep some pages, in document order | `await e.keepPages(PdfPages.from(2))` |
| put the pages in a new order | `await e.reorderPages(PdfPageOrder([2, 0, 1]))` |
| move a page | `await e.movePage(from: 3, to: 0)` |
| rotate pages | `await e.rotatePages(by: .r90, pages: PdfPages.single(0))` |
| set the rotation of pages | `await e.setRotation(.r0)` |
| resize the pages | `await e.setMediaBox(PdfRect(x: 0, y: 0, width: 595, height: 842))` |
| crop the pages, or remove the crop | `await e.setCropBox(null)` |
| trim the margins off every page | `await e.cropMargins(top: 36, bottom: 36)` |
| **Marks and annotations** | |
| watermark the pages | `await e.addWatermark('DRAFT', pages: PdfPages.range(0, 3))` |
| stamp the pages | `await e.addStamp(.draft, bounds: PdfRect(x: 50, y: 700, width: 120, height: 40))` |
| stamp a page with an image | `await e.addImageStamp(logo, bounds: PdfRect(x: 50, y: 700, width: 80, height: 40), pages: PdfPages.single(0))` |
| list the annotations of a page | `await doc.annotations(0)` |
| flatten the annotations | `await e.flattenAnnotations(pages: PdfPages.single(0))` |
| **Forms** | |
| read the form fields | `await doc.formFields()` |
| fill a form field | `await e.field('name').setValue(const PdfFormFieldValue.text('Ada'))` |
| flatten the form | `await e.flattenForms()` |
| export the form values | `await doc.exportFormData(sink)` |
| read the XFA packet | `await doc.xfa()` |
| **Attachments** | |
| list the attached files | `await doc.attachments()` |
| write an attached file to a sink | `await doc.extractAttachment('notes.txt', sink)` |
| attach a file | `await e.addAttachment('notes.txt', notes)` |
| **Removal** | |
| redact a region for good | `await e.redact(0, [region])` |
| erase a region and draw no box | `await e.redact(0, [region], overlay: .none())` |
| remove the metadata, scripts or attached files | `await e.sanitize(stripAttachments: true)` |
| **Signatures and standards** | |
| list the signatures | `await doc.signatures()` |
| verify the signatures | `await doc.verifySignatures()` |
| validate PDF/A | `await doc.validatePdfA()` |
| validate PDF/UA | `await doc.validatePdfUa()` |
| convert to PDF/A | `await e.convertToPdfA()` |
| **Images, fonts and metadata** | |
| shrink the images | `await e.reduceImages(PdfImagePolicy.ebook)` |
| move or resize an image on a page | `await e.setImageBounds(0, 'Im0', PdfRect(x: 0, y: 0, width: 200, height: 100))` |
| remove embedded copies of the standard fonts | `await e.unembedStandardFonts()` |
| set the title and other metadata | `await e.setMetadata(const PdfMetadata(title: 'Q4 Report'))` |
| **Build** | |
| add a page and write on it | `final page = await b.addPage()`, then `await page.paragraph('Thank you.')` |

</details>

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

The font stays in memory after you register it — that is what makes every later fill fast. Each background worker holds its own copy, so when one language is enough, prefer the single-language file over the combined one.

Without a registered font the fill still succeeds — the value is stored correctly and readers with their own fonts display it; only the baked-in (flattened) appearance falls back to the field's font.

---

## Error handling

Every failure is a typed subclass of `PdfError`, and it reaches you through the `await`: no string matching, no `PlatformException`. Catch the cases you handle specially; let the rest fall to a catch-all. Each error carries a human-readable `message`.

```dart
try {
  await pdf.read(input, (doc) async {
    // ... use the document
  });
} on PdfPasswordRequired {
  // encrypted — retry with PdfInput(source, password: '...')
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

A mistake in your own code is not a `PdfError`: an empty page list, an empty `merge` list or a second call on a one-shot throws an `ArgumentError` or a `StateError` right at the call, before any work starts. Fix the code; don't catch it.

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
| `render` | `doc.render()`, `doc.bitmap()`, `doc.extractImages()`, `editor.reduceImages()`, `pdf.compress()`, `editor.redact()`, `editor.markRedaction()`, `editor.applyRedactions()` | — | +4.8 MB |
| `signatures` | `pdf.sign()`, `doc.signatures()`, `doc.verifySignatures()` | — | +1.0 MB |
| `pdfa` | `doc.validatePdfA()`, `doc.validatePdfUa()`, `editor.convertToPdfA()` | — | +0.1 MB |
| `extract` | `doc.extractText()`, `doc.text()`, `doc.search()`, `doc.classifyPages()`, `doc.classifyDocument()` | — | +3.2 MB |
| `office` | `pdf.convertToOffice()`, `pdf.convertFromOffice()` (DOCX / PPTX / XLSX) | `extract`, automatically | +2.9 MB on top of `extract` |

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

Internally the modes differ in *how* the WASM engine gets your bytes: JSPI, Atomics and Asyncify stream them on demand, while OPFS copies the whole file into private browser storage first (works everywhere, just a slower first byte).

| Mode | What it means for you | Streams large files | Picked when |
|---|---|:---:|---|
| **JSPI** | Best: streams, zero setup | ✅ | Chrome 137+ · Firefox 139+ |
| **Atomics** | Streams, but needs two server headers | ✅ | COOP/COEP headers are set |
| **OPFS** | Copies the file to disk first, then reads | ❌ | the browser opens its private file system |
| **Asyncify** | Streams without headers on older browsers, with a second, larger WASM | ✅ | OPFS is refused and you turned on `asyncify: true` ([below](#asyncify-the-second-wasm)) |

<details>
<summary><b>🧩 ok but what do JSPI, Atomics, Asyncify and OPFS actually mean?</b></summary>

<br>

All four solve the same puzzle (synchronous WASM code needs bytes that arrive asynchronously from Dart), just in different ways:

- **JSPI** (JavaScript Promise Integration) — the browser lets the WASM call pause and resume while it waits for the next chunk. Cleanest path, no setup; needs a recent browser. Used automatically where available.
- **Atomics** — the WASM side blocks on a `SharedArrayBuffer` while the main thread fills it. Works on older browsers, but `SharedArrayBuffer` only switches on when your site sends two security headers (see below).
- **Asyncify** — a second build of the engine that can stop in the middle of a call and continue later (binaryen's `--asyncify`), so it waits for the next chunk the way JSPI does without needing the browser's help or any header. You opt in; see below.
- **OPFS** (Origin Private File System) — when none of the above is available, the package copies your file into the browser's private on-disk storage and reads from there. Works everywhere; the copy means a slower first byte and a little disk use.

You never choose; the package tries Atomics, then JSPI, then OPFS (it must open the browser's private file system, which some private-browsing modes refuse), then Asyncify (only if your app ships it), and uses the first that works. OPFS comes before Asyncify because it measured about 2.2 times faster, the copy included. With none of them, `ensureInitialized` throws `PdfUnsupported`.

</details>

<details>
<summary><b>🧰 Advanced: force a mode, or enable streaming on older browsers</b></summary>

<br>

Pin a mode, or just check which one was chosen (native ignores this):

```dart
final pdf = Pdf(config: PdfConfig(webIoMode: PdfIoMode.atomics)); // pin a mode
final mode = await pdf.ensureInitialized(); // or just check
```

Forcing a mode the browser cannot run, or Asyncify when your app did not ship its WASM (no entry in `pdf_oxide_manifest.json`), throws `PdfUnsupported` when the `Pdf` initializes (`ensureInitialized`, or its first operation); it never falls back silently. A missing or unreadable `pdf_oxide_manifest.json` is the same error, naming the file and the setup command.

**Self-hosting the assets.** Serve `.wasm` files as `application/wasm` with compression on: a static host picks the compression from the extension, and the parts total 8.1 MB gzipped (10.2 MB with Asyncify). Keep every file beside `executor_worker.js`. A service worker that precaches assets reads `pdf_oxide_manifest.json` and caches the parts it lists. The page checks each part's size and SHA-256 against the manifest.

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

#### Asyncify, the second WASM

If you cannot send those headers and still want streaming on browsers without JSPI, switch on the Asyncify engine in your pubspec and run setup again (web only; native ignores the key):

```yaml
hooks:
  user_defines:
    pdf_manipulator:
      asyncify: true   # false (default) ships nothing new
```

Setup then also provides the Asyncify wasm's parts, `pdf_oxide_asyncify_bg.0.wasm` and onward, next to the normal one and listed in `pdf_oxide_manifest.json`: downloaded for the full engine, built from source for a `keep:` subset. A browser fetches only the parts of the module its mode runs, so JSPI, Atomics and OPFS visitors never download the larger one; auto-detection picks Asyncify only where OPFS cannot run.

The trade-off, plainly: the Asyncify module is about a third larger than the normal one (the full engine: 28.1 MB against 20.8 MB, 10.2 MB against 8.1 MB gzipped), because most of the engine can be in the middle of a read when it stops. Every module ships as parts of 16 MiB, so neither reaches the 25 MiB per-file limit of Cloudflare Pages and Workers static assets. The instrumented engine also runs slower, and every read is a round trip to the page: on a 300 MB text PDF, opening and extracting 50 pages took 93 s with Asyncify against 41 s with OPFS (the copy included), and merging two such files 16–19 s against 9–10 s. Turn it on when streaming matters more than those costs, and measure your own files.

</details>

---

## Not in the box

A tiny wishlist — what the shipped package doesn't do *yet*, and what to grab in the meantime. For the full engine-vs-shipped picture, see the [capability roadmap](docs/CAPABILITY_ROADMAP.md).

- **A viewer to display PDFs.** This is a manipulation library, not a UI widget. To put a PDF on screen, use [`pdfx`](https://pub.dev/packages/pdfx) or [`flutter_pdfview`](https://pub.dev/packages/flutter_pdfview). It pairs well: pre-process here (merge, decrypt, watermark), display there. (It does render pages to image bytes with `doc.bitmap(...)` or `doc.render(...)`, if you would rather draw your own surface.)
- **OCR and table extraction.** `extract` and `search` read the text already in a PDF, so a scanned page (just an image) comes back empty, and clean rows-and-columns is a separate problem. The engine *has* both (a PaddleOCR pipeline and ML table detection); the default build leaves them out so it doesn't pull the ONNX runtime and ~12 MB of models into every install. An opt-in build is on the [roadmap](docs/CAPABILITY_ROADMAP.md); until then, run them externally ([Tesseract](https://github.com/tesseract-ocr/tesseract), [Camelot](https://github.com/camelot-dev/camelot), or a cloud API) and feed the result back. Want them first-party? [Open an issue](https://github.com/whuppi/pdf_manipulator/issues); it's how we gauge demand.

---

## Docs

The README covers the everyday tasks. These pages go deeper:

| Doc | What's inside |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | How it's built: layers, streaming I/O, four web modes |
| [Capabilities](docs/CAPABILITY_ROADMAP.md) | What's shipped, what's planned |
| [Updating](docs/UPDATING.md) | Maintaining the vendored Rust engine |
| [Migration](docs/MIGRATION.md) | Upgrading from any earlier version, the old Android-only plugin (0.x) included |
| [Contributing](CONTRIBUTING.md) | Setup, PR workflow, adding operations |

---

## License

MIT. See [LICENSE](LICENSE).
