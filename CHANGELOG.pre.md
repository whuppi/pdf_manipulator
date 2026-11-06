# Prerelease Changelog

<!--
═══════════════════════════════════════════════════════════════════════
CHANGELOG STANDARD — read before editing. Applies to both changelogs.
═══════════════════════════════════════════════════════════════════════
Two INDEPENDENT lane changelogs — do NOT mirror one from the other:
  • CHANGELOG.pre.md — the prerelease lane (`## X.Y.Z-dev.N`). Add an
    entry per prerelease you cut on dev.
  • CHANGELOG.md — the stable lane (`## X.Y.Z`). Add an entry per stable
    release, CONSOLIDATING the prerelease entries that ship under it.
They share prose but track their OWN version sequences. There is no
`cp + sed` regen: that mirror falsely assumed every prerelease becomes a
same-numbered stable, so it manufactured stable headings for versions
that never shipped — which `tool/ci/release.sh --check-versions` now flags.
Hand-edit each lane's file directly.

VERSION NUMBERS ARE SEMVER — the bullets decide the number
  Write the entry FIRST, then read it back; the bullets dictate the bump:
  • Any **Breaking:** bullet → MAJOR. No exceptions — not "it's tiny",
    not "nobody uses that knob". Caret consumers (^X.Y.Z) auto-upgrade
    through minors and patches, so a breaking minor/patch ambushes them.
  • Added / Changed capability → MINOR.
  • Only Fixed bullets → PATCH.
  If the heading you were about to write disagrees with its own bullets,
  the heading is wrong — renumber before merging. Precedent: 2.2.0
  shipped a **Breaking:** bullet as a minor and had to be retracted;
  never again.

ADDING A VERSION
  Add a heading at the TOP (newest first) of the right lane's file and
  write the summary. Exactly ONE new (untagged) version may sit at the
  top of each file — every heading BELOW it must already have its git tag
  (or a verified `release: no-tag` HTML-comment directive). `--check-versions`
  enforces this at PR + release time: a second un-released version is
  rejected, since it would collapse into the one release the merge cuts.
  Versions, commit lists, tags, publishing — the release tooling owns all
  of it; you only write the human summary.

