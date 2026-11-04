# Migrating pdf_manipulator

This guide covers the breaking changes from 5.0.1 to 6.0.0. Each entry shows the 5.0.1 code and the 6.0.0 code for the same job. The snippets assume a `pdf`, `source`, `output`, `doc`, `editor`, `builder`, `page`, `rect`, `bytes`, `sources`, `images` and `kind` of the types in the Pdf API.

## From 5.0.1 to 6.0.0

### Sources and sinks

**`DataSource`** — Every source and sink you open must be closed, `FileSource` opens through an asynchronous factory, and `FileSink.create` is now `FileSink.open`.
Before:
```dart
final input = FileSource(File('in.pdf'));
final result = await FileSink.create(File('out.pdf'));
await pdf.compress(input, result);
await result.close();
```
After:
```dart
final input = await FileSource.open(File('in.pdf'));
final result = await FileSink.open(File('out.pdf'));
await pdf.compress(input, result);
await result.close();
await input.close();
```

**`DataSink`** — The library now calls `abort(Object reason)` on the sink of an operation that ends cancelled or failed, so you no longer clean up the half-written output yourself; a sink of your own must implement `abort`.
Before:
```dart
final result = await FileSink.create(File('out.pdf'));
try {
  await pdf.compress(source, result);
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
  await pdf.compress(source, result);
} finally {
  await result.close();
}
```

**`RenderedPage`** — The type is now `PdfRenderedPage`, and `render` writes each page to the sink that `sinkFor` returns instead of yielding the bytes.
Before:
```dart
await for (final RenderedPage shot in doc.render(pages: const PdfPages.all())) {
  final png = shot.data;
}
```
After:
```dart
final sinks = <int, MemorySink>{};
final shots = await doc.render(
  (PdfRenderedPage shot) => sinks[shot.page] = MemorySink(),
  pages: const PdfPages.all(),
);
final png = sinks[shots.first.page]!.takeBytes();
```

**`PdfRect`** — A rectangle has `left`, `bottom`, `right` and `top` edges and value equality, so you no longer add the width to `x`.
Before:
```dart
final right = rect.x + rect.width;
final top = rect.y + rect.height;
```
After:
```dart
final right = rect.right;
final top = rect.top;
```

### Opening, reading and the one-shots

**`PdfDisposed`** — Using a disposed document throws `PdfDisposed`, where it threw a `StateError`.
Before:
```dart
await doc.dispose();
try {
  await doc.verifySignatures();
} on StateError catch (_) {
  final disposed = true;
}
```
After:
```dart
await doc.dispose();
try {
  await doc.verifySignatures();
} on PdfDisposed catch (_) {
  final disposed = true;
}
```

**`read`** — The `read` door opens the document, runs your function and disposes the document, where you opened and disposed it yourself.
Before:
```dart
final opened = await pdf.open(source);
try {
  final count = opened.pageCount;
} finally {
  await opened.dispose();
}
```
After:
```dart
final count = await pdf.read(source, (opened) async => opened.pageCount);
```

**`PdfSaveOptions.incremental()`** — An incremental save now appends to the original bytes and throws `PdfIncrementalRefused` when the edits cannot be appended, so save again with a full rewrite.
Before:
```dart
await editor.save(output, options: const PdfSaveOptions.incremental());
```
After:
```dart
try {
  await editor.save(output, options: const PdfSaveOptions.incremental());
} on PdfIncrementalRefused {
  await editor.save(MemorySink(), options: const PdfSaveOptions.fullRewrite());
}
```

**`PdfError` (new subtype)** — `PdfIncrementalRefused` is a new `PdfError` subtype, so a `switch` over every subtype needs a case for it.
Before:
```dart
try {
  await editor.save(output);
} on PdfError catch (error) {
  final retryable = switch (error) {
    PdfIoError() || PdfCancelled() => true,
    PdfInvalidArgument() ||
    PdfCorrupted() ||
    PdfEngineError() ||
    PdfPageRangeError() ||
    PdfUnsupported() ||
    PdfPasswordRequired() ||
    PdfWrongPassword() ||
    PdfCryptoError() ||
    PdfSearchError() ||
    PdfExtractionFailed() => false,
  };
}
```
After:
```dart
try {
  await editor.save(output);
} on PdfError catch (error) {
  final retryable = switch (error) {
    PdfIoError() || PdfCancelled() => true,
    PdfInvalidArgument() ||
    PdfCorrupted() ||
    PdfEngineError() ||
    PdfPageRangeError() ||
    PdfUnsupported() ||
    PdfPasswordRequired() ||
    PdfWrongPassword() ||
    PdfCryptoError() ||
    PdfDisposed() ||
    PdfIncrementalRefused() => false,
  };
}
```

