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

> **Coming from pdf_manipulator 5.x?** The [migration guide](docs/MIGRATION.md#from-501-to-600) has the before/after for every breaking call.

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
  - [Open a PDF](#open-a-pdf)
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

Web can't auto-download native assets, so run setup once. It fetches the prebuilt WASM engine into `web/pdf_manipulator/`: `executor_worker.js`, `pdf_oxide.js`, `pdf_oxide_manifest.json` and the wasm in parts of 16 MiB (`pdf_oxide_bg.0.wasm`, `pdf_oxide_bg.1.wasm`, and with `asyncify: true` the `pdf_oxide_asyncify_bg.<n>.wasm` ones). Run it again after any `pub upgrade`, because the assets are tied to the package version:

```sh
flutter pub run pdf_manipulator:setup
```

For a `File` from a picker or a drop, import `package:pdf_manipulator/web.dart` and wrap it in a `BlobSource`: the worker reads it in place, with no copy (see **Sources & sinks**).

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

`input` is a `PdfInput` (`PdfInput(source)`), `output` is a `DataSink`, `doc` is an open document and `e` is the editor of `pdf.rewrite(input, output, (e) async { ... })`.

| I want to | Do this |
|---|---|
| keep a document open for many reads | `final doc = await pdf.open(input)`, then `await doc.dispose()` |
| read a document and let the library close it | `await pdf.read(input, (doc) => doc.text())` |
| run one read | `await pdf.readOnce(input).search('invoice')` |
| get the page count, version, metadata and encryption | `await pdf.info(input)` |
| keep an editor open | `final editor = await pdf.edit(input)`, then `await editor.dispose()` |
| make several edits and save once | `await pdf.rewrite(input, output, (e) async { ... })` |
| make one edit | `await pdf.editOnce(input, output).addWatermark('DRAFT')` |
| encrypt, decrypt or recompress with no edit | `await pdf.resave(input, output, save: PdfSave.full(encryption: PdfEncryption.remove()))` |
| keep a builder open | `final builder = await pdf.build()`, then `await builder.dispose()` |
| build a document and let the library save it | `await pdf.buildTo(output, (b) async { ... })` |
| merge PDFs | `await pdf.merge([PdfInput(a), PdfInput(b)], output)` |
| split a PDF | `await pdf.split(input, (part) => sink, by: PdfSplitRule.every(5))` |
| make a PDF smaller | `await pdf.compress(input, output)` |
| make a PDF from images | `await pdf.imagesToPdf([a, b], output)` |
| sign a PDF | `await pdf.sign(input, output, credentials: credentials)` |
| convert a PDF to DOCX, PPTX or XLSX | `await pdf.convertToOffice(input, output, format: .docx)` |
| convert a DOCX, PPTX or XLSX to PDF | `await pdf.convertFromOffice(docx, output, format: .docx)` |
| convert an XFA form to an AcroForm | `await pdf.convertXfaToAcroForm(input, output)` |
| read the size, rotation and label of the pages | `await doc.pageInfo()` |
| get the text as a string | `await doc.text()` |
| write the text to a sink | `await doc.extractText(sink)` |
| search the text | `await doc.search('invoice')` |
| render pages to files | `await doc.render((page) => sinkFor(page))` |
| render one page to bytes | `await doc.bitmap(0)` |
| extract the images | `await doc.extractImages((image) => sinkFor(image))` |
| list the signatures | `await doc.signatures()` |
| verify the signatures | `await doc.verifySignatures()` |
| validate PDF/A | `await doc.validatePdfA()` |
| validate PDF/UA | `await doc.validatePdfUa()` |
| read the page range of each bookmark | `await doc.bookmarkRanges()` |
| classify the pages | `await doc.classifyPages()` |
| classify the whole document | `await doc.classifyDocument()` |
| read the form fields | `await doc.formFields()` |
| export the form values | `await doc.exportFormData(sink)` |
| read the XFA packet | `await doc.xfa()` |
| list the attached files | `await doc.attachments()` |
| write an attached file to a sink | `await doc.extractAttachment('notes.txt', sink)` |
| list the annotations of a page | `await doc.annotations(0)` |
| set the title and other metadata | `await pdf.rewrite(input, output, (e) => e.setMetadata(const PdfMetadata(title: 'Q4 Report')))` |
| append pages of another PDF | `await pdf.rewrite(input, output, (e) => e.appendPages(PdfInput(other), pages: PdfPages.range(0, 2)))` |
| remove pages | `await pdf.rewrite(input, output, (e) => e.removePages(PdfPages.list([1, 3])))` |
| keep some pages, in document order | `await pdf.rewrite(input, output, (e) => e.keepPages(PdfPages.from(2)))` |
| put the pages in a new order | `await pdf.rewrite(input, output, (e) => e.reorderPages(PdfPageOrder([2, 0, 1])))` |
| move a page | `await pdf.rewrite(input, output, (e) => e.movePage(from: 3, to: 0))` |
| rotate pages | `await pdf.rewrite(input, output, (e) => e.rotatePages(by: .r90, pages: PdfPages.single(0)))` |
| set the rotation of pages | `await pdf.rewrite(input, output, (e) => e.setRotation(.r0))` |
| resize the pages | `await pdf.rewrite(input, output, (e) => e.setMediaBox(PdfRect(x: 0, y: 0, width: 595, height: 842)))` |
| crop the pages, or remove the crop | `await pdf.rewrite(input, output, (e) => e.setCropBox(null))` |
| trim the margins off every page | `await pdf.rewrite(input, output, (e) => e.cropMargins(top: 36, bottom: 36))` |
| watermark the pages | `await pdf.rewrite(input, output, (e) => e.addWatermark('DRAFT', pages: PdfPages.range(0, 3)))` |
| stamp the pages | `await pdf.rewrite(input, output, (e) => e.addStamp(.draft, bounds: PdfRect(x: 50, y: 700, width: 120, height: 40)))` |
| stamp a page with an image | `await pdf.rewrite(input, output, (e) => e.addImageStamp(logo, bounds: PdfRect(x: 50, y: 700, width: 80, height: 40), pages: PdfPages.single(0)))` |
| flatten the form | `await pdf.rewrite(input, output, (e) => e.flattenForms())` |
| flatten the annotations | `await pdf.rewrite(input, output, (e) => e.flattenAnnotations(pages: PdfPages.single(0)))` |
| fill a form field | `await pdf.rewrite(input, output, (e) => e.field('name').setValue(const PdfFormFieldValue.text('Ada')))` |
| redact a region for good | `await pdf.rewrite(input, output, (e) => e.redact(0, [region]))` |
| erase a region and draw no box | `await pdf.rewrite(input, output, (e) => e.redact(0, [region], overlay: .none()))` |
| attach a file | `await pdf.rewrite(input, output, (e) => e.addAttachment('notes.txt', notes))` |
| convert to PDF/A | `await pdf.rewrite(input, output, (e) => e.convertToPdfA())` |
| shrink the images | `await pdf.rewrite(input, output, (e) => e.reduceImages(PdfImagePolicy.ebook))` |
| move or resize an image on a page | `await pdf.rewrite(input, output, (e) => e.setImageBounds(0, 'Im0', PdfRect(x: 0, y: 0, width: 200, height: 100)))` |
| remove embedded copies of the standard fonts | `await pdf.rewrite(input, output, (e) => e.unembedStandardFonts())` |
| remove the metadata, scripts or attached files | `await pdf.rewrite(input, output, (e) => e.sanitize(stripAttachments: true))` |
| add a page and write on it | `final page = await b.addPage()`, then `await page.paragraph('Thank you.')` |

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
await pdf.merge([PdfInput(firstPdf), PdfInput(secondPdf)], output);

// take the merged bytes, then dispose the instance
final merged = output.takeBytes();
await pdf.dispose();
```

On mobile or desktop, point the same program at files. `PdfFile` opens a file when the job needs it and closes it when the job ends, so you close only the sink:

```dart
import 'dart:io';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/io.dart'; // adds PdfFile / FileSource / FileSink

final pdf = Pdf();

// write the result straight to a file
final output = await FileSink.open(File('merged.pdf'));

// the merge call is the same; the inputs are files
try {
  await pdf.merge([
    PdfFile(File('first.pdf')),
    PdfFile(File('second.pdf')),
  ], output);
} finally {
  // whoever opens a file closes it
  await output.close();
}
await pdf.dispose();
```

That is the shape of every **edit**: an input in, a sink out. Watermark, compress, split and sign have the same shape with another verb. Reading has the other shape: open a document and query it, with no sink needed. See [Usage](#usage) for both.

### Cancellation

Is there a long job that the user no longer needs? You can stop it. Every operation is a **`PdfTask`**: an ordinary `Future` that you `await`, with a `cancel()` method added.

Keep the task in a variable. `await` it as usual, and call `cancel()` to stop it early, from a Cancel button or from your widget's `dispose()`.

```dart
// run it and await it, as in your normal code
final task = pdf.merge([PdfInput(firstPdf), PdfInput(secondPdf)], output);
try {
  await task;
  final merged = output.takeBytes(); // success
} on PdfCancelled {
  // cancelled: the Pdf and everything else keep working
}
```

```dart
// stop it, from a Cancel button or your widget's dispose()
task.cancel(); // the await above now throws PdfCancelled
```

Three rules keep you safe:

- **The `cancel()` call never throws.** It sends a stop request and returns. You need no `try/catch`. It is safe to call twice, and it does nothing after the task is done.
- **`PdfCancelled` appears only when you `await` the task.** You never get a half-finished result. That is the place for a `try/catch`.
- **If you never await the task, there is nothing to handle.** Cancel it and move on. There is no error and no crash.

To stop *everything*, call **`pdf.dispose()`**. It cancels every operation and returns at once. It does not wait for work in progress to end.

<details>
<summary><b>🧩 Advanced: when do I want more than one <code>Pdf</code>?</b></summary>

<br>

**Most apps need one `Pdf` instance.** Create it once and reuse it everywhere.

You want a second one when **two parts of your app must stop independently.** Say each screen runs its own PDF work. Give each screen its own `Pdf` and call `dispose()` on it when the user leaves. That cancels only the jobs of that screen and leaves the others running. (With one shared `Pdf`, `dispose()` stops everything, everywhere.)

A `Pdf` runs operations in parallel on executors that it leases from a shared pool. To change how many operations one instance runs at once, set `concurrency`:

```dart
final pdf = Pdf(config: PdfConfig(concurrency: 8)); // default: max(2, cores ÷ 2)
```

</details>

---

## Sources & sinks

Every operation reads from a **source** and writes to a **sink**. You have used the built-ins already: `MemorySource` and `MemorySink` for bytes, `FileSource` and `FileSink` for files. Write your own for anything else (a server, a database).

| Source | Where | Import |
|---|---|---|
| `MemorySource(bytes)` | every platform | `pdf_manipulator.dart` |
| `FileSource.open(file)`, `PdfFile(file)` | native | `io.dart` |
| `BlobSource(blob)` (a `File` from a picker is a `Blob`) | web | `web.dart` |

The rule for closing is the same for both: **whoever obtained it closes it.** A source or sink that you pass to an operation stays yours, and the library never closes it. A sink that a factory of yours returns (the `sinkFor` of `split`, `render` or `extractImages`) belongs to the library, and it closes that sink after its item, on success, error and cancel. A source that a `PdfInput.owning` opens, `PdfFile` for one, belongs to the job in the same way.

<details>
<summary><b>🧩 What are sources and sinks, really? (and writing your own)</b></summary>

<br>

A **source** is how `pdf_manipulator` reads your PDF. It never reads all of it at once, only small pieces:

```dart
abstract interface class DataSource {
  int get length; // how big are you?
  FutureOr<Uint8List> readAt(int offset, int count); // count bytes from offset
  FutureOr<void> close(); // release what you hold
}
```

The engine asks for *64 KB from here*, then the next piece, then the next. So you hand it a **reader**, not your whole file in one giant `Uint8List`. It jumps to any place in the file, so a one-way stream such as a live socket cannot be a source. Keep that data in memory or in a file first.

**Memory** is bounded for any number of pages; one page's text costs a known amount per piece. An operation holds a fixed ceiling of 16 MiB over what the process held before it, and reading the text of one page adds a declared number of bytes for each text piece (run) of that page, because reading order is a sort over all of a page's pieces and no text can leave before every piece is read. The numbers are declared in the engine (`piece_budget`) and held by its tests, so they cannot grow quietly.

A **sink** is the other side:

```dart
abstract interface class DataSink {
  FutureOr<void> write(Uint8List chunk); // a chunk of output
  FutureOr<void> abort(Object reason); // the operation was cancelled or failed: discard it all
  FutureOr<void> close(); // release what you hold
}
```

**Write your own** for any backing store. This is a PDF on a server, read over HTTP. It asks only for the byte ranges that the engine needs and never downloads the whole file:

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
  void close() {} // holds nothing to release
}

// then use it like any other source
final doc = await pdf.open(PdfInput(UrlSource(uri, contentLength)));
```

For a browser `Blob` from a file picker or a drag and drop, do not write one: `BlobSource` ships in `package:pdf_manipulator/web.dart` (web only, as `io.dart` is for native):

```dart
import 'package:pdf_manipulator/web.dart';

final doc = await pdf.open(PdfInput(BlobSource(file))); // file is a web.File
```

On web a source reaches the worker one of three ways, and the I/O mode's own read path serves only a source with none of them:

- **A `BlobSource`** goes to the worker by reference, with no copy. In JSPI, Atomics and Asyncify the worker reads slices of it in place with `FileReaderSync`: memory is O(1) for a file of any size and no read crosses to the main thread. In OPFS mode the worker copies the Blob to its OPFS file itself, from the Blob and without a main-thread read, as the pre-copy of any source in that mode (measured faster there than reading it slice by slice).
- **Any other source** that fits in what is left of the lane's 16 MiB transfer budget is read whole once on the main thread and copied into the worker, as a snapshot at job start. A merge of 100 small inputs transfers until the budget fills, and the rest take the mode's path.
- **A source past the budget** takes the mode's path: streamed through the page in JSPI, Atomics and Asyncify, and pre-copied to OPFS disk in OPFS mode.

A file that changes on disk while an operation reads it fails that operation with an I/O error. It never reads wrong bytes.

A custom `DataSink` works the same way. Implement `write(chunk)` for an upload stream, a database column or any other place for the bytes. Implement `abort(reason)` to take the bytes back (cancel the upload, delete the row) when the operation is cancelled or fails. Then a stopped operation leaves no partial output.

</details>

---

## Usage

Every call is a member of `Pdf`, and every call that opens a PDF takes a `PdfInput`. The calls come in four kinds. Pick the one that fits. This page shows the main ones. The [API reference](https://pub.dev/documentation/pdf_manipulator/latest/) has every member and signature.

| Kind | Calls | Who closes the document |
|---|---|---|
| A handle that you own | `open`, `edit`, `build` | You, with `dispose()` |
| A scope | `read`, `rewrite`, `buildTo` | The library, when your function returns |
| A one-shot: one call on one document | `readOnce`, `editOnce`, `info`, `resave` | The library, after the call |
| Several documents, or a policy | `merge`, `split`, `compress`, `imagesToPdf`, `sign`, `convertToOffice`, `convertFromOffice`, `convertXfaToAcroForm` | The library, after the call |

### Open a PDF

A `PdfInput` says where the bytes are and, when the file is encrypted, the password that opens it. The password goes with its source, and no other call names a password.

```dart
final plain = PdfInput(source); // you opened source, so you close it
final locked = PdfInput(source, password: 'secret');
final file = PdfFile(File('in.pdf')); // io.dart: the job opens and closes the file
final owned = PdfInput.owning(() => FileSource.open(File('in.pdf')));
```

A missing or wrong password is a `PdfPasswordRequired` or a `PdfWrongPassword` that reaches you through the task.

### One-shot operations

For a single change, call it on `pdf`, with an input in and a sink out:

```dart
await pdf.editOnce(input, output).addWatermark(
  'CONFIDENTIAL',
  style: PdfWatermarkStyle(opacity: 0.2, fontSize: 60),
);

await pdf.compress(input, output); // images stay lossless; unused objects are dropped
await pdf.compress(input, output, images: PdfImagePolicy.ebook); // images to 150 ppi

await pdf.resave(
  input,
  output,
  save: PdfSave.full(
    encryption: PdfEncryption.password(
      ownerPassword: 'secret',
      algorithm: PdfEncryptionAlgorithm.aesKey256,
    ),
  ),
);

final sinks = <MemorySink>[];
final parts = await pdf.split(input, (part) {
  final sink = MemorySink();
  sinks.add(sink);
  return sink;
}, by: PdfSplitRule.every(5)); // or .maxBytes(5000000), or .bookmarks()
```

`split` asks `sinkFor` for the sink of each part before it writes the part, and closes the sink after the part. It returns what each part holds: `index`, `start` and `end` page, and the `title` of the bookmark. `compress` never makes a file larger: if the result is not smaller, the output gets the bytes of the input as they are.

`pdf.merge` appends the whole of each input to the first one. To append only some pages, use `appendPages` in `rewrite`.

`editOnce` and `readOnce` return an object that runs one call. A second call on it throws a `StateError`. Use `rewrite` or `read` for several calls.

> Do you have several edits for the *same* PDF? Use `rewrite` (below). It parses the file once and saves once.

### Read a document

`pdf.read` opens the document, runs your function on it and closes it, also after an error or a cancel:

```dart
final hits = await pdf.read(input, (doc) async {
  print('${doc.pageCount} pages · encrypted: ${doc.encryption != null}');

  final text = await doc.text(); // the whole text as a string
  for (final field in await doc.formFields()) {
    print('${field.name}: ${field.kind.name} = ${field.value}');
  }

  return doc.search('revenue');
});
```

`pdf.open` gives you the document itself for as long as you want. You must dispose it, so use `try` and `finally`:

```dart
final doc = await pdf.open(input);
try {
  // The sink is yours, so the library never closes it.
  final text = MemorySink();
  await doc.extractText(text, pages: PdfPages.range(0, 10));

  // Each page goes to the sink that the function returns for it, row by row.
  // The library closes that sink. `size` is PdfRenderSize.dpi(150) by default.
  final pages = await doc.render(
    (page) => FileSink.open(File('page-${page.page}.png')),
    size: PdfRenderSize.fit(maxWidth: 200, maxHeight: 200),
  );
  // pages[i].width and pages[i].height: the pixel size of each file written
} finally {
  await doc.dispose();
}
```

The facts that the open already read are fields of the document: `pageCount`, `version`, `isTagged`, `metadata` (title, author, subject, keywords, creator, producer and the dates) and `encryption` (the algorithm, the permissions and whether a password is needed; null when the document is not encrypted). Everything per page is a read of its own: `pageInfo`, `text` and `extractText`, `search`, `render` and `bitmap`, `extractImages`, `classifyPages` and `classifyDocument`, `bookmarkRanges`, `annotations`, `signatures` and `verifySignatures` (the digest and the signer key; the trust of the certificate is not checked), `validatePdfA` and `validatePdfUa`, `formFields`, `exportFormData`, `xfa`, and `attachments` with `extractAttachment`.

A page selection is a `PdfPages`. A page range ends exclusive: `PdfPages.range(0, 10)` is pages 0 to 9, as in `List.sublist` and in Effective Dart's rule that a range has an inclusive start and an exclusive end. Other tools differ: PyMuPDF's `insert_pdf`, for one, includes both ends.

### Edit a document

`pdf.rewrite` is for *many* changes to one PDF. It parses once, applies each edit in memory and writes once when your function returns. If your function throws, nothing is written:

```dart
await pdf.rewrite(input, output, (editor) async {
  await editor.setMetadata(const PdfMetadata(title: 'Q4 Report'));
  await editor.appendPages(PdfInput(appendix));
  await editor.removePages(PdfPages.single(4));
  await editor.addWatermark(
    'FINAL',
    pages: PdfPages.single(0),
    style: PdfWatermarkStyle(opacity: 0.15),
  );
  final report = await editor.reduceImages(PdfImagePolicy.ebook);
  print('${report.changed} images, ${report.bytesBefore} → ${report.bytesAfter} bytes');
});
```

Use `pdf.edit` when you must keep the editor open. It returns a handle that you save and dispose yourself:

```dart
final editor = await pdf.edit(input);
try {
  await editor.rotatePages(by: PdfRotation.r90);
  await editor.save(output); // see the save options below
} finally {
  await editor.dispose();
}
```

The edits are the members of `PdfEdits`: `setMetadata`, `rotatePages` and `setRotation`, `removePages`, `keepPages`, `reorderPages`, `movePage`, `addWatermark`, `addStamp` and `addImageStamp`, `addAttachment`, `cropMargins`, `setMediaBox` and `setCropBox`, `setImageBounds`, `flattenForms` and `flattenAnnotations`, `redact`, `reduceImages`, `unembedStandardFonts`, `convertToPdfA` and `sanitize`. An editor adds `appendPages`, `pageImages`, `pageInfo`, `metadata`, `pageCount`, the form fields below and the queued redactions further down.

An edit that takes pages checks all of them before it changes one. A page that the document does not have is refused through the task, with a `PdfPageOutOfRange` for most edits, and the document stays as it was. A bad page set, such as an empty list or a page listed twice, never gets that far: `PdfPages.list` throws an `ArgumentError` when you build it.

`keepPages` keeps the pages you name in the order of the document. To put pages in an order of your own, use `reorderPages` with a `PdfPageOrder`:

```dart
await editor.keepPages(PdfPages.range(0, 3));
await editor.reorderPages(PdfPageOrder([2, 0, 1]));
```

A form field is edited through `editor.field(name)`:

```dart
final f = editor.field('country');
await f.setValue(const PdfFormFieldValue.choice('DE')); // a combo box or list box
await editor.field('tags').setValue(const PdfFormFieldValue.choices(['a', 'c'])); // a multi-select list box
await editor.field('name').setValue(const PdfFormFieldValue.text('Ada'));
await editor.field('agree').setValue(const PdfFormFieldValue.checked(true));
await editor.field('name').setValue(const PdfFormFieldValue.none()); // clear it
```

The other edits of a field are `setReadOnly`, `setRequired`, `setFlags`, `setTooltip`, `setBounds`, `setMaxLength`, `setAlignment`, `setBackgroundColor`, `setBorderColor`, `setBorderWidth`, `setAppearance` and `remove`. `setTooltip`, `setMaxLength`, `setBackgroundColor` and `setBorderColor` take null to remove what they set. A choice that is not an option of the field (an editable combo box takes any text) throws `PdfEngineFailed` and changes nothing.

Save options, a `PdfSave`:

- `PdfSave.full()` is the default. It writes the file again, deflates the streams and drops unused objects. Its fields are `deflate`, `garbageCollect`, `useObjectStreams`, `mergeDuplicates`, `pruneResources` and `encryption`.
- `PdfSave.full(encryption: ...)` encrypts on save. `PdfEncryption.remove()` strips the encryption, and `PdfEncryption.keep()` is the default.
- `PdfSave.incremental()` keeps the original bytes and appends the changes, so an existing signature stays valid. An edit that an append cannot carry (a removed page, a redaction, a content edit) throws `PdfIncrementalRefused` and writes nothing.
- `PdfSave.incrementalOrRewrite()` appends when it can and writes the file again when it cannot. The rewrite breaks a signature, as any full save does.

Pass it to `editor.save(output, save: ...)`, `pdf.rewrite(..., save: ...)`, `pdf.editOnce(..., save: ...)`, `pdf.resave` or `pdf.merge`.

Redaction removes the content. It does not only hide it. `redact` marks and applies the regions you give it in one step. `markRedaction` queues marks, and `applyRedactions` applies the queued ones:

```dart
await pdf.rewrite(input, output, (editor) async {
  final once = await editor.redact(0, [PdfRect(x: 72, y: 700, width: 200, height: 20)]);

  await editor.markRedaction(0, [PdfRect(x: 72, y: 650, width: 200, height: 20)]);
  print(await editor.redactionCount(0)); // pending marks on this page
  final report = await editor.applyRedactions(); // gone for good
  print('${report.glyphsRemoved} glyphs and ${report.imagesRemoved} images removed');
  await editor.clearRedactions(); // or drop every queued mark instead
});
```

`redact` applies only its own regions. Marks queued earlier stay queued, and a later `applyRedactions` keeps what `redact` removed. `overlay:` is what is drawn over a region, a black box by default: `PdfRedactionOverlay.fill(PdfColor.red)` for another colour, or `PdfRedactionOverlay.none()` for no box. An image under a region keeps a black patch where its pixels were removed, whatever the overlay.

### Build from scratch

`pdf.buildTo` gives your function an empty document. Add pages, then content. The library saves the document when your function returns:

```dart
await pdf.buildTo(output, (builder) async {
  await builder.setMetadata(const PdfMetadata(title: 'Invoice #1042'));

  final page = await builder.addPage();
  await page.heading(1, 'Invoice');
  await page.paragraph('Thank you for your purchase.');
  await page.text('Paid on time.', style: const PdfTextStyle(bold: true));
  await page.text(
    'example.com',
    link: const PdfLink.url('https://example.com'),
  );
  await page.textField('notes', PdfRect(x: 50, y: 400, width: 300, height: 100));
});
```

Use `pdf.build` when you must keep the builder open. It returns a handle with `save(output)` and `dispose()`.

Pages: `addPage()` (A4), `addPage(PdfPageSize.letter())`, `addPage(PdfPageSize.points(400, 300))`, and `addImagePage` for a page that holds one picture. That page has the size of the picture, one point per pixel, unless you name one. Photos become a PDF with `pdf.imagesToPdf([a, b], output)`.

Content: `text` and `appendText` (with a `PdfTextStyle` for the font, size, weight, slant and colour, and a `PdfLink` or `PdfMarkup` for the text), `heading`, `paragraph`, `space`, `newline`, `horizontalRule`, `textIn`, `image`, `rect` (filled, stroked or both), `line`, `table`, `stickyNote`, `freeText`, `stamp`, `columns`, `footnote` and `watermark`. Form fields: `textField`, `checkbox`, `radioGroup`, `comboBox`, `pushButton` and `signatureField`, each with `PdfFieldScripts` for Acrobat JavaScript actions. Page actions: `onOpen` and `onClose`. A position is a `PdfPoint`.

```dart
await page.rect(
  const PdfRect(x: 50, y: 300, width: 200, height: 80),
  fill: PdfColor.red,
  stroke: const PdfStroke(color: PdfColor.black, width: 2),
);
await page.line(const PdfPoint(50, 300), const PdfPoint(250, 380),
    stroke: const PdfStroke(dash: [3, 2]));
await page.text(
  'Important',
  markup: const PdfMarkup(PdfMarkupKind.highlight),
);
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

Filling a form with text that the font of the field cannot draw (Japanese, Korean, Chinese, emoji) needs a fallback font. The binary does not include one, because that is many MB that most apps never use. You register your own once, and every later fill uses it.

**1. Download a font.** Any complete `.ttf` or `.otf` file works:

- For Chinese, Japanese or Korean, pick the Noto Sans font for your language: [Noto Sans SC](https://fonts.google.com/noto/specimen/Noto+Sans+SC) (Simplified Chinese), [Noto Sans TC](https://fonts.google.com/noto/specimen/Noto+Sans+TC) (Traditional Chinese), [Noto Sans JP](https://fonts.google.com/noto/specimen/Noto+Sans+JP) (Japanese) or [Noto Sans KR](https://fonts.google.com/noto/specimen/Noto+Sans+KR) (Korean). If your forms mix several of these languages, [notofonts/noto-cjk](https://github.com/notofonts/noto-cjk) has combined files that cover all of them. The file is bigger, but you register it once.
- For emoji, use [Noto Emoji](https://fonts.google.com/noto/specimen/Noto+Emoji). Use this black-and-white font: baked emoji come out as black-and-white shapes, like printed text. Colour is not possible here. PDF text is drawn from character shapes in one colour, and no PDF tool can bake a colour emoji font into text. (The stored value keeps the real emoji character either way.)

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

The font stays in memory after you register it. That is why every later fill is fast. Each executor holds its own copy of the font, and the font is registered again whenever a `Pdf` leases an executor. So when one language is enough, use the single-language file and not the combined one.

Without a registered font, the fill still succeeds. The value is stored correctly, and readers with their own fonts show it. Only the flattened appearance falls back to the font of the field.

---

## Error handling

Every runtime failure is a typed subclass of `PdfError`, and it reaches you through the task: no string matching and no `PlatformException`. Catch the cases that you handle in a special way, and let the rest fall to a catch-all. Each error has a `message` that a person can read.

```dart
try {
  await pdf.read(PdfInput(source), (doc) async {
    // ... use the document
  });
} on PdfPasswordRequired {
  // encrypted: open it again with PdfInput(source, password: '...')
} on PdfWrongPassword {
  // wrong password: ask the user again
} on PdfCorrupted catch (e) {
  print('Not a valid PDF: ${e.message}');
} on PdfError catch (e) {
  // anything else: I/O failure, unsupported feature, page out of range...
  print(e.message);
}
```

`PdfError` is sealed, so you can also `catch (e)` once and `switch` over it. The compiler then flags each case that you did not handle. `PdfCancelled` is part of the same hierarchy (the one that [Cancellation](#cancellation) throws).

A mistake in your own code is not a `PdfError`. An empty page list, a negative page, an empty `merge` list or a second call on a one-shot throws an `ArgumentError` or a `StateError` at the call, before any work starts. Fix the code. Do not catch it.

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

Internally the modes differ in *how* the WASM engine gets your bytes: the first three stream them on demand, while the fallback copies the whole file into private browser storage first (works everywhere, just a slower first byte).

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

The trade-off, plainly: the Asyncify module is about a third larger than the normal one (the full engine: 28.2 MB against 20.9 MB, 10.2 MB against 8.1 MB gzipped), because most of the engine can be in the middle of a read when it stops. Every module ships as parts of 16 MiB, so neither reaches the 25 MiB per-file limit of Cloudflare Pages and Workers static assets. The instrumented engine also runs slower, and every read is a round trip to the page: on a 300 MB text PDF, opening and extracting 50 pages took 93 s with Asyncify against 41 s with OPFS (the copy included), and merging two such files 16–19 s against 9–10 s. Turn it on when streaming matters more than those costs, and measure your own files.

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
| [Architecture](docs/ARCHITECTURE.md) | How it's built: layers, streaming I/O, three web modes |
| [Capabilities](docs/CAPABILITY_ROADMAP.md) | What's shipped, what's planned |
| [Updating](docs/UPDATING.md) | Maintaining the vendored Rust engine |
| [Migration](docs/MIGRATION.md) | Upgrading between versions |
| [Contributing](CONTRIBUTING.md) | Setup, PR workflow, adding operations |

---

## License

MIT. See [LICENSE](LICENSE).