ENTRY SHAPE
  ## X.Y.Z-dev.0
  <one-line prose lead — only to frame a big release or signal "no
   behavior change"; omit when the bullets speak for themselves>
  - **Breaking:** <what changed> → <migration step, INLINE>   ← always first
  - <upgrade action, e.g. "re-run setup --force web">         ← any required action next
  - Added/Changed <capability or improvement>                ← then improvements
  - Fixed <symptom> — <the fix, one clause> ([#N](issue-url) reported by [@user](abs-url))  ← fixes last

  Order IS the grouping — Breaking → action → added/changed → fixed. No
  `###` subsections: bullet order carries the categories. Only Breaking
  is bold-tagged; everything else is verb-led. Fixes start with "Fixed".

BULLET SHAPE — one change, one sentence
  A reader asks two things: does this touch me, and what do I change.
  Answer those and stop.
  • `<Verb> <what the reader sees changed> — <one clause: the fix or
    the why> (<issue credit>)`. A Fixed bullet names the symptom first,
    as the user met it (the error text, the wrong result), then the fix.
  • One sentence. A **Breaking** bullet may add a second: the migration.
    No trailing period.
  • Under 40 words is the target; 60 is the wall, 80 for a Breaking
    bullet whose migration is inline. `make test-guards` counts.
  • The cause, the mechanism, the spec citation and the proof belong in
    the PR, which the "Commits since" list under every entry already
    reaches. A bullet that explains why the bug existed is a PR body in
    the wrong file.
  • Short is not vague: "Fixed image optimization" is short and useless;
    the symptom and the change must both survive the cut.

  EXCEPTION — the genesis entry (a ground-up rewrite, no prior version)
  uses facet tags instead of deltas: **Engine:** / **API:** / **I/O:** /
  etc., describing the new package's dimensions. See the 1.0.0 entry.

CONTENT RULES (never change)
  • Migrate from the entry ALONE — breaking changes inline, old → new.
    (pub.dev freezes each version's CHANGELOG as a snapshot, so an entry
    can't rely on anything that later moves.)
  • NEVER link a living doc (README, docs/*) from an entry — it rots when
    the doc moves on. The migration guide is reached from the README.
  • Links point only at IMMUTABLE targets. Credit the issue and its
    reporter when a reported issue drove the change:
    ([#N](https://github.com/whuppi/pdf_manipulator/issues/N) reported by
    [@user](https://github.com/user)). No PR links in bullets: the release
    tooling appends a "Commits since" list with every PR number under
    each entry, so the trail exists without lengthening the line. A PR in
    another repository (a contributor's engine fix) may be linked as the
    credit, since no commit list here reaches it.
  • No capability inventories — "what's shipped" lives in README +
    docs/CAPABILITY_ROADMAP.md; the changelog says only what CHANGED.
  • Engine/submodule bump → web re-fetch action (NEVER miss this). When a
    release bumps a vendored engine submodule (vendor/pdf_oxide,
    vendor/office_oxide), the web WASM is rebuilt and consumers must
    re-fetch it, so ALWAYS add the action bullet:
      - Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
    Native self-updates via the build hook; only web needs the manual step.
    When cutting a release, diff the submodule pointer against the previous
    tag (`git ls-tree <prev-tag> vendor/pdf_oxide`) so an engine bump never
    ships without the bullet.
═══════════════════════════════════════════════════════════════════════
-->

<!-- Add new versions below, newest first. -->

## 6.0.0-dev.0

- **Breaking:** `DataSource` has a new `close()`, and `DataSink` has new `abort(Object reason)` and `close()`, where the library calls `abort` on the sink of an operation that ends cancelled or failed → implement all three in your own sources and sinks (clear a buffer, truncate a file, cancel an upload); the built-in ones do
- **Breaking:** `FileSource` and `FileSink.create` are `await FileSource.open(file)` and `await FileSink.open(file)`, and whoever opens a source or sink closes it → await the factory and call `close()` when the job is over
- **Breaking:** `open`, `edit`, `merge`, `split`, `compress`, `sign` and every other call that opens a PDF take a `PdfInput` where they took a `DataSource` and a `password:` → write `PdfInput(source, password: 'x')`; `PdfFile(file)` and `PdfInput.owning` open and close the source for the job
- **Breaking:** `PdfDoc` is `PdfDocument`, and `pdf.open`, `pdf.edit` and `pdf.build` return the owning `PdfDocumentHandle`, `PdfEditorHandle` and `PdfBuilderHandle`, the only types with `dispose` and `save` → rename the type; `pdf.read`, `pdf.rewrite` and `pdf.buildTo` dispose for you
- **Breaking:** the one-shots `deletePages`, `reorderPages`, `movePage`, `rotatePages`, `rotateAllPages`, `flattenForms`, `embedFile`, `watermark`, `addStamp`, `addImageStamp` and `convertToPdfA` are removed → call the edit on `pdf.editOnce(input, output)`, for example `.removePages(PdfPages.single(0))`, or put several edits in one `pdf.rewrite`
- **Breaking:** the `applyRedactions` one-shot is removed → call `applyRedactions()` in a `pdf.rewrite` body
- **Breaking:** `eraseRegions` and `clearEraseRegions`, on the editor and as a one-shot, are removed → call `redact(page, regions, overlay: const PdfRedactionOverlay.none())`, or `clearRedactions()` to drop queued marks
- **Breaking:** the `encrypt` and `decrypt` one-shots are removed → call `pdf.resave(input, output, save: PdfSave.full(encryption: PdfEncryption.password(ownerPassword: 'x')))`, or `PdfEncryption.remove()` with `PdfInput(source, password: 'x')` to decrypt
- **Breaking:** `extractPages` is removed → call `pdf.editOnce(input, output).keepPages(PdfPages.list([0, 2]))` to keep pages in document order, or `reorderPages(PdfPageOrder([2, 0]))` for another order
- **Breaking:** `split`, `splitBySize` and `splitByBookmarks` are one `split(input, sinkFor, by: ...)` with `PdfSplitRule.every(n)`, `.maxBytes(n)` or `.bookmarks()`, and `sinkFor` takes the `PdfSplitPart` it is for → rename the call; it returns the parts (index, first page, end page, bookmark title) and no byte sizes
- **Breaking:** `compress` has no `garbageCollect` parameter, since it always collects garbage, and its `images` default is `PdfImagePolicy.lossless` where it was `ebook` → drop the argument, and pass `images: PdfImagePolicy.ebook` for the old result
- **Breaking:** `imagesToPdf` gives every page the image's own size, one point per pixel, where it drew each image on an A4 page → pass `page: const PdfImagePage(size: PdfImagePageSize.fixed(PdfPageSize.a4()))` to keep A4 pages
- **Breaking:** `convertTo` and `convertToPdf` are `convertToOffice(input, output, format:)` and `convertFromOffice(document, output, format:)`, and `PdfDocumentFormat` is `PdfOfficeFormat` → rename the calls and the type
- **Breaking:** `registerFallbackFont` takes the font as a `DataSource`, which it reads whole, where it took `Uint8List` bytes → wrap the bytes: `MemorySource(bytes)`
- **Breaking:** the `title`, `author`, `subject`, `keywords`, `creator`, `producer` and `creationDate` of `PdfDoc` are fields of `doc.metadata` (`PdfMetadata`), and `creationDate` is a UTC `DateTime?` where it was a raw PDF date string → read `doc.metadata.title`
- **Breaking:** the `isEncrypted`, `requiresPassword`, `encryptionAlgorithm` and `permissions` of `PdfDoc` are fields of `doc.encryption` (`PdfEncryptionInfo`), which is null when the document is not encrypted → read `doc.encryption?.permissions`
- **Breaking:** `PdfDoc.pages` is gone, and opening a document no longer reads the pages → call `await doc.pageInfo()`, or `doc.pageInfo(pages: PdfPages.single(0))` for one page
- **Breaking:** `PdfPageInfo` has `page`, `mediaBox`, `cropBox` and a `PdfRotation` `rotation`, where it had `index`, an `int` `rotation` and `width` and `height` as fields (they stay as getters) → read `info.page`, and compare `info.rotation` with `PdfRotation.r90`
- **Breaking:** `extract({pages, format})` is `text({pages, format})`, which returns the string, and `extractText(output, {pages, format})`, which writes it to a sink as the engine produces it → call `await doc.text()`; `pages` is optional
- **Breaking:** `search(query:, pages:)` is `search(query, {pages})`, and `SearchResult` is `PdfSearchHit` with the same fields → drop the label and rename the type
- **Breaking:** `render` returns a `Future` of the `PdfRenderedPage` descriptors and writes each page to the sink that `sinkFor` returns, where it returned a `Stream` of `RenderedPage` with the bytes; `pages` is optional → pass `(page) => MemorySink()`, or call `doc.bitmap(0)` for one page's bytes
- **Breaking:** `extractImages` returns a `Future` of `PdfImageInfo` and writes each image to the sink that `sinkFor` returns, where it returned a `Stream` of `PdfImage` with the bytes; `pages` is optional → pass `(image) => sink`
- **Breaking:** `PdfRenderSize(maxWidth:, maxHeight:)` and `PdfRenderSize.thumbnail(n)` are `PdfRenderSize.fit(maxWidth:, maxHeight:)`, and `PdfRenderSize.dpi(n)` is new → rename the constructor; `thumbnail(n)` is `fit(maxWidth: n, maxHeight: n)`
- **Breaking:** `signatures`, `formFields`, `xfa` and `attachments` are methods now, where they were getters → add the parentheses
- **Breaking:** `PdfDoc.formField(name)` is gone → find the field in `await doc.formFields()`
- **Breaking:** `validatePdfA` and `validatePdfUa` answer a `PdfValidationReport` (`isCompliant`, `errorCount`, `warningCount`) and take a `PdfALevel` or `PdfUaLevel`, where `PdfValidationResult` had `compliant`, `errors` and `warnings`, `validatePdfUa` answered a bool and `level` was an `int` → read `report.isCompliant`
- **Breaking:** `convertToPdfA` takes a `PdfALevel` (`a1`, `a2`, `a3`) and defaults to `a2`, where it took an `int` and defaulted to level 1 → pass `level: PdfALevel.a1` for the old default
- **Breaking:** `planSplitByBookmarks` is `bookmarkRanges()`, and `PdfBookmarkSplit` with `startPage` and `endPage` is `PdfBookmarkRange` with `start` and `end` → rename the call and the fields
- **Breaking:** `classifyPage` is `classifyPages({pages})`, a page's `type` string and `confidence` are a `kind` (`PdfPageKind`), and `classifyDocument` answers a `PdfDocumentKind` where it answered a `PdfDocumentClassification` → read `result.first.kind`
- **Breaking:** `PdfFormField` has `kind` (`PdfFormFieldKind`, with `textField`, `pushButton` and `signatureField`), `isReadOnly` and `isRequired`, where it had `type` (`PdfFormFieldType`), `readOnly` and `required`; `PdfCheckedValue.checked` is `isChecked` → rename the fields and the type
- **Breaking:** a radio group's value reads as `PdfChoiceValue` with the selected option's name, or `PdfNoValue` when none is selected, where it read as `PdfCheckedValue` → match on `PdfChoiceValue` for radio groups
- **Breaking:** a combo box's or list box's value reads as `PdfChoiceValue` (`PdfMultiChoiceValue` for several options), where it read as `PdfTextValue` → match on `PdfChoiceValue` for choice fields
- **Breaking:** `size` of `PdfAttachment` is `sizeBytes`, `serial` of `PdfCertificateInfo` is `serialNumber`, and `regions` of `PdfRedactionReport` is `regionCount` → rename the fields
- **Breaking:** `PdfSignatureInfo` has a required `coversWholeDocument` parameter → pass it where you construct one, in test fakes for example
- **Breaking:** the fields of `PdfPermissions` are `canPrint`, `canPrintHighQuality`, `canModify`, `canCopy`, `canAnnotate`, `canFillForms`, `canExtractForAccessibility` and `canAssemble`, where they were `print`, `printHq`, `modify`, `copy`, `annotate`, `fillForms`, `accessibility` and `assemble`, and `toBits` is gone → rename the arguments and the fields
- **Breaking:** `monoPpi` and `minSavings` of `PdfImagePolicy` are `bilevelPpi` and `minSavingsRatio`, and `PdfImageOutcome` has `bitsPerComponent`, `isIndexed`, `minPpi` and `placementCount` where it had `bits`, `indexed`, `ppiMin` and `uses` → rename the fields
- **Breaking:** `rotation` of `PdfWatermarkStyle` is `angleDegrees` → rename the argument
- **Breaking:** `PdfExtractionFormat.plainText` is `plain` and `.text` is `layout`, and `PdfEncryptionAlgorithm` has `rc4Key40`, `rc4Key128`, `aesKey128` and `aesKey256` where it had `rc4_40`, `rc4_128`, `aes128` and `aes256` → rename the values
- **Breaking:** `PdfConfig(webLaneWorkerUrl:, maxLanes:)` is `PdfConfig(webWorkerUrl:, concurrency:)` → rename the arguments; `concurrency` defaults to `max(2, cores / 2)` operations at once
- **Breaking:** `PdfEditor.pageCount`, `version` and `isModified` are methods → write `await editor.pageCount()`
- **Breaking:** the `setTitle`, `setAuthor`, `setSubject`, `setKeywords`, `setProducer` and `setCreationDate` of the editor and the builder are one `setMetadata(PdfMetadata)`, which replaces the standard fields, and the editor reads them with `metadata()` → change one with `setMetadata((await editor.metadata()).copyWith(title: 'X'))`
- **Breaking:** `rotatePage`, `rotateAllPages` and `setPageRotation` are `rotatePages(by:, pages:)` and `setRotation(rotation, pages:)`, which take a `PdfRotation` (`r0`, `r90`, `r180`, `r270`) where they took `degrees:` → call `rotatePages(by: PdfRotation.r90)`
- **Breaking:** `pageMediaBox`, `pageCropBox`, `setPageMediaBox` and `setPageCropBox` are `pageInfo({pages})` with `mediaBox` and `cropBox`, `setMediaBox(box, {pages})` and `setCropBox(box, {pages})` → a null crop box removes the crop box
- **Breaking:** `deletePage` is `removePages(PdfPages)`, which throws `PdfEngineFailed` for a page the document does not have, or for every page → call `removePages(PdfPages.list([1, 3]))`
- **Breaking:** `selectPages` is `keepPages(PdfPages)`, which keeps the pages in document order, and `reorderPages(PdfPageOrder)`, which keeps them in the order given → call `reorderPages(PdfPageOrder([2, 0, 1]))` to keep the old order
- **Breaking:** `mergeFrom(source, {pages})` is `appendPages(PdfInput, {pages})`, and `pages` is a `PdfPages` where it was a list of indexes → pass `PdfInput(source, password: 'x')` for an encrypted source
- **Breaking:** `addWatermark(page, text)` is `addWatermark(text, {pages})` with every page as the default, and a page of `-1` no longer means every page → pass `pages: const PdfPages.all()`
- **Breaking:** `addStamp(page, type:, rect:)` and `addImageStamp(page, image, rect:)` are `addStamp(stamp, bounds:, pages:)` and `addImageStamp(image, bounds:, pages:)`, and `PdfStampType` is `PdfStamp` → write `addStamp(PdfStamp.draft, bounds: r)`
- **Breaking:** `embedFile` is `addAttachment` → rename the call
- **Breaking:** `flattenForms({int? page})` and `flattenAnnotations({int? page})` take `pages` (a `PdfPages`, every page by default) → write `flattenForms(pages: PdfPages.single(0))`
- **Breaking:** `addRedaction` is `markRedaction(page, regions, {overlay})`, since the engine never drew the overlay text → pass `PdfRedactionOverlay.fill(color)` for a box of that colour, or `PdfRedactionOverlay.none()`
- **Breaking:** the editor's `setFormFieldValue`, `setCheckboxFieldValue`, `removeFormField` and the other `setFormField…` methods are on `editor.field(name)` as `setValue`, `setReadOnly`, `setRequired`, `setTooltip`, `setBounds`, `setMaxLength`, `setAlignment`, `setBackgroundColor`, `setBorderColor`, `setBorderWidth`, `setAppearance`, `setFlags` and `remove` → write `editor.field('name').setValue(const PdfFormFieldValue.text('Ada'))`
- **Breaking:** `resizeImage` and `repositionImage` are removed, and `pageImages` returns `PdfImagePlacement` where it returned `PdfPageImage` → call `setImageBounds(page, name, bounds)` with the size and position you want
- **Breaking:** `scrubMetadata` and `sanitize(PdfSanitizeOptions)` are `sanitize({stripMetadata, stripJavaScript, stripAttachments})`, where `PdfSanitizeOptions` had `metadata`, `javascript` and `embeddedFiles` → call `sanitize(stripJavaScript: false)` for `scrubMetadata()`
- **Breaking:** `PdfSaveOptions` is `PdfSave`, and `PdfSaveOptions.fullRewrite(compress:, garbageCollect:, encryption:)` is `PdfSave.full(deflate:, garbageCollect:, encryption:)` with a `PdfDeflate` level; `save(output, options:)` is `save(output, save:)` and exists only on the handles → rename the class and the arguments
- **Breaking:** `PdfSaveOptions` with `incremental()` now appends the edits to the original bytes, so a signature over them stays valid, and throws `PdfIncrementalRefused` when an append cannot carry them (pages removed or reordered, redaction, content edits, an encrypted or repaired source) → catch it and save with `PdfSave.full()`
- **Breaking:** `PdfEncryption.config` is `PdfEncryption.password(ownerPassword:, userPassword:, algorithm:, permissions:)`, whose `userPassword` is null for a document that opens without one and whose `algorithm` defaults to `aesKey256` → rename the call
- **Breaking:** the subclasses `PdfPagesAll`, `PdfPagesSingle`, `PdfPagesList`, `PdfPagesRange`, `PdfEncryptionKeep`, `PdfEncryptionRemove`, `PdfEncryptionConfig`, `PdfSaveFullRewrite`, `PdfSaveIncremental`, `PdfWatermarkCenter`, `PdfWatermarkCorner`, `PdfWatermarkTiled`, `PdfWatermarkExact`, `PdfPkcs12Credentials` and `PdfPemCredentials` are not exported → call the factory, for example `PdfPages.single(0)` or `PdfEncryption.keep()`
- **Breaking:** `PdfColor` and every result and options type (`PdfPageInfo`, `PdfImagePolicy`, `PdfRect` and the rest) are `final` with value `==` and `hashCode` → replace an `extends` or `implements` of one with a wrapper
- **Breaking:** `PdfPages.list` is not `const`, sorts the indexes, and throws `ArgumentError` for an empty list, a negative index or an index given twice, where it kept the list as given; `PdfPages.from(start)` is new → write `PdfPages.list([3, 1])`
- **Breaking:** the `bottom` of a `PdfRect` is its lower edge, `y`, where it was `y + height` → read `top` for the upper edge; `PdfRect.fromEdges` builds a rectangle from `left`, `bottom`, `right` and `top`
- **Breaking:** `PdfInvalidArgument` is removed: a programmer mistake (an empty list, a negative page, a second call on a one-shot) throws `ArgumentError` or `StateError` before any I/O, and an engine refusal throws `PdfEngineFailed` → catch `PdfError` for runtime failures only
- **Breaking:** `PdfIoError`, `PdfEngineError`, `PdfPageRangeError` and `PdfCryptoError` are `PdfIoFailed`, `PdfEngineFailed`, `PdfPageOutOfRange` and `PdfCryptoFailed` → rename the type in each `catch` and `switch`
- **Breaking:** `PdfSearchError` and `PdfExtractionFailed` are removed, since nothing threw them → delete their `catch` clauses and `switch` cases; engine failures arrive as the other `PdfError` subtypes
- **Breaking:** `PdfError` has two new subtypes, `PdfNotFound` (a missing attachment or image) and `PdfIncrementalRefused` → add a case to every exhaustive `switch` over `PdfError`
- **Breaking:** `addA4Page`, `addLetterPage` and `addPage(width:, height:)` are `addPage([PdfPageSize])` → call `addPage()`, `addPage(PdfPageSize.letter())` or `addPage(PdfPageSize.points(400, 300))`
- **Breaking:** the page builder's `font` and `at` are `setFont(name, size)` and `moveTo(PdfPoint(x, y))`, and `done()` and `newPageSameSize()` are removed → a page ends when the next is added or the document is saved
- **Breaking:** the page builder's `fieldKeystroke`, `fieldFormat`, `fieldValidate` and `fieldCalculate`, and its `linkUrl` and `linkPage`, are arguments of the call that writes the field or text → pass `scripts: const PdfFieldScripts(format: 'x')` or `link: const PdfLink.url('https://example.com')`
- **Breaking:** the page builder's `textField` takes `value` where it took `defaultValue`, `checkbox` takes `isChecked` where it took `checked`, and `radioGroup` takes `PdfRadioOption(value, bounds)` where options were records → rename the arguments
- **Breaking:** `PdfTextAlignment` is `PdfAlignment`, and the page builder's `image(data, rect, {altText})` is `image(data, bounds, {altText})` where a null `altText` places an artifact → rename the type; pass `altText: 'x'` for a described image
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added `BlobSource` (`package:pdf_manipulator/web.dart`), which the worker reads in place with `FileReaderSync` in JSPI, Atomics and Asyncify, and copies to OPFS itself in OPFS mode: a `File` from a picker opens with no main-thread read and O(1) memory at any size
- Changed web sources of any other kind that fit in 16 MiB per lane to be copied into the worker once as a snapshot at job start instead of pre-copied to disk or read through the page, and a source past that budget takes the I/O mode's own path
- Added `redact(page, regions, {overlay})`, which marks and applies its own regions in one step and returns the report — marks queued before stay queued, and an error leaves nothing marked or applied
- Added `PdfFormFieldValue.choice`, `.choices` and `.none`: `field(name).setValue` selects an option of a combo box or list box, the options of a multi-select list box, or clears the value, and refuses an option the field does not list
- Added `PdfEditor.clearRedactions()`, and `field(name).setTooltip`, `setMaxLength`, `setBackgroundColor` and `setBorderColor` take null to remove the tooltip, the limit or the colour
- Added `PdfRedactionOverlay`, `fill(color)` or `none()`: a redaction with no overlay removes the content and draws no box
- Added `PdfDocument.annotations(page)`, which lists a page's annotations (subtype, rectangle, contents, author, form field name) as `PdfAnnotationInfo`
- Added the page builder methods the engine already had: `appendText`, `textIn`, `rect`, `line`, `stickyNote`, `stamp`, `freeText`, `onOpen`, `onClose` and `table`, and a `PdfMarkup` or `PdfLink` argument on `text`
- Added a `PdfTextStyle` to `text`, `stamp(PdfStamp...)`, `table(columns, rows, {options})` with `PdfTableCell` and `PdfTableOptions`, and `stickyNote(..., at:)` to the page builder
- Added `PdfBuilder.addImagePage`, a page that holds one picture and takes the picture's own size unless you name one
- Added the web Asyncify mode, `asyncify: true` under `hooks: user_defines: pdf_manipulator:`, which also ships the Asyncify wasm so a browser with neither JSPI nor `SharedArrayBuffer` streams instead of copying to storage first — it is 28 MB for the full engine and is fetched only in this mode, and only where OPFS cannot run
- Changed the web wasm to ship as parts of 16 MiB listed in `pdf_oxide_manifest.json`, which the browser joins into one stream as it compiles, so no file reaches the 25 MiB limit of static hosts; apps that list `pdf_oxide_bg.wasm` re-run setup and list the parts
- Changed the web mode the package picks by itself to Atomics first where the page has `SharedArrayBuffer`, then JSPI, OPFS and Asyncify, since Atomics measured faster than JSPI on every source
- Changed `PdfBuilder.save` to Flate-compress page content and font streams; `save(output, deflate: false)` keeps them plain for debugging
- Changed `open`, `edit`, `split`, a full-rewrite save and `compress` to hold a bounded window of the document instead of all of it: a 38 MB, 10,000-page file now opens and saves in under 16 MB of engine memory, where it took up to 85 MB
- Changed `appendPages` to keep the merged file open until the editor is disposed and copy its objects one at a time at save: merging a 10,000-page file takes under 16 MB of engine memory and about 2 seconds, where it took 163 MB and minutes
- Added rotation, media box and crop box edits on merged pages; stamps, watermarks, erases and redactions on a merged page throw `PdfUnsupported`
- Changed page edits to work on merged pages: `removePages`, `movePage` and `keepPages` take a merged page like any other, where they threw `PdfUnsupported` on a document with merged pages
- Changed reading a source to fetch it in 64 KB pieces instead of one round trip per read: compressing a 70-page scan takes 18 ms on native, where it took 29 ms, and a third less time on the web
- Changed `extractText` of all pages to stream one page at a time from the engine: extracting text from a 10,000-page file takes under 16 MB of engine memory, where its first call held 281 MB
- Changed every engine worker, native and web, to be retired once an operation's heap peaked past 32 MB, where only a web worker past 256 MB of WebAssembly memory was, and the machine's font caches no longer count against a worker
- Changed `registerFallbackFont` so a second registration of a kind replaces the first and reaches only the `Pdf` that made it, where the font was shared by every instance of the process
- Changed `render` and `extractImages` to deliver each page or image to its sink as the engine produces it, where they held every page until the last; cancelling the task stops the engine
- Changed cancellation to stop text extraction, search, validation, `reduceImages`, `convertToPdfA` and saves within one page, where it waited for the next read or write; on the web this needs a cross-origin isolated page
- Changed cancellation to reach `merge`, `split`, `compress`, saves, rendering, `bookmarkRanges`, `classifyPages`, PDF/A conversion and the opening of a damaged file inside their loops, where each ran to its end
- Changed `edit` to open the source once; an editor used to hold the document twice
- Changed a full-rewrite save to copy each unchanged stream from the source in 64 KB pieces, so a page holding a 60 MB image saves in under 1 MB of engine memory
- Changed a full-rewrite save to pack objects into object streams with a cross-reference stream (PDF 1.5), which makes files smaller; `PdfSave.full(useObjectStreams: false)` writes the classic layout
- Added `PdfSave.full(deflate:)`, how hard a full save deflates its streams (`PdfDeflate.none`, `fast`, `balanced` or `best`), default `best`
- Changed a full-rewrite save to re-deflate Flate streams at the save's level, keeping a stream's source bytes when the result is not smaller
- Changed `compress` to write the source unchanged when its result is not smaller
- Changed `compress` and every full save to recompress copied page content, pack object streams fuller, drop orphan `/Length` objects, rewrite LZW as Flate and deflate like native zlib level 9: on 13 public test files a lossless `compress` is now smaller than pdf_document 12.0.0's on 12 and equal on 1, where it was up to 6 times larger
- Added `PdfSave.full(mergeDuplicates:)`, which writes one copy of each set of identical images, ICC profiles and embedded fonts across the document and every merged source, default false
- Changed `compress` to write one copy of identical images, ICC profiles and embedded fonts
- Added `PdfSave.full(pruneResources:)`, which drops the resource entries that no content names, default false
- Changed `compress` to drop the fonts, images, colour spaces and other resources a page lists but no content of it uses; a resource any content might use, or whose content cannot be fully read, is kept
- Fixed text extraction of a page with many separate text runs on one line taking minutes to hours — a million characters now extract in about three seconds
- Changed `PdfImagePolicy.screen` to re-encode a stored JPEG at its quality even when the image is not downsampled (`recompressJpeg: true`); `ebook` and `print` still keep stored JPEGs
- Changed `verifySignatures` to check each signature's digest and signer key, where it returned `false` for every document; `PdfSignatureInfo` has `isValid`, `coversWholeDocument` and `signingTime` filled
- Fixed an empty input to `merge` or any other multi-source op shifting every source after it, so the op read the wrong documents — an empty input now throws `PdfCorrupted`
- Fixed a full-rewrite save of a file with no `/Size` writing new objects over existing ones, and dropping an object it could not read without a word — it now throws `PdfCorrupted`
- Fixed a full-rewrite save of a file holding objects above generation 0 writing references other readers resolve to nothing — each reference now names the generation its object is written at
- Fixed `render` and `extractImages` returning an empty stream when the engine failed, a page past the end included — they throw the engine's error
- Fixed a full-rewrite save writing an object over 100 MB truncated — it throws instead
- Fixed a page index past the end reaching the engine as a huge number or a generic error — every operation that takes a page throws `PdfPageOutOfRange` with the page and the page count
- Fixed a save that keeps an encrypted document's encryption setting its owner password to the password it was opened with, so anyone who could open the result had owner rights and its permissions were void — the save now reuses the source's own encryption dictionary and key
- Fixed `PdfEncryptionInfo.permissions` reporting the wrong flags (a print-allowed file read as `canPrint: false`) — the engine's `/P` word is read at its spec bit positions
- Fixed a page tree whose inner nodes each hold one page reporting those nodes as pages — every node is read before it counts as a page
- Fixed article threads longer than 4,096 beads being cut short and tables wider than 128 columns being missed, and a hostile CMap or xref stream size aborting the engine — XMP metadata is now read as a stream of events
- Fixed text extraction of a page that names thousands of fonts holding every font at once (472 MB for 20,000 fonts, now 46 MB): font caches are bounded by bytes and a page reads the fonts it shows as it shows them
- Fixed text extraction of a tagged document holding its whole structure tree and every page's tagged content at once (1.75 GiB for 600,000 elements) — it walks the tree for the page it reads; XFA forms are parsed as a stream of events instead of read whole
- Fixed `compress` with `mergeDuplicates` never merging identical images, fonts or ICC profiles over 256 KiB in an encrypted source: they are compared decrypted, read in small pieces
- Fixed TrueType text rendering letters the PDF never named: "fi" shown as two glyphs drew the "fi" ligature, and other font substitutions changed what was drawn — the renderer draws exactly the glyphs the content names
- Changed `compress` to subset embedded TrueType and CID CFF fonts: outlines no content shows are emptied, glyph ids and tables kept, fonts of form fields left whole; a subset that is not smaller is not written
- Fixed watermarks placed in a corner, tiled or at an exact rectangle all landing at default spots — every position is placed where it says; an unknown corner name is refused
- Fixed a save that sets a form field's value, flattens the forms and redacts elsewhere on the page failing with "object referenced but unwritten" when unused objects are dropped
- Fixed `applyRedactions` keeping the drawing of a removed glyph when a flattened field's own Type 3 font drew it — the glyph's procedure leaves the file
- Fixed `rotatePages` and other per-page edits slowing down with the square of the page count on large documents
- Fixed a merged page losing the resources, page size and rotation it inherited from its source page tree, which could leave it blank
- Fixed a merge copying the source's whole page tree into the output when a merged page linked to a page that was not merged — such a link's target is written as null
- Fixed page edits on a document with merged pages hitting the wrong page, and `movePage` crashing the engine — they apply to the merged page or refuse with a typed error
- Fixed a failed `open` or `edit` keeping its source channel until the instance was disposed
- Fixed a native crash when a job was cancelled while its sink was still writing: the late write touched freed memory
- Fixed a cancelled read on a document held open sending its late answer to the next job on that document, which then read the wrong bytes
- Fixed an operation on a disposed document reaching another open document that happened to have the same engine id — it fails with "document not found"
- Fixed `cancel()` on a job still waiting behind another one waiting for that job to end — it resolves at once; on JSPI a cancel during a sink write no longer waits for the write
- Changed `cancel()` when the operation finished before the cancel landed: the task resolves with its result, so a finished edit is not reported as cancelled; an `open` or `edit` that finished is closed and reports `PdfCancelled`
- Fixed a cancelled or failed `reduceImages` leaving the images it had already re-encoded staged, so a later save wrote half the reduction — it stages nothing unless every image is done
- Fixed a cancelled or failed `convertToPdfA` leaving the editor half converted, or converted while the call reported the error — the editor is as it was before the call
- Fixed an `applyRedactions` refused on a later page leaving the earlier pages redacted while the call reported the error — it redacts every page or none
- Fixed `extractText` of all pages reporting success when its sink threw on the last chunk, with the text lost — it throws the sink's error
- Fixed an operation that ended cancelled or failed leaving its partial output in the sink — the sink is aborted, and a combinator that ends cancelled aborts its output even when the save had finished
- Fixed one failed web worker start making every later operation of that `Pdf` fail — the next operation starts a worker again
- Fixed a lane that panicked (native) or trapped (web) going on to serve jobs on broken engine state — the lane ends, operations on its documents fail with a typed error, and new work goes to a new lane
- Fixed `PdfPageBuilder.radioGroup` always failing with "unknown pageOp"
- Fixed a radio option named Yes, No, On or Off reading back as a checked flag instead of its name
- Fixed `formFields` misreading a field whose type, flags or value live on an ancestor field (a radio group read as a checkbox), and reporting a nameless parent as a field with an empty name
- Fixed page builder calls on a disposed or unknown builder being queued without an error — each call now applies at once and fails where it is made
- Fixed `keepPages`, `split` and `removePages` writing the pages they drop into the output, so the output kept its original size and the dropped pages' text, images and form values — a save now writes nothing only a dropped page reaches, and drops the links, bookmark targets and form fields that pointed at one ([#261](https://github.com/whuppi/pdf_manipulator/issues/261) reported by [@crurui](https://github.com/crurui))
- Fixed page labels shifting onto the wrong pages after `keepPages`, `split`, `removePages` or a reorder — each kept page keeps its label
- Fixed `PdfPageInfo.label` always being `null` — it now holds the page's label when the document has page labels
- Fixed `sign` dropping the document's bookmarks, form fields, page labels and every other catalog entry, and replacing an earlier signature — the signature now appends to the existing form and page ([#263](https://github.com/whuppi/pdf_manipulator/issues/263))
- Fixed `PdfSignatureInfo` showing a non-ASCII signer name, reason or location as garbled text — UTF-16 text strings are decoded
- Fixed `flattenForms()` and `flattenAnnotations()` of every page leaving pages unflattened after a page was deleted or moved, `addImageStamp` stamping the page the index named before the deletion, and a second image stamp on a page replacing the first
- Fixed a redaction applied after `redact` on the same page bringing back the text `redact` had removed — the page is redacted again over both sets of regions
- Fixed a save after `markRedaction` without `applyRedactions` drawing a black box over text it never removed, so the file looked redacted and the text could still be copied — an unapplied redaction is saved as a redaction mark (a `/Redact` annotation) and the page content is left as it was
- Fixed `applyRedactions` leaving the pixels of images under a redacted area in the file — a covered image is removed, a partly covered one is copied with the covered pixels filled, masks included, and other pages keep the original
- Fixed `render` ignoring line width, caps, joins, miter limit and dashes set through a graphics state resource (`gs`), which drew such lines 1 pt wide and solid
- Changed opening a damaged file (no cross-reference table, or a wrong one) and reading objects packed in object streams to scan the file in fixed windows: a 64 MB damaged file opens in under 16 MB of engine memory, and the 1 MB and 32 MB caps are gone
- Fixed `render` drawing outlined text (text rendering modes 1, 2, 5 and 6) filled in the fill colour — the glyph outline is stroked in the stroke colour and line style
- Fixed `render` drawing strokes and text painted with a tiling pattern in a flat colour — the pattern now shows inside the stroke or the glyphs
- Fixed fit and thumbnail sizes of `render` coming out smaller than asked on pages with a crop box
- Fixed images with fewer than 8 bits a sample and the TIFF predictor decoding to wrong pixels
- Fixed 16-bit images and images with several colours below 8 bits a sample decoding to wrong pixels when stored with a PNG or TIFF predictor
- Fixed `applyRedactions` leaving the removed text's font widths, Unicode mappings, Type 3 glyphs and structure alt text in the file — entries only the removed text used are cleared
- Fixed a form field flattened and redacted on one page keeping its value under the box — flattening now runs before redaction
- Fixed image bounds edits saving the page unchanged without an error when its content could not be rewritten — the save now fails
- Fixed `applyRedactions` leaving drawings, clip shapes, gradient meshes and form content under a redacted area in the file — they are removed or cut at the area's edge, and a form other pages show is copied for the redacted page only
- Changed redacting a JPEG to replace only the 8×8 blocks under the area, so the rest of the picture keeps its exact pixels; the copy drops embedded thumbnails
- Fixed `applyRedactions` dropping every annotation on a redacted page — only the annotations under a redacted area go, with their form fields
- Changed `applyRedactions` to rewrite each page one operator at a time while the document saves: redacting a page with 9 million operators takes under 1 MB of engine memory
- Fixed `applyRedactions` keeping removed text in alt text written inside page content, in CID font widths and in Type 3 glyphs with no `/Differences` name; its report now counts images, paths and forms; an image change on a redacted image is kept; a flattened CJK field redacts and saves
- Fixed `image()` with bytes that are not an image succeeding and drawing nothing — it throws `PdfEngineFailed`
- Fixed table rows with a `rowSpan` above 1 never drawing, and `PdfTableCell.rowSpan` documented as a column span — it is the rows a cell spans, each row still lists one cell per column, and a row with the wrong cell count throws `ArgumentError`
- Fixed `sign` writing a cross-reference table that other readers had to repair — each entry is now 20 bytes
- Fixed a full-rewrite save dropping objects the kept pages use (content streams, colour spaces, fonts, structure) on files with a nested page tree or many objects, leaving pages blank or text missing
- Fixed `compress` and every other full-rewrite save making a file with object streams up to six times larger — the save keeps its objects packed
- Fixed `compress` and every other full-rewrite save dropping the document's title, author, dates and file identifier unless the title was edited — the saved file keeps the source's `/Info` and `/ID`, and an edited title replaces the old `/Info` instead of leaving it behind
- Fixed the same input saving to different bytes on every run, so `compress` and every other rewriting save could not be compared or cached by hash — objects are written in id order, so only the random AES IVs of an encrypted save differ
- Fixed creating and disposing `Pdf` instances in a loop starting a new web worker or native thread for most of them — a disposed instance hands its worker or thread back for the next instance to reuse
- Fixed an operation whose last write failed or was cancelled still reporting success with its output cut short — the engine now flushes the final bytes before it answers, and a refusal fails or cancels the operation
- Fixed a web source read past 4 GiB returning the bytes at the offset modulo 4 GiB, with no error — the read offset crosses to the engine as a JS number, not a 32-bit integer
- Fixed a cancel not reaching a redaction, a text extraction of a form or a change of an image's bounds until the page or form it was reading ended, so a page of millions of operators could not be stopped — each now stops within one operator

## 5.0.1-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed a `keep:` list without `extract`, such as `keep: []`, failing to compile the engine with `cannot find search in crate` — the search module now compiles in every keep-set ([#256](https://github.com/whuppi/pdf_manipulator/issues/256) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed a PDF encrypted with Standard Security R2–R4 (RC4) failing to open on web with `compiled without 'legacy-crypto'` — the web engine now carries the same legacy encryption and ICC colour support as native ([#257](https://github.com/whuppi/pdf_manipulator/issues/257) reported by [@crurui](https://github.com/crurui))

## 5.0.0-dev.0

- **Breaking:** `PdfEditor.optimizeImages(quality:, minSize:)` is removed → use `reduceImages(PdfImagePolicy(jpegQuality: quality, minPixels: minSize, convertCmykToRgb: true))`; the count it returned is `report.changed`
- **Breaking:** `compress(imageQuality:)` → `compress(images:)`, a `PdfImagePolicy` defaulting to `PdfImagePolicy.ebook` (150 ppi, JPEG quality 75), so a plain `compress` now downsamples images drawn above 225 ppi; pass `PdfImagePolicy(jpegQuality: q)` for the old behaviour
- **Breaking:** the editor's reads are noun methods, never with a `get` prefix: `getTitle()` and the other metadata reads → `metadata()`, `getPageMediaBox(page)` → `pageInfo(pages:)`, and on `PdfDoc` `getSignatures()` → `signatures()`
- **Breaking:** `flattenAllAnnotations()` is `flattenAnnotations({int? page})`, and `flattenForms()` gains the same optional `page`; without it both flatten every page as before
- **Breaking:** `applyRedactions()` now returns a `PdfRedactionReport` (regions, glyphs removed, images modified or removed, paths pruned) instead of `void`; awaiting it still applies the redactions
- **Breaking:** `mergeFrom(other)` gains `pages:` (a 0-based list; `null` merges every page) and `embedFile(name, data)` gains `description:`, `mimeType:` and `relationship:`; positional callers are unchanged
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added `PdfEditor.pageImages(page)` — every image XObject drawn on a page with its resource name, bounds and transform; the name is what `setImageBounds` takes
- Added `PdfEditor.setImageBounds(page, name, bounds)` — move, or move and resize, an image by the name `pageImages` lists
- Added `PdfEditor.reduceImages(PdfImagePolicy)` — downsamples images to the resolution they are drawn at and re-encodes them by kind (JPEG, predicted Flate, CCITT G4), soft masks included, presets `screen`/`ebook`/`print`/`lossless`, one `PdfImageReport` row per image with the reason when it is kept; images reached only through annotation appearances, patterns or inline `BI … EI` stay as stored
- Added `compress(images: PdfImagePolicy.screen)` — the one-shot takes the same policy
- Added `PdfImagePolicy.minSavings` (default 10%: a lossy re-encode must earn its bytes or the image is kept), `chromaSubsampling` (`auto` = 4:4:4 from quality 90, `full`, `half`; `print` pins `full`) and `recompressJpeg` (re-encode stored JPEGs at `jpegQuality` even when not downsampled; off by default)
- Added `PdfEditor.pageInfo({pages})` with each page's `mediaBox` and `cropBox` (`null` when the page has none), `setMediaBox`, `setCropBox` (`null` removes it) and `setRotation` (absolute, where `rotatePages` is relative), each over a `PdfPages`
- Added `PdfMetadata.copyWith`, and `modDate` is read from a document's info dictionary, where it stayed null
- Added `PdfEditor.clearEraseRegions(page)` to drop the regions queued by `eraseRegions` before save
- Added `PdfEditor.sanitize(PdfSanitizeOptions)` — strip metadata, JavaScript actions and embedded files in one pass; `scrubMetadata()` is its metadata-only form
- Added `PdfDoc.formFields` and `formField(name)` — every AcroForm field with its type, typed value, tooltip, bounds, max length, alignment and read-only/required flags
- Added the form field property setters on `PdfEditor`: `removeFormField`, `setFormFieldReadOnly`, `setFormFieldRequired`, `setFormFieldTooltip`, `setFormFieldBounds`, `setFormFieldMaxLength`, `setFormFieldAlignment`, `setFormFieldBackgroundColor`, `setFormFieldBorderColor`, `setFormFieldBorderWidth`, `setFormFieldAppearance` and `setFormFieldFlags` — each read back after save by `formFields`
- Added `PdfDoc.exportFormData(sink, format:)` — the filled values as FDF or XFDF
- Added `PdfDoc.xfa` (`PdfXfaInfo` with field count, page count and field types, or `null` without an XFA packet) and the one-shot `convertXfaToAcroForm(source, output)` that rewrites an XFA form as a plain AcroForm
- Added `PdfDoc.attachments` (name, size, description, MIME type, read from the name tree without decoding the files) and `extractAttachment(name, sink)`
- Fixed images optimized in an edit session being saved unchanged — the full-rewrite writer copied page-referenced XObjects from the source and skipped the staged replacement
- Fixed `pageImages` listing Form XObjects — only `/Subtype /Image` resources are listed, so every name is a valid `setImageBounds` target

## 4.2.1-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed a DOCX table converting to PDF several times wider than declared, columns off the page — the converter now honours the `w:tblGrid` widths, scaled to the printable width ([#243](https://github.com/whuppi/pdf_manipulator/issues/243) reported by [@kampmapa1-design](https://github.com/kampmapa1-design))
- Fixed a web instance failing with `WASM init failed: Out of memory` during rapid create-and-dispose churn — a worker slot is freed when the browser confirms the worker is gone, not at `terminate()`
- Fixed a page's media box reporting the far corner as its width and height whenever the MediaBox origin is not (0, 0)
- Fixed form field property changes being lost on save unless the form was also flattened — the engine wrote only the value; every property and a removed field now reach the saved file
- Fixed `eraseRegions` and `addRedaction` acting on the wrong rectangle — the width and height were passed where the far corner was expected, so every region past the origin was off by its own size
- Fixed `addStamp` losing the stamp on save when its rectangle sits high on the page — the same size-for-corner mix-up placed the label above the page edge; the builder's `image` was drawn at the wrong size for the same reason
- Fixed `mergeFrom` dropping the merged document's form fields — the pages arrived but no `/Fields` entry pointed at them, so they could not be listed or filled; the AcroForm is carried across now
- Fixed `compress` and full-rewrite saves taking 25 seconds on a 3.8 MB file whose object streams hold large arrays, and 460 ms on a 10-page file of 3,500 objects — each object read more of the file than it needed, and the first now takes half a second, the second 60 ms
- Fixed an LZW stream decoding to wrong bytes without an error when it sets `/EarlyChange 0` or needs the other code-width switch — `/EarlyChange` is read now (default 1), and LZW files decode about twice as fast
- Fixed an embedded file's MIME type being written as `text#232Fplain` — the solidus was escaped twice
- Vendored engines synced upstream: pdf_oxide v0.3.73 → v0.3.78 and office_oxide v0.1.3 → v0.1.11 (extraction, rendering, table-structure and legacy `.doc` fixes)

## 4.2.0-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added `setCheckboxFieldValue(fieldName, checked)` — checks a checkbox without knowing its on-state name (`/Yes`, `/On`, `/1` …); `setFormFieldValue` still takes the name directly
- Fixed a checked checkbox flattening as an empty box and losing its value on reopen — button values are read, resolved and written as appearance-state names, and flattening draws the state that was set ([#215](https://github.com/whuppi/pdf_manipulator/issues/215) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed radio groups being impossible to select — the state is set on the kid widget that offers it (siblings cleared), and the Radio/Pushbutton flag bits are read per ISO 32000-1

## 4.1.0-dev.0

- Added `scan-dirs` — extra directories for `keep: auto` to scan beyond `lib/` and `bin/` (`scan-dirs: [tools, packages/shared/lib]`, relative to your app); a directory that is not there keeps the full binary rather than scanning less
- Verify your app after upgrading: `keep: auto` now actually trims the native engine; if something is missing, state it with `keep: [render, …]`
- Fixed `setup` ignoring your config when run from a subfolder of your project — both lanes now locate your app from its own `.dart_tool/`, not from the working directory
- Fixed `keep: auto` keeping every capability on native — the build hook scanned its own package instead of your app; it now derives your app from the hook's output path (web was never affected)
- Fixed the native keep decision never re-evaluating — your source is now a hook dependency, so adding a `sign()` call recompiles the engine with signatures

## 4.0.0-dev.0

- **Breaking:** the engine is configured with three flat keys under `hooks: user_defines: pdf_manipulator:` — `keep` (`auto`, `[render, …]`, or omit for everything), `detector` (only with `keep: auto`) and `build`. Migrate: `trim: auto` → `keep: auto`, `trim: {keep: […]}` → `keep: […]`, `trim-detector: X` → `detector: X`, drop `setup --trim` and re-run `flutter pub run pdf_manipulator:setup`; an invalid config fails the build with a clear message
- Added `build:` in the same block — `speed` (default, prebuilt), `size` (smaller, opt-level z) or `debug` (symbols for native crash traces); `size` and `debug` compile from source and need [Rust](https://rustup.rs)
- Changed the prebuilt-binary download to retry and resume over HTTP Range, so one network blip on a large asset no longer forces a from-source compile
- Fixed a cryptic `feature edition2024 is required` cargo error on older Rust toolchains — the build checks the required Rust version first and names the `rustup` command to run ([#183](https://github.com/whuppi/pdf_manipulator/issues/183) reported by [@mrhazelh](https://github.com/mrhazelh))

## 3.0.0-dev.1

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Changed the web build to compile wasm-bindgen and wasm-opt inside the engine's cargo workspace — `wasm-bindgen-cli`, `binaryen` and `jq` are no longer needed, [Rust](https://rustup.rs) is the only requirement ([#177](https://github.com/whuppi/pdf_manipulator/issues/177) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed `trim: auto` counting member names inside comments as usage — the scan ignores comments and prints where each kept member matched, e.g. `render (lib/preview.dart:12)` ([#175](https://github.com/whuppi/pdf_manipulator/issues/175) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed source builds failing with "rustup: command not found" on Rust installs not managed by rustup — the build asks rustc which targets are installed and uses rustup only as the fallback ([#176](https://github.com/whuppi/pdf_manipulator/issues/176) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))

## 3.0.0-dev.0

- **Breaking:** flattening CJK or emoji form values no longer uses a bundled font (4.4 MB on every install) → register one: `await pdf.registerFallbackFont(PdfFallbackFontKind.cjk, fontBytes)` (`.emoji` for emoji); without one the value is still saved, only the baked-in look falls back to the field's font
- **Breaking** (only if you took the retracted 2.2.0): `trim-detector: analyzer` is now `scan` → change it or delete the line, it is the default
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added trim — keep only the features your app uses: `trim: auto` under `hooks: user_defines: pdf_manipulator:`, or `trim: {keep: [render, signatures]}`; on web also run `flutter pub run pdf_manipulator:setup --trim`; needs [Rust](https://rustup.rs), compiled once and cached ([#167](https://github.com/whuppi/pdf_manipulator/issues/167))
- Added `Pdf.registerFallbackFont(PdfFallbackFontKind, Uint8List)`
- Changed the default binary — dead engine surfaces, barcode support and the embedded fonts are gone: native 28.7 MB → 21.1 MB, gzipped web download 11.3 MB → 7.2 MB; no action needed
- Fixed the package forcing an old `analyzer` version onto your app — the dependency is gone; `trim: auto` uses a dependency-free scan that can only keep slightly more than you use, never less ([#171](https://github.com/whuppi/pdf_manipulator/issues/171) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed the package archive missing the engine's `Cargo.lock`, which broke `trim` and every compile-from-source path ([#171](https://github.com/whuppi/pdf_manipulator/issues/171) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed the web compile error telling you to install `wasm-pack` — it now names the tool it actually misses, with the install command

## 2.2.0-dev.0

> Retracted on pub.dev: the published archive was missing a build file (the engine's `Cargo.lock`), breaking `trim` and compile-from-source installs. Superseded by 3.0.0, which consolidates everything here.


- **Breaking:** flattening CJK or emoji form values no longer uses a bundled font (4.4 MB on every install) → register one: `await pdf.registerFallbackFont(PdfFallbackFontKind.cjk, fontBytes)` (`.emoji` for emoji); without one the value is still saved, only the baked-in look falls back to the field's font
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added trim — keep only the features your app uses: `trim: auto` under `hooks: user_defines: pdf_manipulator:`, or `trim: {keep: [render, signatures]}`; on web also run `flutter pub run pdf_manipulator:setup --trim`; needs [Rust](https://rustup.rs), compiled once and cached ([#167](https://github.com/whuppi/pdf_manipulator/issues/167))
- Added `Pdf.registerFallbackFont(PdfFallbackFontKind, Uint8List)`
- Changed the default binary — dead engine surfaces, barcode support and the embedded fonts are gone: native 28.7 MB → 21.1 MB, gzipped web download 11.3 MB → 7.2 MB; no action needed

## 2.1.4-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed `flattenForms` dropping a field's value when fill and flatten happen in separate editor sessions — the appearance is regenerated from the persisted `/V` ([#161](https://github.com/whuppi/pdf_manipulator/issues/161) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed `addImageStamp` erasing a page's form widgets when `/Annots` is an indirect reference ([#161](https://github.com/whuppi/pdf_manipulator/issues/161) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed a reopened filled form rendering blank where its value should appear — the renderer regenerates a widget's appearance from `/V` under `/NeedAppearances`, matching the flattener

## 2.1.3-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed `setFormFieldValue` reporting field-not-found on forms whose field names are raw UTF-8 (LibreOffice-class producers) — one spec-tolerant text-string decoder covers UTF-16, UTF-8 and PDFDocEncoding ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed `flattenForms` baking mojibake for values outside ASCII (`ß` → `ÃŸ`) — values decode per ISO 32000-1 §7.9.2.2 and appearance text is written in WinAnsi ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed fill → flatten silently dropping the value on widgets without an appearance stream when the field name is non-ASCII — the flattener and the form extractor decode names the same way ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed flattening CJK and emoji values drawing nothing — the bundled fallback font ships in native and web builds ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed document metadata (`getTitle` and friends) mangling non-ASCII on read, and writes now encode per spec ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))

## 2.1.2-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed a Flutter Web WASM (`dart2wasm`) compile failure — a switch over `Object?` that dart2js accepted was non-exhaustive under dart2wasm ([#145](https://github.com/whuppi/pdf_manipulator/issues/145) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed pub.dev not advertising Flutter Web support — the web runtime resolves to a stub default that `pana` can analyze

## 2.1.1-dev.0

- Fixed the README banner not rendering on pub.dev — the `<picture>` element is flattened to a plain image in the published package

## 2.1.0-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added document producer and creation-date metadata — `PdfEditor.setProducer()` / `getProducer()` and `setCreationDate()` / `getCreationDate()` (raw PDF date strings, e.g. `D:20240101120000Z`), plus `PdfDoc.producer`, `PdfDoc.creator`, and `PdfDoc.creationDate` read on open

## 2.0.2-dev.0

- Fixed `addImageStamp` rendering a transparent-background PNG as a solid black box — the alpha channel ships as a grayscale `/SMask` ([#103](https://github.com/whuppi/pdf_manipulator/issues/103) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed the `RenderedPage.data` doc — `render()` returns PNG-encoded bytes (decode to read pixels), not raw RGBA

## 2.0.1-dev.0

Docs-only — no code or API changes.

- README and every guide (architecture, capabilities, migration, updating, contributing) rewritten, restructured, and verified against the source

## 2.0.0-dev.0

The concurrency rewrite. Every operation now runs fully isolated on its
own *lane* — a dedicated Rust thread (native) or Web Worker (web).

- **Breaking:** `webCoordinatorUrl` + `webWorkerUrl` → one `webLaneWorkerUrl`
- **Breaking:** web now ships `lane_worker.js` (was `coordinator.js` + `worker.js`)
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Every method returns `PdfTask<T>` — a `Future` plus `cancel()`; cancelling kills just that job
- `pdf.dispose()` is instant — no joins, no timeouts, no leaks; in-flight ops resolve with `PdfCancelled`
- Operations on different handles now run truly parallel on native (1.x serialized them)
- Lane budgets never error — past the cap, work queues instead of failing
- Added `PdfConfig.maxLanes` — concurrent lanes per instance (default: half the cores, min 2)
- All three web modes (JSPI, Atomics, OPFS) behave identically; a bad worker/WASM URL now fails instantly with a typed error instead of hanging
- Fixed flattening — translated appearances no longer land off-page, unfilled fields render their default value, stamps render their label
- Fixed missing WASM binaries in the 1.0.6 release
- Fixed Android 16 KB page-size alignment for Google Play API 35+ ([PR whuppi/pdf_oxide#1](https://github.com/whuppi/pdf_oxide/pull/1), [@Binary-Parse](https://github.com/Binary-Parse))

## 1.0.6-dev.0

- Fixed release build routing for consumer builds (by [@Binary-Parse](https://github.com/Binary-Parse))
- Fixed Windows NDK linker `.cmd` extension for Android cross-compilation (by [@Binary-Parse](https://github.com/Binary-Parse))
- Added CI verify tests — release builds now verified on all 6 targets (Android, iOS, macOS, Linux, Windows, Web)

## 1.0.5-dev.0

- Fixed README version not stamped on pub.dev
- Updated tracking links for web build hook support

## 1.0.4-dev.0

- Web setup now verifies all assets against release hashes — detects stale files automatically
- `setup` supports `--force` to re-download everything, `--native` to pre-fetch the native binary

## 1.0.3-dev.0

- Fixed changelog on pub.dev

## 1.0.2-dev.0

- Fixed changelog on pub.dev missing commit history between versions

## 1.0.1-dev.0

- Added public API doc comments across all exported classes and methods
- Minimum Android API corrected from 35 to 21 (Android 5.0)
- Setup command is now `flutter pub run pdf_manipulator:setup` (avoids triggering native build hooks with `dart run`)

## 1.0.0-dev.0

Complete ground-up rewrite — new Rust engine, new instance API, cross-platform (previously Android only). The package docs carry the migration guide and the full capability list.

- **Engine:** pdf_oxide (Rust, MIT/Apache-2.0) replaces the Android-only backend
- **Targets:** iOS, Android, macOS, Windows, Linux, Web — previously Android only
- **API:** instance-based `Pdf()` with `dispose()`; batch editing via `pdf.edit(source)`, create from scratch via `pdf.build()`
- **I/O:** `DataSource` in, `DataSink` out — no file paths, no `dart:io`, same code on every target
- **Errors:** typed `PdfError` sealed class — no more `PlatformException`
- **Performance:** every operation off the main thread, no full-file buffers
- **SDK:** requires Dart >=3.10.0