**`keepPages`** — `keepPages` (5.0.1: `selectPages`) throws `PdfInvalidArgument` for a page listed twice, where it wrote a damaged file.
Before:
```dart
await editor.selectPages([0, 2, 0]);
```
After:
```dart
try {
  await editor.keepPages([0, 2, 0]);
} on PdfInvalidArgument {
  await editor.keepPages([0, 2]);
}
```

**`PdfError` (engine subtypes)** — Engine failures throw their own subtype, such as `PdfCorrupted`, where they threw `PdfEngineError`.
Before:
```dart
try {
  await pdf.open(source);
} on PdfEngineError catch (error) {
  final reported = error;
}
```
After:
```dart
try {
  await pdf.open(source);
} on PdfCorrupted catch (error) {
  final reported = error;
} on PdfError catch (error) {
  final reported = error;
}
```

**`fieldKeystroke`** — The field-script calls throw `PdfInvalidArgument` on a page with no form field, where they did nothing, so add the field first.
Before:
```dart
await page.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true);');
```
After:
```dart
await page.textField('price', rect);
await page.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true);');
```

**`PdfChoiceValue` (radio groups)** — A radio group's value reads as `PdfChoiceValue` with the selected option's name, or `PdfNoValue`, where it read as `PdfCheckedValue`.
Before:
```dart
final fields = await doc.formFields;
final group = fields.firstWhere((field) => field.name == 'size');
final value = group.value;
final selected = value is PdfCheckedValue && value.checked;
```
After:
```dart
final fields = await doc.formFields();
final group = fields.firstWhere((field) => field.name == 'size');
final value = group.value;
final selected = value is PdfChoiceValue ? value.choice : null;
```

**`PdfSignatureInfo`** — The constructor requires `coversWholeDocument`.
Before:
```dart
const signature = PdfSignatureInfo(signerName: 'Ada', isValid: true);
```
After:
```dart
const signature = PdfSignatureInfo(
  signerName: 'Ada',
  isValid: true,
  coversWholeDocument: true,
);
```

**`PdfRotation`** — A page's rotation is a `PdfRotation`, no longer an `int` of degrees.
Before:
```dart
final turned = doc.pages.first.rotation == 90;
```
After:
```dart
final turned = (await doc.pageInfo()).first.rotation == PdfRotation.r90;
```

**`DateTime?`** — A document date is a UTC `DateTime?`, where it was a raw PDF date string.
Before:
```dart
final String? raw = doc.creationDate;
final year = raw == null ? null : int.tryParse(raw.substring(2, 6));
```
After:
```dart
final DateTime? created = doc.metadata.creationDate;
final year = created?.year;
```

**`validatePdfA`** — PDF/A validation takes a `PdfALevel`, no longer an `int`.
Before:
```dart
final result = await doc.validatePdfA(level: 2);
```
After:
```dart
final result = await doc.validatePdfA(level: PdfALevel.a2);
```

**`PdfColor`** — A `PdfColor` is a final class with value equality, so wrap it instead of extending it and compare with `==`.
Before:
```dart
const first = PdfColor(0.1, 0.2, 0.3);
const second = PdfColor(0.1, 0.2, 0.3);
final same = first.r == second.r && first.g == second.g && first.b == second.b;
```
After:
```dart
const first = PdfColor(0.1, 0.2, 0.3);
const second = PdfColor(0.1, 0.2, 0.3);
final same = first == second;
```

**`PdfPagesAll`** — The subclasses of `PdfPages` are not exported, so create a page set with its factory.
Before:
```dart
const everything = PdfPagesAll();
```
After:
```dart
const everything = PdfPages.all();
```

**`SearchResult`** — The type is now `PdfSearchHit`, with the same fields.
Before:
```dart
final hits = <SearchResult>[];
hits.sort((a, b) => a.page.compareTo(b.page));
```
After:
```dart
final hits = <PdfSearchHit>[];
hits.sort((a, b) => a.page.compareTo(b.page));
```

