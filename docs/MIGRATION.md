# Migrating pdf_manipulator

This guide covers the breaking changes from 5.0.1 to 6.0.0. Five concepts changed shape, and each has a Before and an After. The rename table and the list of removed members after them cover the rest. The snippets assume a `pdf`, `source`, `output`, `doc`, `editor`, `builder`, `page`, `rect`, `bytes`, `sources`, `images` and `kind` of the types in the Pdf API.

## From 5.0.1 to 6.0.0

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

**`PdfPages`** — A page set is a `PdfPages`, and it is valid when you build it: `PdfPages.list` sorts the pages and throws `ArgumentError` for an empty list, a negative page and a page given twice. This follows Bloch's "Hard to misuse". A range ends exclusive: `PdfPages.range(0, 4)` is pages 0 to 3, as in `List.sublist` and in Effective Dart's rule that a range has an inclusive start and an exclusive end. PyMuPDF's `insert_pdf`, for one, includes both ends, so do not copy a range from it. Where the order is the point, `PdfPageOrder` keeps the order you give; `keepPages` keeps pages in document order.
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

| 5.0.1 | 6.0.0 |
|---|---|
| `FileSource`, `FileSink.create` | `await FileSource.open(file)`, `await FileSink.open(file)` |
| `PdfDoc`, `PdfSugar`, `PdfStandalone` | `PdfDocument` and `PdfDocumentHandle`; the calls are members of `Pdf` |
| `title`, `author`, `subject`, `keywords`, `creator`, `producer`, `creationDate` of a document | `doc.metadata.title` and the rest; `creationDate` is a UTC `DateTime?` |
| `isEncrypted`, `requiresPassword`, `encryptionAlgorithm`, `permissions` | `doc.encryption`, which is null when the document is not encrypted |
| `extract({pages, format})` | `text({pages, format})`, or `extractText(output, {pages, format})` |
| `search(query:, pages:)`, `SearchResult` | `search(query, {pages})`, `PdfSearchHit` |
| `render`, `RenderedPage`, `extractImages`, `PdfImage` | `render(sinkFor)`, `PdfRenderedPage`, `extractImages(sinkFor)`, `PdfImageInfo` |
| `PdfRenderSize(maxWidth:, maxHeight:)`, `thumbnail(n)` | `PdfRenderSize.fit(maxWidth:, maxHeight:)`, `.dpi(n)` |
| `signatures`, `formFields`, `xfa`, `attachments` (getters) | the same names as methods |
| `validatePdfA`, `validatePdfUa`, `PdfValidationResult` | `PdfALevel` and `PdfUaLevel` levels, `PdfValidationReport` with `isCompliant` |
| `convertToPdfA` (level `int`, default 1) | `convertToPdfA` with a `PdfALevel`, default `a2` |
| `planSplitByBookmarks`, `PdfBookmarkSplit` | `bookmarkRanges`, `PdfBookmarkRange` with `start` and `end` |
| `classifyPage`, `PdfDocumentClassification` | `classifyPages({pages})`, `PdfDocumentKind` |
| `PdfPageInfo` with `index`, `rotation` as `int` | `page`, `mediaBox`, `cropBox`, `rotation` as `PdfRotation` |
| `PdfFormField` with `type`, `readOnly`, `required`, `PdfFormFieldType` | `kind`, `isReadOnly`, `isRequired`, `PdfFormFieldKind` |
| `PdfCheckedValue.checked` | `isChecked` |
| `size` of `PdfAttachment`, `serial`, `regions` | `sizeBytes`, `serialNumber`, `regionCount` |
| `PdfPermissions` with `print`, `modify`, `copy` and the rest | `canPrint`, `canModify`, `canCopy` and the rest |
| `monoPpi`, `minSavings` of `PdfImagePolicy`; `bits`, `indexed`, `ppiMin`, `uses` of `PdfImageOutcome` | `bilevelPpi`, `minSavingsRatio`, `bitsPerComponent`, `isIndexed`, `minPpi`, `placementCount` |
| `rotation` of `PdfWatermarkStyle` | `angleDegrees` |
| `PdfExtractionFormat.plainText`, `.text` | `plain`, `layout` |
| `PdfEncryptionAlgorithm.rc4_40`, `rc4_128`, `aes128`, `aes256` | `rc4Key40`, `rc4Key128`, `aesKey128`, `aesKey256` |
| `PdfConfig(webLaneWorkerUrl:, maxLanes:)` | `PdfConfig(webWorkerUrl:, concurrency:)` |
| `PdfEditor.pageCount`, `version`, `isModified` (getters) | methods |
| `setTitle`, `setAuthor`, `setSubject`, `setKeywords`, `setProducer`, `setCreationDate` | `setMetadata(PdfMetadata)`, and `metadata()` to read |
| `rotatePage`, `rotateAllPages`, `setPageRotation` | `rotatePages(by:, pages:)`, `setRotation(rotation, pages:)` |
| `pageMediaBox`, `pageCropBox`, `setPageMediaBox`, `setPageCropBox` | `pageInfo`, `setMediaBox(box, {pages})`, `setCropBox(box, {pages})` |
| `deletePage`, `selectPages` | `removePages`, `keepPages` or `reorderPages` |
| `mergeFrom(source, {pages})` | `appendPages(PdfInput, {pages})` |
| `addWatermark(page, text)`, `addStamp(page, type:, rect:)`, `PdfStampType` | `addWatermark(text, {pages})`, `addStamp(stamp, bounds:, pages:)`, `PdfStamp` |
| `embedFile`, `addRedaction` | `addAttachment`, `markRedaction(page, regions)` |
| `flattenForms({int? page})`, `flattenAnnotations` | `pages:` takes a `PdfPages` |
| `PdfSaveOptions`, `PdfSaveOptions.fullRewrite(compress:)`, `PdfEncryption.config` | `PdfSave`, `PdfSave.full(deflate:)`, `PdfEncryption.password` |
| `PdfSaveOptions` with `incremental()` | `PdfSave.incremental()` appends and throws `PdfIncrementalRefused` when it cannot; `PdfSave.incrementalOrRewrite()` falls back to a full save |
| `PdfEncryptionKeep`, `PdfPagesAll`, `PdfWatermarkCenter` and the other subclasses | not exported; call `PdfEncryption.keep()`, `PdfPages.all()`, the factories |
| `PdfIoError`, `PdfEngineError`, `PdfPageRangeError`, `PdfCryptoError` | `PdfIoFailed`, `PdfEngineFailed`, `PdfPageOutOfRange`, `PdfCryptoFailed` |
| `convertTo`, `convertToPdf`, `PdfDocumentFormat` | `convertToOffice`, `convertFromOffice`, `PdfOfficeFormat` |
| `addA4Page`, `addLetterPage`, `addPage(width:, height:)` | `addPage([PdfPageSize])` |
| `textField` (`defaultValue`), `checkbox` (`checked`), `PdfTextAlignment` | `value`, `isChecked`, `PdfAlignment` |
| `bottom` of a `PdfRect` (`y + height`) | `top`; `bottom` is now `y` |
| `PdfColor` and every result and options type | `final` with value `==`; wrap one, do not extend it |
| `PdfSignatureInfo` | needs `coversWholeDocument` when you build one |
| `PdfCheckedValue` for a radio group, `PdfTextValue` for a combo box | `PdfChoiceValue` (`PdfMultiChoiceValue` for several options), or `PdfNoValue` |
| `font`, `at` of a page | `setFont`, `moveTo(PdfPoint)` |

