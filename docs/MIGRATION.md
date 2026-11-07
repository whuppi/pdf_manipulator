<!--
═══════════════════════════════════════════════════════════════════════
MIGRATION GUIDE STANDARD — read before editing this file.
═══════════════════════════════════════════════════════════════════════
One guide for every version a user can be on. A user finds their version
in the table at the top and reads the sections it lists, in order. No
user is ever sent to the changelog alone to migrate.

WHEN A SECTION IS ADDED
  Every release whose entry holds a **Breaking:** bullet gets one
  section, written in the same PR as the entry:
    ## From <previous> to <X.Y.Z>
  <previous> is the newest stable release in CHANGELOG.md below X.Y.Z;
  a retracted release is skipped (3.0.0 covers the retracted 2.2.0).
  A release with no Breaking bullet gets no section: the top table says
  so ("a release that is not listed broke nothing").
  A prerelease (X.Y.Z-dev.N) migrates under the X.Y.Z heading it ships
  as; while dev is ahead of prod, the section already exists on dev.

THE PATHS
  • The chain: each section goes from the previous breaking release to
    the next, so a user on any 1.x or later reads every section from
    theirs upward. This is how Flutter, React and go_router do it.
  • The one direct jump: From 0.5.9 to 6.0.0. 0.x is the Android-only
    plugin and 1.0.0 a rewrite of it, so a 0.x user goes straight to the
    current API instead of learning five dead ones. It stays frozen at
    6.0.0; a 0.x user reads it, then the chain from 6.0.0 upward.
  • A new major that is another rewrite may get its own direct jump the
    same way. That is a decision, recorded before the section is written.

SECTION ORDER
  The top table first, then the chain newest first, then the direct
  jump(s) last. When a section is added, its row goes into the top
  table in the same edit, and every row below it gains "then each one
  above" if it lacked it.

SECTION SHAPE
  ## From A to B
  <one or two sentences: what kind of change this release is (an API
   redesign, a build-config change, web files only) and who it touches>
  <a numbered summary of the ideas that changed, when there are three
   or more, each linking its subsection>
  <the free names the snippets assume, one sentence, as the wall reads
   them: "The snippets assume a `pdf`, `source`, ...">
  ### <one subsection per idea or per old call>
  <entries>
  ### Renamed  /  ### Removed  /  ### Types   (only when the release has
   more renames or removals than entries can carry; tables, grouped by
   area, at most ~15 rows each, two columns: old | new)

ENTRY SHAPE — every Breaking bullet has at least one entry
  A bold line that starts with the old name in backticks, an em dash,
  then the explanation: what changed and what to write instead. Then:
    Before:   (its own paragraph: a blank line above it)
    a fenced block in the OLD version's code
    After:    (its own paragraph)
    a fenced block in the NEW version's code
  • The first backticked name of each Breaking bullet must appear in
    the section in backticks (`tool/check_migration.sh` checks it).
  • Before and After are complete statements a user can paste, using
    only the free names the section's intro promises.
  • Dart blocks are compiled. A pubspec, a shell command or a file list
    goes in a yaml, sh or text block: shown, never compiled.
  • A mapping of three or more fields (permissions, watermark options)
    is a two-column table under the After block, not a long sentence.
  • Page numbers, defaults that flipped, and anything that changes
    behaviour without a compile error (a default policy, a permission
    that starts true) are said in words: the compiler will not warn.

THE WALLS — both run in `make test-guards`
  • tool/check_migration.sh: the newest entry of either changelog with a
    Breaking bullet has its "From <previous> to <X.Y.Z>" section, and
    that section names every Breaking bullet's first backticked name.
    Every ## heading here is a "From X to Y" naming a version a
    changelog holds.
  • tool/check_migration_snippets.sh: every Before block compiles
    against version A from pub.dev and every After block against
    version B (this checkout when B is the version being built). A new
    version in a heading needs a row in its config() table: imports,
    free names, and whether resolving it needs Flutter.
  Fix the snippet, never the wall. A snippet that cannot compile is
  wrong documentation.

STYLE
  Present tense, second person, plain global English; no history of
  why a change was made (that is the PR), only what to change. Render
  the file as GitHub shows it
  before merging: tables must not wrap into walls, labels must sit on
  their own line.
═══════════════════════════════════════════════════════════════════════
-->

# Migrating pdf_manipulator