**`PdfSearchError`** — `PdfSearchError` and `PdfExtractionFailed` are removed because nothing threw them, so delete their `catch` clauses; failures arrive as other `PdfError` subtypes.
Before:
```dart
try {
  await doc.verifySignatures();
} on PdfSearchError catch (_) {
  final failed = true;
} on PdfExtractionFailed catch (_) {
  final failed = true;
}
```
After:
```dart
try {
  await doc.verifySignatures();
} on PdfError catch (_) {
  final failed = true;
}
```

### Editing

**`compress`** — In `PdfSaveOptions.fullRewrite` the argument `compress` is now `deflate`.
Before:
```dart
await editor.save(output, options: const PdfSaveOptions.fullRewrite(compress: false));
```
After:
```dart
await editor.save(output, options: const PdfSaveOptions.fullRewrite(deflate: false));
```

**`setTitle`** — The builder's four metadata setters are one `setMetadata`.
Before:
```dart
await builder.setTitle('Report');
await builder.setAuthor('Ada');
```
After:
```dart
await builder.setMetadata(const PdfMetadata(title: 'Report', author: 'Ada'));
```

**`PdfPageSize`** — A page takes a `PdfPageSize`; `addA4Page`, `addLetterPage` and `addPage(width:, height:)` are gone.
Before:
```dart
await builder.addA4Page();
await builder.addLetterPage();
await builder.addPage(width: 400, height: 300);
```
After:
```dart
await builder.addPage();
await builder.addPage(const PdfPageSize.letter());
await builder.addPage(const PdfPageSize.custom(400, 300));
```

**`PdfCellAlign`** — `PdfTextAlignment` (and `PdfCellAlign`, which 5.0.1 did not have) are one `PdfAlignment`.
Before:
```dart
const align = PdfTextAlignment.center;
```
After:
```dart
const align = PdfAlignment.center;
```

**`imagesToPdf`** — Each page now has the image's own size, one point per pixel, so name a size to keep the A4 pages of 5.0.1.
Before:
```dart
await pdf.imagesToPdf(images, output);
```
After:
```dart
await pdf.imagesToPdf(images, output, page: const PdfImagePageOptions(size: PdfPageSize.a4()));
```

**`PdfInfo`** — A document keeps its metadata in `metadata` and its encryption in `encryption`, which is null when the document is not encrypted.
Before:
```dart
final title = doc.title;
final encrypted = doc.isEncrypted;
final permissions = doc.permissions;
```
After:
```dart
final title = doc.metadata.title;
final encrypted = doc.encryption != null;
final permissions = doc.encryption?.permissions;
```

**`PdfDoc.pages`** — The per-page facts are a read of their own, `pageInfo`, and opening a document no longer reads them.
Before:
```dart
final first = doc.pages.first;
```
After:
```dart
final first = (await doc.pageInfo()).first;
```

**`PdfDoc.extract`** — `extractText` writes the text to a sink as the engine produces it, where `extract` returned a string.
Before:
```dart
final text = await doc.extract(pages: const PdfPages.all());
```
After:
```dart
final sink = MemorySink();
await doc.extractText(sink, pages: const PdfPages.all());
final text = sink.takeString();
```

**`search`** — The query is the first positional argument.
Before:
```dart
await doc.search(query: 'Ada', pages: const PdfPages.all());
```
After:
```dart
await doc.search('Ada', pages: const PdfPages.all());
```

**`PdfEditor.pageCount`** — `pageCount`, `version` and `isModified` are methods.
Before:
```dart
final count = await editor.pageCount;
```
After:
```dart
final count = await editor.pageCount();
```

**`title`** — The editor's metadata is read with `metadata()` and replaced with `setMetadata`, so copy the current value to change one field.
Before:
```dart
final title = await editor.title;
await editor.setTitle('X');
```
After:
```dart
final current = await editor.metadata();
final title = current.title;
await editor.setMetadata(current.copyWith(title: 'X'));
```

**`rotatePage`** — Rotation is set for a page set with a `PdfRotation`.
Before:
```dart
await editor.rotatePage(0, degrees: 90);
await editor.rotateAllPages(degrees: 90);
```
After:
```dart
await editor.rotatePages(const PdfPages.single(0), by: PdfRotation.r90);
await editor.rotatePages(const PdfPages.all(), by: PdfRotation.r90);
```

**`pageMediaBox`** — The page boxes are read with `pageInfo`.
Before:
```dart
final media = await editor.pageMediaBox(0);
final crop = await editor.pageCropBox(0);
```
After:
```dart
final info = (await editor.pageInfo(pages: const PdfPages.single(0))).single;
final media = info.mediaBox;
final crop = info.cropBox;
```