### Removed

- `pdf_oxide_bg.wasm` and `pdf_oxide_asyncify_bg.wasm` in a precache list, a hosting rule or a deploy script: the web assets are `pdf_oxide_bg.<n>.wasm` (and, with `asyncify: true`, `pdf_oxide_asyncify_bg.<n>.wasm`) listed in `pdf_oxide_manifest.json`. Run setup again, delete the old whole files from `web/pdf_manipulator/`, and list the parts the manifest names where the whole file was listed; serve `.wasm` as `application/wasm`.
- `compress(garbageCollect:)`: it always collects garbage. Its `images` default is `PdfImagePolicy.lossless`; pass `PdfImagePolicy.ebook` for the old result.
- `imagesToPdf` on A4 pages: each page has its image's size; pass `page: PdfImagePage(size: PdfImagePageSize.fixed(PdfPageSize.a4()))` for A4.
- `split`, `splitBySize` and `splitByBookmarks`: one `split(input, sinkFor, by: PdfSplitRule.every(n))`, or `.maxBytes(n)`, or `.bookmarks()`.
- `deletePages`, `reorderPages`, `movePage`, `rotatePages`, `rotateAllPages`, `flattenForms`, `embedFile`, `watermark`, `addStamp`, `addImageStamp` and `convertToPdfA` as one-shots: `pdf.editOnce(input, output)` and the edit.
- `applyRedactions` as a one-shot: call `applyRedactions()` in a `pdf.rewrite` body.
- `eraseRegions` and `clearEraseRegions`: `redact(page, regions, overlay: const PdfRedactionOverlay.none())` and `clearRedactions()`.
- `encrypt` and `decrypt`: `pdf.resave(input, output, save: PdfSave.full(encryption: ...))` with `PdfEncryption.password` or `.remove()`.
- `extractPages`: `keepPages(PdfPages.list([0, 2]))`, or `reorderPages(PdfPageOrder([2, 0]))`, on `pdf.editOnce`.
- `registerFallbackFont`: the font is a `DataSource`; wrap bytes in `MemorySource(bytes)`.
- `PdfDoc.pages`: `await doc.pageInfo()`.
- `PdfDoc.formField(name)`: find the field in `await doc.formFields()`.
- `setFormFieldValue`, `setCheckboxFieldValue`, `removeFormField` and the other `setFormField…`: `editor.field(name).setValue(...)` and its siblings.
- `resizeImage` and `repositionImage`: `setImageBounds(page, name, bounds)`.
- `scrubMetadata` and `PdfSanitizeOptions`: `sanitize(stripMetadata:, stripJavaScript:, stripAttachments:)`.
- `PdfInvalidArgument`: an `ArgumentError` or a `StateError` for your own mistake, `PdfEngineFailed` for an engine refusal.
- `PdfSearchError` and `PdfExtractionFailed`: nothing threw them; delete their `catch` clauses.
- `fieldKeystroke`, `fieldFormat`, `fieldValidate`, `fieldCalculate`, `linkUrl` and `linkPage`: the `scripts:` and `link:` arguments of the call that writes the field or text.
- `done` and `newPageSameSize`: a page ends when the next is added or the document is saved.
- `toBits` of `PdfPermissions`: gone; the library encodes the flags itself.