| You are on | Read, in this order |
|:---|:---|
| 5.x | [5.0.1 → 6.0.0](#from-501-to-600) |
| 4.x | [4.2.0 → 5.0.0](#from-420-to-500), then 5.0.1 → 6.0.0 |
| 3.x | [3.0.0 → 4.0.0](#from-300-to-400), then each one above |
| 2.x | [2.1.4 → 3.0.0](#from-214-to-300), then each one above |
| 1.x | [1.0.6 → 2.0.0](#from-106-to-200), then each one above |
| 0.x, the old Android-only plugin | [0.5.9 → 6.0.0](#from-059-to-600) alone: it is a rewrite, so it goes straight to 6.0.0 |

Each section covers one release that broke something, newest first; a release that is not listed broke nothing. Every entry has a Before and an After. A wall compiles each Before block against the old version from pub.dev and each After block against the new one, so the code here is code that compiles.

## From 5.0.1 to 6.0.0

6.0.0 changes how you call the library, not what it can do. Five ideas changed shape:

1. [A document to open](#a-document-to-open) is a `PdfInput`: the source and its password together.
2. [A page selection](#a-page-selection) is a `PdfPages`, checked when you build it, with ranges that end before `end`.
3. [The doors](#the-doors): a handle you close, a function the library closes, or one call. The one-shot helpers (`pdf.watermark`, `pdf.deletePages`, ...) are gone; `pdf.editOnce` has every edit.
4. [Sinks and ownership](#sinks-and-ownership): whoever opened a source or sink closes it, and a failed job calls `abort` on its sink.
5. [Errors](#errors): a runtime failure is a `PdfError`; a mistake in your code is an `ArgumentError` or a `StateError`.

How to migrate: update the dependency, run `dart analyze`, and fix each error with the entry for its idea below. A name the analyzer cannot find is in [Renamed](#renamed) or [Removed](#removed).

The snippets assume a `pdf`, `source`, `output`, `doc`, `editor`, `builder`, `page`, `rect`, `bytes`, `sources`, `images` and `kind` of the types in the Pdf API.

### A document to open

**`open`** — Every call that opens a PDF takes a `PdfInput`: the source and, when the file is encrypted, its password. The password goes with its source, and no other call names a password. `PdfFile(file)` and `PdfInput.owning` open the source for the job and close it afterwards.

Before:
```dart
final doc2 = await pdf.open(source, password: 'secret');
final text = await doc2.extract(pages: const PdfPages.all());
await doc2.dispose();
await pdf.merge([source, source], output);
```
After:
```dart
final locked = PdfInput(source, password: 'secret');
final text = await pdf.read(locked, (doc) => doc.text());
await pdf.merge([locked, PdfInput(source)], output);
```

### A page selection

**`PdfPages`** — A page set is a `PdfPages`, and it is valid when you build it: `PdfPages.list` sorts the pages and throws `ArgumentError` for an empty list, a negative page and a page given twice. A range ends before its end: `PdfPages.range(0, 4)` is pages 0 to 3, as in `List.sublist`. When the order matters, `PdfPageOrder` keeps the order you give; `keepPages` keeps pages in document order.

Before:
```dart
await editor.deletePage(2);
await editor.selectPages([2, 0, 1]);
await editor.rotatePage(0, degrees: 90);
```
After:
```dart
await editor.removePages(PdfPages.single(2));
await editor.reorderPages(PdfPageOrder([2, 0, 1]));
await editor.rotatePages(by: PdfRotation.r90, pages: PdfPages.single(0));
```

### The doors

**`edit`** — `pdf.open`, `pdf.edit` and `pdf.build` return a handle that you own, and only a handle has `dispose` and `save`. Three more kinds of call close the document for you: the scopes `read`, `rewrite` and `buildTo`, which run your function; the one-shots `readOnce`, `editOnce`, `info` and `resave`, which run one call; and the calls over several documents, such as `merge`, `split` and `compress`. A one-shot runs one call, and a second call throws `StateError`.

Before:
```dart
final copy = await pdf.edit(source);
await copy.addWatermark(-1, 'DRAFT');
await copy.deletePage(0);
await copy.save(output);
await copy.dispose();
```
After:
```dart
await pdf.rewrite(PdfInput(source), output, (e) async {
  await e.addWatermark('DRAFT');
  await e.removePages(PdfPages.single(0));
});
await pdf.editOnce(PdfInput(source), output).rotatePages(by: PdfRotation.r90);
```

### Sinks and ownership

**`DataSink`** — Whoever obtained a source or a sink closes it, so `DataSource` and `DataSink` have a `close()`. The library never closes one you pass; it closes the sink that a `sinkFor` of yours returns. `FileSource` and `FileSink` open through `await FileSource.open(file)` and `await FileSink.open(file)`. A sink of your own implements `abort(Object reason)`, which the library calls when the operation is cancelled or fails, so you no longer delete the half-written file.

Before:
```dart
final from = FileSource(File('in.pdf'));
final result = await FileSink.create(File('out.pdf'));
try {
  await pdf.compress(from, result);
} catch (_) {
  await result.close();
  await File('out.pdf').delete();
  rethrow;
}
await result.close();
```
After:
```dart
final result = await FileSink.open(File('out.pdf'));
try {
  await pdf.compress(PdfFile(File('in.pdf')), result);
} finally {
  await result.close();
}
```

### Errors

**`PdfError`** — A `PdfError` is a runtime failure and reaches you through the task; a `switch` over it is exhaustive. A mistake in your own code is not a `PdfError`: it throws `ArgumentError` or `StateError` before any I/O. `PdfInvalidArgument` is gone, and an engine refusal is `PdfEngineFailed`.

Before:
```dart
try {
  await editor.deletePage(99);
} on PdfPageRangeError catch (e) {
  print('${e.page} of ${e.pageCount}');
} on PdfEngineError catch (e) {
  print(e.message);
}
```
After:
```dart
try {
  await editor.removePages(PdfPages.single(99));
} on PdfPageOutOfRange catch (e) {
  print('${e.page} of ${e.pageCount}');
} on PdfEngineFailed catch (e) {
  print(e.message);
}
```

### Renamed

Each table holds one area. The left column is the 5.0.1 name, the right column what to write in 6.0.0.

#### Opening, sources and the instance

| 5.0.1 | 6.0.0 |
|:---|:---|
| `FileSource`, `FileSink.create` | `await FileSource.open(file)`, `await FileSink.open(file)` |
| `PdfDoc`, `PdfSugar`, `PdfStandalone` | `PdfDocument` and `PdfDocumentHandle`; the calls are members of `Pdf` |
| `PdfConfig(webLaneWorkerUrl:, maxLanes:)` | `PdfConfig(webWorkerUrl:, concurrency:)` |

#### Reading a document

| 5.0.1 | 6.0.0 |
|:---|:---|
| `title`, `author`, `subject`, `keywords`, `creator`, `producer`, `creationDate` of a document | `doc.metadata.title` and the rest; `creationDate` is a UTC `DateTime?` |
| `isEncrypted`, `requiresPassword`, `encryptionAlgorithm`, `permissions` | `doc.encryption`, which is null when the document is not encrypted |
| `extract({pages, format})` | `text({pages, format})`, or `extractText(output, {pages, format})` |
| `PdfExtractionFormat.plainText`, `.text` | `plain`, `layout` |
| `search(query:, pages:)`, `SearchResult` | `search(query, {pages})`, `PdfSearchHit` |
| `render`, `RenderedPage`, `extractImages`, `PdfImage` | `render(sinkFor)`, `PdfRenderedPage`, `extractImages(sinkFor)`, `PdfImageInfo` |
| `PdfRenderSize(maxWidth:, maxHeight:)`, `thumbnail(n)` | `PdfRenderSize.fit(maxWidth:, maxHeight:)`, `.dpi(n)` |
| `signatures`, `formFields`, `xfa`, `attachments` (getters) | the same names as methods |
| `validatePdfA`, `validatePdfUa`, `PdfValidationResult` | `PdfALevel` and `PdfUaLevel` levels, `PdfValidationReport` with `isCompliant` |
| `planSplitByBookmarks`, `PdfBookmarkSplit` | `bookmarkRanges`, `PdfBookmarkRange` with `start` and `end` |
| `classifyPage`, `PdfDocumentClassification` | `classifyPages({pages})`, `PdfDocumentKind` |
| `PdfPageInfo` with `index`, `rotation` as `int` | `page`, `mediaBox`, `cropBox`, `rotation` as `PdfRotation` |
| `PdfFormField` with `type`, `readOnly`, `required`, `PdfFormFieldType` | `kind`, `isReadOnly`, `isRequired`, `PdfFormFieldKind` |
| `PdfCheckedValue.checked` | `isChecked` |
| `size` of `PdfAttachment`, `serial`, `regions` | `sizeBytes`, `serialNumber`, `regionCount` |
| `PdfPermissions` with `print`, `modify`, `copy` and the rest | `canPrint`, `canModify`, `canCopy` and the rest |

#### Editing

| 5.0.1 | 6.0.0 |
|:---|:---|
| `PdfEditor.pageCount`, `version`, `isModified` (getters) | methods |
| `setTitle`, `setAuthor`, `setSubject`, `setKeywords`, `setProducer`, `setCreationDate` | `setMetadata(PdfMetadata)`, and `metadata()` to read |
| `rotatePage`, `rotateAllPages`, `setPageRotation` | `rotatePages(by:, pages:)`, `setRotation(rotation, pages:)` |
| `pageMediaBox`, `pageCropBox`, `setPageMediaBox`, `setPageCropBox` | `pageInfo`, `setMediaBox(box, {pages})`, `setCropBox(box, {pages})` |
| `deletePage`, `selectPages` | `removePages`, `keepPages` or `reorderPages` |
| `mergeFrom(source, {pages})` | `appendPages(PdfInput, {pages})` |
| `addWatermark(page, text)`, `addStamp(page, type:, rect:)`, `PdfStampType` | `addWatermark(text, {pages})`, `addStamp(stamp, bounds:, pages:)`, `PdfStamp` |
| `rotation` of `PdfWatermarkStyle` | `angleDegrees` |
| `embedFile`, `addRedaction` | `addAttachment`, `markRedaction(page, regions)` |
| `flattenForms({int? page})`, `flattenAnnotations` | `pages:` takes a `PdfPages` |
| `convertToPdfA` (level `int`, default 1) | `convertToPdfA` with a `PdfALevel`, default `a2` |
| `monoPpi`, `minSavings` of `PdfImagePolicy`; `bits`, `indexed`, `ppiMin`, `uses` of `PdfImageOutcome` | `bilevelPpi`, `minSavingsRatio`, `bitsPerComponent`, `isIndexed`, `minPpi`, `placementCount` |

#### Saving and encryption

| 5.0.1 | 6.0.0 |
|:---|:---|
| `PdfSaveOptions`, `PdfSaveOptions.fullRewrite(compress:)`, `PdfEncryption.config` | `PdfSave`, `PdfSave.full(deflate:)`, `PdfEncryption.password` |
| `PdfSaveOptions` with `incremental()` | `PdfSave.incremental()` appends and throws `PdfIncrementalRefused` when it cannot; `PdfSave.incrementalOrRewrite()` falls back to a full save |
| `PdfEncryptionAlgorithm.rc4_40`, `rc4_128`, `aes128`, `aes256` | `rc4Key40`, `rc4Key128`, `aesKey128`, `aesKey256` |
| `PdfEncryptionKeep`, `PdfPagesAll`, `PdfWatermarkCenter` and the other subclasses | not exported; call `PdfEncryption.keep()`, `PdfPages.all()`, the factories |

#### Building

| 5.0.1 | 6.0.0 |
|:---|:---|
| `addA4Page`, `addLetterPage`, `addPage(width:, height:)` | `addPage([PdfPageSize])` |
| `textField` (`defaultValue`), `checkbox` (`checked`), `PdfTextAlignment` | `value`, `isChecked`, `PdfAlignment` |
| `font`, `at` of a page | `setFont`, `moveTo(PdfPoint)` |

#### Conversions, values and errors

| 5.0.1 | 6.0.0 |
|:---|:---|
| `convertTo`, `convertToPdf`, `PdfDocumentFormat` | `convertToOffice`, `convertFromOffice`, `PdfOfficeFormat` |
| `PdfIoError`, `PdfEngineError`, `PdfPageRangeError`, `PdfCryptoError` | `PdfIoFailed`, `PdfEngineFailed`, `PdfPageOutOfRange`, `PdfCryptoFailed` |
| `bottom` of a `PdfRect` (`y + height`) | `top`; `bottom` is now `y` |
| `PdfColor` and every result and options type | `final` with value `==`; wrap one, do not extend it |
| `PdfSignatureInfo` | needs `coversWholeDocument` when you build one |
| `PdfCheckedValue` for a radio group, `PdfTextValue` for a combo box | `PdfChoiceValue` (`PdfMultiChoiceValue` for several options), or `PdfNoValue` |

### Removed

| Removed in 6.0.0 | Use instead |
|:---|:---|
| `pdf_oxide_bg.wasm` and `pdf_oxide_asyncify_bg.wasm` in a precache list, a hosting rule or a deploy script | the parts `pdf_oxide_bg.<n>.wasm` (and, with `asyncify: true`, `pdf_oxide_asyncify_bg.<n>.wasm`) that `pdf_oxide_manifest.json` lists. Run setup again, delete the old whole files from `web/pdf_manipulator/`, and serve `.wasm` as `application/wasm` |
| `compress(garbageCollect:)` | nothing: it always collects garbage. Its `images` default is now `PdfImagePolicy.lossless`; pass `PdfImagePolicy.ebook` for the old result |
| `imagesToPdf` on A4 pages | each page has its image's size; pass `page: PdfImagePage(size: PdfImagePageSize.fixed(PdfPageSize.a4()))` for A4 |
| `split`, `splitBySize`, `splitByBookmarks` | one `split(input, sinkFor, by: PdfSplitRule.every(n))`, or `.maxBytes(n)`, or `.bookmarks()` |
| `deletePages`, `reorderPages`, `movePage`, `rotatePages`, `rotateAllPages`, `flattenForms`, `embedFile`, `watermark`, `addStamp`, `addImageStamp`, `convertToPdfA` as one-shots | `pdf.editOnce(input, output)` and the edit |
| `applyRedactions` as a one-shot | `applyRedactions()` in a `pdf.rewrite` body |
| `eraseRegions`, `clearEraseRegions` | `redact(page, regions, overlay: const PdfRedactionOverlay.none())`, `clearRedactions()` |
| `encrypt`, `decrypt` | `pdf.resave(input, output, save: PdfSave.full(encryption: ...))` with `PdfEncryption.password` or `.remove()` |
| `extractPages` | `keepPages(PdfPages.list([0, 2]))`, or `reorderPages(PdfPageOrder([2, 0]))`, on `pdf.editOnce` |
| `registerFallbackFont` with bytes | the font is a `DataSource`: wrap bytes in `MemorySource(bytes)` |
| `PdfDoc.pages` | `await doc.pageInfo()` |
| `PdfDoc.formField(name)` | find the field in `await doc.formFields()` |
| `setFormFieldValue`, `setCheckboxFieldValue`, `removeFormField` and the other `setFormField…` | `editor.field(name).setValue(...)` and its siblings |
| `resizeImage`, `repositionImage` | `setImageBounds(page, name, bounds)` |
| `scrubMetadata`, `PdfSanitizeOptions` | `sanitize(stripMetadata:, stripJavaScript:, stripAttachments:)` |
| `PdfInvalidArgument` | an `ArgumentError` or a `StateError` for your own mistake, `PdfEngineFailed` for an engine refusal |
| `PdfSearchError`, `PdfExtractionFailed` | nothing threw them; delete their `catch` clauses |
| `fieldKeystroke`, `fieldFormat`, `fieldValidate`, `fieldCalculate`, `linkUrl`, `linkPage` | the `scripts:` and `link:` arguments of the call that writes the field or text |
| `done`, `newPageSameSize` | nothing: a page ends when the next is added or the document is saved |
| `toBits` of `PdfPermissions` | nothing: the library encodes the flags itself |

## From 4.2.0 to 5.0.0

5.0.0 renamed the editor's reads and changed how images are reduced. Any 4.x release migrates the same way: nothing broke between 4.0.0 and 4.2.0.

The snippets assume a `pdf`, `source`, `output`, `doc`, `editor` and `bytes` of the types in the Pdf API.

### Image reduction

**`optimizeImages`** — The quality and size knobs are a `PdfImagePolicy`. `reduceImages` returns a report; the count `optimizeImages` returned is `report.changed`.

Before:
```dart
final changed = await editor.optimizeImages(quality: 70, minSize: 128);
```

After:
```dart
final report = await editor.reduceImages(
  const PdfImagePolicy(jpegQuality: 70, minPixels: 128, convertCmykToRgb: true),
);
final changed = report.changed;
```

**`compress(imageQuality:)`** — `compress` takes a `PdfImagePolicy` too. Its default is now `PdfImagePolicy.ebook`, which also lowers images drawn above 225 ppi to 150 ppi. For the old behaviour, pass only the quality.

Before:
```dart
await pdf.compress(source, output, imageQuality: 70);
```

After:
```dart
await pdf.compress(source, output, images: const PdfImagePolicy(jpegQuality: 70));
```

### Reads are getters

**`getTitle`** — A read with no argument is a getter: `getTitle()` is `title`, and the same for `getAuthor`, `getSubject`, `getKeywords`, `getProducer`, `getCreationDate` and the document's `getSignatures`. The one read with an argument drops the `get`: `getPageMediaBox(page)` is `pageMediaBox(page)`.

Before:
```dart
final title = await editor.getTitle();
final box = await editor.getPageMediaBox(0);
final signed = await doc.getSignatures();
```

After:
```dart
final title = await editor.title;
final box = await editor.pageMediaBox(0);
final signed = await doc.signatures;
```

### Flattening

**`flattenAllAnnotations`** — It is `flattenAnnotations`, and both flattens take an optional `page`. Without it they flatten every page, as before.

Before:
```dart
await editor.flattenAllAnnotations();
await editor.flattenForms();
```

After:
```dart
await editor.flattenAnnotations();
await editor.flattenForms(page: 0); // or no page, for every page
```

### Redaction

**`applyRedactions`** — It returns a `PdfRedactionReport` instead of nothing. Awaiting it still applies the redactions, so code that ignores the result needs no change.

Before:
```dart
await editor.applyRedactions();
```

After:
```dart
final report = await editor.applyRedactions();
print('${report.glyphsRemoved} glyphs removed');
```

### New optional arguments

**`mergeFrom`** — `mergeFrom` gains `pages:` and `embedFile` gains `description:`, `mimeType:` and `relationship:`. A call without them works as before.

Before:
```dart
await editor.mergeFrom(source);
await editor.embedFile('notes.txt', source);
```

After:
```dart
await editor.mergeFrom(source, pages: [0, 2]); // 0-based; no pages merges all
await editor.embedFile('notes.txt', source, mimeType: 'text/plain');
```

## From 3.0.0 to 4.0.0

4.0.0 changed only the build configuration in your app's pubspec. No Dart code changes.

### The engine configuration

**`keep`** — Three flat keys under `hooks: user_defines: pdf_manipulator:` replace the `trim` block: `keep` says which capabilities go in, `detector` how `keep: auto` finds them, and `build` what the engine is optimized for. An invalid configuration now fails the build with a message. After the change, run `flutter pub run pdf_manipulator:setup` again for the web, without `--trim`.

Before:
```yaml
hooks:
  user_defines:
    pdf_manipulator:
      trim: auto
      trim-detector: scan
```

After:
```yaml
hooks:
  user_defines:
    pdf_manipulator:
      keep: auto
      detector: scan   # only with keep: auto; scan is the default, so this line can go
```

A list form moves the same way: `trim: {keep: [render]}` is `keep: [render]`.

## From 2.1.4 to 3.0.0

3.0.0 stopped bundling a 4.4 MB fallback font. 2.2.0 made this change first and was retracted; 3.0.0 is the release that carries it.

The snippets assume a `pdf`, `source`, `output`, `doc`, `editor` and `bytes` of the types in the Pdf API.

### CJK and emoji in form fields

**`registerFallbackFont`** — Flattening a form value that the field's font cannot draw (Chinese, Japanese, Korean, emoji) needs a font you register once. Without one the value is still saved; only its flattened look falls back to the field's font. The README's "CJK & emoji in form fields" says which font to pick.

Before:
```dart
await editor.flattenForms(); // a CJK value drew with the bundled font
```

After:
```dart
await pdf.registerFallbackFont(PdfFallbackFontKind.cjk, bytes); // .emoji for emoji
await editor.flattenForms();
```

### The detector name, only if you used 2.2.0

**`trim-detector: analyzer`** — The retracted 2.2.0 named its source-scan detector `analyzer`. It is `scan`, the default, so change the line or delete it.

Before:
```yaml
hooks:
  user_defines:
    pdf_manipulator:
      trim: auto
      trim-detector: analyzer
```

After:
```yaml
hooks:
  user_defines:
    pdf_manipulator:
      trim: auto
```

## From 1.0.6 to 2.0.0

2.0.0 replaced the web's two worker scripts with one. Only web apps change.

The snippets assume a `pdf`, `source`, `output`, `doc`, `editor` and `bytes` of the types in the Pdf API.

### The web worker

**`webLaneWorkerUrl`** — If you set the worker URLs yourself, the two settings are one.

Before:
```dart
final custom = Pdf(
  config: const PdfConfig(
    webCoordinatorUrl: 'pdf_manipulator/coordinator.js',
    webWorkerUrl: 'pdf_manipulator/worker.js',
  ),
);
```

After:
```dart
final custom = Pdf(
  config: const PdfConfig(webLaneWorkerUrl: 'pdf_manipulator/lane_worker.js'),
);
```

**`lane_worker.js`** — Setup writes `lane_worker.js` where it wrote `coordinator.js` and `worker.js`. Run `flutter pub run pdf_manipulator:setup` again, delete the two old files from `web/pdf_manipulator/`, and change any precache list or hosting rule that names them.

Before:
```text
web/pdf_manipulator/coordinator.js
web/pdf_manipulator/worker.js
```

After:
```text
web/pdf_manipulator/lane_worker.js
```

## From 0.5.9 to 6.0.0

0.5.9 was the last release of the Android-only plugin. 6.0.0 is a different library with the same name, so every call changes. Four shifts cover most of the work:

| 0.5.9 | 6.0.0 |
|:---|:---|
| Android only | Android, iOS, macOS, Windows, Linux and the web |
| A file path in, the path of a temporary result file out | a `PdfInput` in (`PdfFile(File(path))` for a file), and the result written to a sink you choose (`await FileSink.open(File(path))`, or `MemorySink()` for bytes) |
| Page numbers start at 1 | page indexes start at 0: subtract 1 from every page number |
| `PlatformException` on failure; `cancelManipulations()` stops everything | a typed `PdfError`; every call returns a `PdfTask` you can cancel on its own |

You make one `Pdf()` for the app and `dispose()` it at the end; it replaces `PdfManipulator()`. Several changes to one file are one `pdf.rewrite`, which reads the file once and writes once.

The Before snippets assume a `plugin` (a `PdfManipulator`), a `path`, `pathA` and `pathB` (`String`) and `imagePaths` (`List<String>`). The After snippets assume a `pdf` (a `Pdf`), an `output` (a `DataSink`) and the same strings.

### Merge

**`mergePDFs`** — Pass the inputs in order; the first is the base and the others are appended whole.

Before:
```dart
final mergedPath = await plugin.mergePDFs(
  params: PDFMergerParams(pdfsPaths: [pathA, pathB]),
);
```
After:
```dart
await pdf.merge([PdfFile(File(pathA)), PdfFile(File(pathB))], output);
```

### Split

**`splitPDF`** — `pageCount` and `byteSize` are a `PdfSplitRule`. `split` asks your function for one sink per part and closes each one after its part. `pageRanges`, `pageRange` and `pageNumbers` (a new part before each listed page) become one `keepPages` per part.

Before:
```dart
final byCount = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, pageCount: 2),
);
final bySize = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, byteSize: 500000),
);
final byRange = await plugin.splitPDF(
  params: PDFSplitterParams(pdfPath: path, pageRanges: ['1-3', '4-6']),
);
```
After:
```dart
await pdf.split(
  PdfFile(File(path)),
  (part) => FileSink.open(File('part-${part.index}.pdf')),
  by: PdfSplitRule.every(2),
);
await pdf.split(
  PdfFile(File(path)),
  (part) => FileSink.open(File('part-${part.index}.pdf')),
  by: PdfSplitRule.maxBytes(500000),
);

// pages 1-3 in 0.5.9 are PdfPages.range(0, 3): indexes 0, 1 and 2
final first = await FileSink.open(File('pages-1-3.pdf'));
try {
  await pdf.editOnce(PdfFile(File(path)), first).keepPages(PdfPages.range(0, 3));
} finally {
  await first.close();
}
```

### Delete pages

**`pdfPageDeleter`** — Subtract 1 from each page number.

Before:
```dart
final resultPath = await plugin.pdfPageDeleter(
  params: PDFPageDeleterParams(pdfPath: path, pageNumbers: [2, 4]),
);
```
After:
```dart
await pdf.editOnce(PdfFile(File(path)), output).removePages(PdfPages.list([1, 3]));
```

### Reorder pages

**`pdfPageReorder`** — A `PdfPageOrder` keeps the order you give and drops the pages you leave out, as 0.5.9 did.

Before:
```dart
final resultPath = await plugin.pdfPageReorder(
  params: PDFPageReorderParams(pdfPath: path, pageNumbers: [3, 1, 2]),
);
```
After:
```dart
await pdf.editOnce(PdfFile(File(path)), output).reorderPages(PdfPageOrder([2, 0, 1]));
```

### Rotate pages

**`pdfPageRotator`** — `rotatePages` turns pages by an angle on top of their current rotation, as `rotationAngle` did. Pages that turn by the same angle go in one call; `setRotation` sets an absolute rotation instead.

Before:
```dart
final resultPath = await plugin.pdfPageRotator(
  params: PDFPageRotatorParams(
    pdfPath: path,
    pagesRotationInfo: [
      PageRotationInfo(pageNumber: 1, rotationAngle: 90),
      PageRotationInfo(pageNumber: 3, rotationAngle: 180),
    ],
  ),
);
```
After:
```dart
await pdf.rewrite(PdfFile(File(path)), output, (editor) async {
  await editor.rotatePages(by: PdfRotation.r90, pages: PdfPages.single(0));
  await editor.rotatePages(by: PdfRotation.r180, pages: PdfPages.single(2));
});
```

### Rotate, delete and reorder in one pass

**`pdfPageRotatorDeleterReorder`** — One `rewrite` holds all three. 0.5.9 dropped the deleted pages from the order; with `reorderPages` you write the order without them, and any page you leave out is removed too.

Before:
```dart
final resultPath = await plugin.pdfPageRotatorDeleterReorder(
  params: PDFPageRotatorDeleterReorderParams(
    pdfPath: path,
    pagesRotationInfo: [PageRotationInfo(pageNumber: 1, rotationAngle: 90)],
    pageNumbersForDeleter: [2, 4],
    pageNumbersForReorder: [3, 1, 2],
  ),
);
```
After:
```dart
await pdf.rewrite(PdfFile(File(path)), output, (editor) async {
  await editor.rotatePages(by: PdfRotation.r90, pages: PdfPages.single(0));
  await editor.reorderPages(PdfPageOrder([2, 0])); // pages 3 and 1; 2 and 4 are gone
});
```

### Compress

**`pdfCompressor`** — `imageScale` shrank every image by the same factor. A `PdfImagePolicy` gives a target resolution instead, in pixels per inch at the size the image is drawn, so a small logo and a full-page scan each get what they need. `imageQuality` is `jpegQuality`. `unEmbedFonts` is `unembedStandardFonts`.

Before:
```dart
final resultPath = await plugin.pdfCompressor(
  params: PDFCompressorParams(
    pdfPath: path,
    imageQuality: 70,
    imageScale: 0.5,
    unEmbedFonts: true,
  ),
);
```
After:
```dart
await pdf.rewrite(PdfFile(File(path)), output, (editor) async {
  await editor.reduceImages(
    const PdfImagePolicy(colorPpi: 150, grayPpi: 150, jpegQuality: 70),
  );
  await editor.unembedStandardFonts();
});

// without the fonts, one call that never makes the file larger
await pdf.compress(PdfFile(File(path)), output, images: PdfImagePolicy.ebook);
```

### Watermark

**`pdfWatermark`** — The text's look is a `PdfWatermarkStyle`, where it goes a `PdfWatermarkPosition`, and above or below the content a `PdfWatermarkLayer`; the table under the code maps each field.

Before:
```dart
final resultPath = await plugin.pdfWatermark(
  params: PDFWatermarkParams(
    pdfPath: path,
    text: 'DRAFT',
    fontSize: 80,
    opacity: 0.3,
    rotationAngle: 45,
    watermarkLayer: WatermarkLayer.overContent,
    positionType: PositionType.center,
  ),
);
```
After:
```dart
await pdf.editOnce(PdfFile(File(path)), output).addWatermark(
  'DRAFT',
  style: const PdfWatermarkStyle(fontSize: 80, opacity: 0.3, angleDegrees: 45),
  position: const PdfWatermarkPosition.center(),
  layer: PdfWatermarkLayer.foreground,
);
```

| 0.5.9 | 6.0.0 |
|:---|:---|
| `fontSize`, `opacity` | the same names in `PdfWatermarkStyle` |
| `rotationAngle` | `angleDegrees` |
| `watermarkColor` (a Flutter `Color`) | `color`, a `PdfColor` with red, green and blue from 0 to 1 |
| `WatermarkLayer.overContent`, `.underContent` | `PdfWatermarkLayer.foreground`, `.background` |
| `PositionType.center` | `PdfWatermarkPosition.center()` |
| `PositionType.topLeft`, `.topRight`, `.bottomLeft`, `.bottomRight` | `PdfWatermarkPosition.corner(PdfCorner.topLeft)` and its siblings |
| `PositionType.custom` with coordinate lists, and the other `PositionType` values | `PdfWatermarkPosition.exact(PdfRect(...))` |

### Encrypt

**`pdfEncryption`** — `resave` writes the file again with new encryption. The `allow…` flags become a `PdfPermissions` and the algorithm flags a `PdfEncryptionAlgorithm`; the tables under the code map each one.

Before:
```dart
final resultPath = await plugin.pdfEncryption(
  params: PDFEncryptionParams(
    pdfPath: path,
    userPassword: 'user',
    ownerPassword: 'owner',
    allowPrinting: true,
    allowCopy: true,
    encryptionAES256: true,
  ),
);
```
After:
```dart
await pdf.resave(
  PdfFile(File(path)),
  output,
  save: PdfSave.full(
    encryption: PdfEncryption.password(
      ownerPassword: 'owner',
      userPassword: 'user',
      algorithm: PdfEncryptionAlgorithm.aesKey256,
      permissions: const PdfPermissions(
        canModify: false,
        canAnnotate: false,
        canFillForms: false,
        canExtractForAccessibility: false,
        canAssemble: false,
      ),
    ),
  ),
);
```

`PdfPermissions` starts with every flag `true`, where 0.5.9 started with every `allow…` flag `false`, so set to `false` what you did not allow.

| 0.5.9 | 6.0.0 |
|:---|:---|
| `allowPrinting` | `canPrint` and `canPrintHighQuality` |
| `allowDegradedPrinting` | `canPrint` alone |
| `allowModifyContents` | `canModify` |
| `allowCopy` | `canCopy` |
| `allowModifyAnnotations` | `canAnnotate` |
| `allowFillIn` | `canFillForms` |
| `allowScreenReaders` | `canExtractForAccessibility` |
| `allowAssembly` | `canAssemble` |
| `standardEncryptionAES40`, `standardEncryptionAES128` (RC4, despite the names) | `PdfEncryptionAlgorithm.rc4Key40`, `.rc4Key128` |
| `encryptionAES128`, `encryptionAES256` | `PdfEncryptionAlgorithm.aesKey128`, `.aesKey256` |
| `encryptEmbeddedFilesOnly`, `doNotEncryptMetadata` | no equivalent: everything is encrypted |

### Decrypt

**`pdfDecryption`** — The password goes with the input, and `resave` writes the file again without encryption.

Before:
```dart
final resultPath = await plugin.pdfDecryption(
  params: PDFDecryptionParams(pdfPath: path, password: 'pw'),
);
```
After:
```dart
await pdf.resave(
  PdfFile(File(path), password: 'pw'),
  output,
  save: PdfSave.full(encryption: PdfEncryption.remove()),
);
```

### Validity and protection

**`pdfValidityAndProtection`** — `pdf.info` reads the facts of a document. A file that is not a valid PDF throws `PdfCorrupted`; a file that needs a password to open throws `PdfPasswordRequired` until you pass one. `encryption` is null when the file is not encrypted.

Before:
```dart
final result = await plugin.pdfValidityAndProtection(
  params: PDFValidityAndProtectionParams(pdfPath: path),
);
print('${result?.isPDFValid} ${result?.isOwnerPasswordProtected} '
    '${result?.isOpenPasswordProtected} ${result?.isPrintingAllowed}');
```
After:
```dart
try {
  final info = await pdf.info(PdfFile(File(path)));
  final encryption = info.encryption;
  print('protected: ${encryption != null}, '
      'printing allowed: ${encryption?.permissions.canPrint ?? true}');
} on PdfPasswordRequired {
  print('needs a password to open');
} on PdfCorrupted {
  print('not a valid PDF');
}
```

### Page sizes

**`pdfPagesSize`** — `pageInfo` lists every page with its index from 0, its boxes in points, its rotation and its label.

Before:
```dart
final sizes = await plugin.pdfPagesSize(
  params: PDFPagesSizeParams(pdfPath: path),
);
for (final size in sizes ?? <PageSizeInfo>[]) {
  print('${size.pageNumber}: ${size.widthOfPage} x ${size.heightOfPage}');
}
```
After:
```dart
for (final page in await pdf.readOnce(PdfFile(File(path))).pageInfo()) {
  print('${page.page}: ${page.mediaBox.width} x ${page.mediaBox.height}');
}
```

### Images to PDF

**`imagesToPdfs`** — Images are plain sources, one page per image at the image's size. `createSinglePdf: true` is one `imagesToPdf` call; for one PDF per image, call it once per image.

Before:
```dart
final pdfPaths = await plugin.imagesToPdfs(
  params: ImagesToPDFsParams(imagesPaths: imagePaths, createSinglePdf: true),
);
```
After:
```dart
final images = [for (final p in imagePaths) await FileSource.open(File(p))];
try {
  await pdf.imagesToPdf(images, output);
} finally {
  for (final image in images) {
    await image.close();
  }
}
```

### Cancel

**`cancelManipulations`** — Every call returns a `PdfTask`, a `Future` with `cancel()`. Awaiting a cancelled task throws `PdfCancelled`, and nothing is half-written. `pdf.dispose()` cancels everything.

Before:
```dart
await plugin.cancelManipulations();
```
After:
```dart
final task = pdf.merge([PdfFile(File(pathA)), PdfFile(File(pathB))], output);
task.cancel();
await pdf.dispose();
```

### Errors

**`PlatformException`** — A failure is a `PdfError` subtype that reaches you through the `await`; catch the cases you handle and let the rest fall to `PdfError`.

Before:
```dart
try {
  await plugin.mergePDFs(params: PDFMergerParams(pdfsPaths: [pathA, pathB]));
} on PlatformException catch (e) {
  print(e.message);
}
```
After:
```dart
try {
  await pdf.merge([PdfFile(File(pathA)), PdfFile(File(pathB))], output);
} on PdfPasswordRequired {
  print('ask for the password');
} on PdfError catch (e) {
  print(e.message);
}
```

### Types

| 0.5.9 | 6.0.0 |
|:---|:---|
| `PdfManipulator` | `Pdf` |
| `PDFMergerParams` | the arguments of `pdf.merge` |
| `PDFSplitterParams` | `PdfSplitRule`, or `keepPages` per part |
| `PDFPageDeleterParams` | `removePages` |
| `PDFPageReorderParams` | `reorderPages` with a `PdfPageOrder` |
| `PDFPageRotatorParams`, `PageRotationInfo` | `rotatePages(by:, pages:)` |
| `PDFPageRotatorDeleterReorderParams` | one `pdf.rewrite` with each edit |
| `PDFCompressorParams` | `PdfImagePolicy`, `unembedStandardFonts` |
| `PDFWatermarkParams`, `WatermarkLayer`, `PositionType` | `addWatermark` with `PdfWatermarkStyle`, `PdfWatermarkLayer`, `PdfWatermarkPosition` |
| `PDFEncryptionParams` | `PdfSave.full(encryption: PdfEncryption.password(...))` |
| `PDFDecryptionParams` | `PdfSave.full(encryption: PdfEncryption.remove())` |
| `PDFValidityAndProtectionParams`, `PdfValidityAndProtection` | `pdf.info`, `PdfDocumentInfo` |
| `PDFPagesSizeParams`, `PageSizeInfo` | `pageInfo`, `PdfPageInfo` |
| `ImagesToPDFsParams` | the arguments of `pdf.imagesToPdf` |
| `PlatformException` | `PdfError` |

Everything else 6.0.0 does is new to you: text, search, rendering, forms, signatures, redaction, PDF/A and Office conversion, and building PDFs from scratch. The [README](../README.md) shows each one.