**`deletePage(int)`** — Deletion takes a `PdfPages`.
Before:
```dart
await editor.deletePage(0);
```
After:
```dart
await editor.deletePages(const PdfPages.single(0));
```

**`mergeFrom(source, {pages, password})`** — `pages` is a `PdfPages`, no longer a list of indexes.
Before:
```dart
await editor.mergeFrom(source, pages: [0, 2]);
```
After:
```dart
await editor.mergeFrom(source, pages: const PdfPages.list([0, 2]));
```

**`resizeImage`** — `resizeImage` and `repositionImage` are replaced by `setImageBounds`, which takes the size and the position together.
Before:
```dart
await editor.resizeImage(0, 'hero', width: 100, height: 80);
```
After:
```dart
final hero = (await editor.pageImages(0)).firstWhere((image) => image.name == 'hero');
await editor.setImageBounds(
  0,
  'hero',
  PdfRect(x: hero.bounds.x, y: hero.bounds.y, width: 100, height: 80),
);
```

**`addWatermark(page, text)`** — The text comes first, the pages are a `PdfPages` that defaults to every page, and a page of `-1` no longer means every page.
Before:
```dart
await editor.addWatermark(0, 'DRAFT');
await editor.addWatermark(-1, 'DRAFT');
```
After:
```dart
await editor.addWatermark('DRAFT', pages: const PdfPages.single(0));
await editor.addWatermark('DRAFT', pages: const PdfPages.all());
```

**`PdfPageInfo`** — A `PdfPageInfo` is built from `mediaBox` and `cropBox`; `width` and `height` stay as getters.
Before:
```dart
const info = PdfPageInfo(index: 0, width: 595, height: 842);
```
After:
```dart
const info = PdfPageInfo(
  index: 0,
  mediaBox: PdfRect(x: 0, y: 0, width: 595, height: 842),
);
```

**`render`** — `pages` is optional, so leave it out to read every page.
Before:
```dart
await doc.render(pages: const PdfPages.all()).drain<void>();
```
After:
```dart
await doc.render((shot) => MemorySink());
```

**`signatures`** — `signatures`, `formFields`, `xfa` and `attachments` are methods.
Before:
```dart
final signatures = await doc.signatures;
final fields = await doc.formFields;
final xfa = await doc.xfa;
final attachments = await doc.attachments;
```
After:
```dart
final signatures = await doc.signatures();
final fields = await doc.formFields();
final xfa = await doc.xfa();
final attachments = await doc.attachments();
```

**`PdfDoc.formField(name)`** — `formField` is gone, so find the field by its name in `formFields()`.
Before:
```dart
final field = await doc.formField('name');
```
After:
```dart
final fields = await doc.formFields();
final field = fields.where((candidate) => candidate.name == 'name').firstOrNull;
```

**`validatePdfUa`** — PDF/UA validation answers a `PdfValidationResult`, where it answered a bool, and takes a `PdfUaLevel`.
Before:
```dart
final valid = await doc.validatePdfUa(level: 1);
```
After:
```dart
final result = await doc.validatePdfUa(level: PdfUaLevel.ua1);
final valid = result.compliant;
```

**`planSplitByBookmarks()`** — The call is `bookmarkRanges()`, and each result has `start` and `end` where it had `startPage` and `endPage`.
Before:
```dart
final plan = await doc.planSplitByBookmarks();
final first = plan.first.startPage;
```
After:
```dart
final ranges = await doc.bookmarkRanges();
final first = ranges.first.start;
```

**`classifyPage(page)`** — `classifyPages` takes a page set, and a page's kind is a `PdfPageType`, where it was a string.
Before:
```dart
final kind = (await doc.classifyPage(0)).type;
```
After:
```dart
final kind = (await doc.classifyPages(pages: const PdfPages.single(0))).first.type;
```

**`PdfDoc.internal`** — The handle constructors are `@internal`; no code changes, since `pdf.open`, `pdf.edit` and `pdf.build` build the handles.
Before:
```dart
final opened = await pdf.open(source);
final editing = await pdf.edit(source);
final building = await pdf.build();
```
After:
```dart
final opened = await pdf.open(source);
final editing = await pdf.edit(source);
final building = await pdf.build();
```

**`addStamp(page, …)`** — Stamps and flattening take a `PdfPages` where they took an `int` page.
Before:
```dart
await editor.addStamp(0, type: PdfStampType.draft, rect: rect);
await editor.flattenForms(page: 0);
```
After:
```dart
await editor.addStamp(const PdfPages.single(0), type: PdfStampType.draft, rect: rect);
await editor.flattenForms(pages: const PdfPages.single(0));
```

**`setFormFieldValue`** — The form-field edits are on `editor.field(name)`.
Before:
```dart
await editor.setFormFieldValue('name', 'Ada');
```
After:
```dart
await editor.field('name').setValue(const PdfFormFieldValue.text('Ada'));
```

**`PdfChoiceValue` (choice fields)** — A combo box or list box reads as `PdfChoiceValue`, where it read as `PdfTextValue`.
Before:
```dart
final fields = await doc.formFields;
final country = fields.firstWhere((field) => field.name == 'country');
final value = country.value;
final chosen = value is PdfTextValue ? value.text : null;
```
After:
```dart
final fields = await doc.formFields();
final country = fields.firstWhere((field) => field.name == 'country');
final value = country.value;
final chosen = value is PdfChoiceValue ? value.choice : null;
```

**`addRedaction(page, rect, {overlayText})`** — The call is `markRedaction` with an overlay; the engine never drew the overlay text, so pass a fill colour for a box.
Before:
```dart
await editor.addRedaction(0, rect, overlayText: 'REDACTED');
```
After:
```dart
await editor.markRedaction(0, rect, overlay: const PdfRedactionOverlay.fill(PdfColor.black));
```

**`eraseRegions`** — `eraseRegions` and `clearEraseRegions` are replaced by `redact` with no overlay and `clearRedactions`.
Before:
```dart
await editor.eraseRegions(0, [rect]);
await editor.clearEraseRegions(0);
```
After:
```dart
await editor.redact(0, [rect], overlay: const PdfRedactionOverlay.none());
await editor.clearRedactions();
```

**`convertToPdfA`** — `convertToPdfA` is a member of the new `PdfEdits` base class that `PdfEditor` extends, and its level is a `PdfALevel`.
Before:
```dart
await editor.convertToPdfA(level: 1);
```
After:
```dart
final PdfEdits edits = editor;
await edits.convertToPdfA(level: PdfALevel.a1);
```

**`deletePages`** — The one-shot is removed; call the edit on `pdf.edits`.
Before:
```dart
await pdf.deletePages(source, output, pages: [0, 2]);
```
After:
```dart
await pdf.edits(source, output).deletePages(const PdfPages.list([0, 2]));
```

**`encrypt`** — The one-shot is removed; `resave` writes the document again with encryption in its save options.
Before:
```dart
await pdf.encrypt(source, output, encryption: const PdfEncryptionConfig(ownerPassword: 'secret'));
```
After:
```dart
await pdf.resave(
  source,
  output,
  options: const PdfSaveOptions.fullRewrite(
    encryption: PdfEncryption.config(ownerPassword: 'secret'),
  ),
);
```

**`extractPages`** — The one-shot is removed; call `keepPages` on `pdf.edits`.
Before:
```dart
await pdf.extractPages(source, output, pages: [0, 2]);
```
After:
```dart
await pdf.edits(source, output).keepPages([0, 2]);
```

### Building

**`split`** — `split`, `splitBySize` and `splitByBookmarks` are one `split` with a `PdfSplitRule`, and `sinkFor` takes the part it is for.
Before:
```dart
await pdf.split(source, (index) => MemorySink(), every: 2);
```
After:
```dart
await pdf.split(source, (part) => MemorySink(), by: PdfSplitRule.every(2));
```

**`merge(sources, output, {passwords})`** — A password-protected source is opened with its entry of `passwords`.
Before:
```dart
await pdf.merge(sources, output);
```
After:
```dart
await pdf.merge(sources, output, passwords: {sources.first: 'secret'});
```

**`compress` (garbage collection)** — `compress` always collects garbage, so it has no `garbageCollect` parameter.
Before:
```dart
await pdf.compress(source, output, garbageCollect: true);
```
After:
```dart
await pdf.compress(source, output);
```

**`registerFallbackFont(kind, font)`** — The font is a `DataSource`, where it was `Uint8List` bytes.
Before:
```dart
await pdf.registerFallbackFont(kind, bytes);
```
After:
```dart
await pdf.registerFallbackFont(kind, MemorySource(bytes));
```
