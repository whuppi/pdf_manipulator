# Capability Roadmap

Every Rust engine capability mapped to its Dart surface.

**Bounded-buffer I/O — non-negotiable.** Every shipped op streams
through fixed-size buffers (64KB read, 256KB write); every PLANNED op
must too. The test guards (TestSource 64KB, TestSink 256KB) enforce the
*transport* limits, and `make test-memory` holds the engine's own peak
under its ceiling (see **Test infrastructure gaps**). The memory promise
and its exceptions are in [`ARCHITECTURE.md`](ARCHITECTURE.md) "Memory".

Every read and every edit is declared once, and the handles and the
one-call doors share that declaration:

| File | Declares | Rule |
|---|---|---|
| `pdf.dart` | `Pdf` | Lifecycle and every door: handle, scope, one-shot, several documents |
| `pdf_doc.dart` | `PdfDocument`, `PdfDocumentHandle` | The facts read at open, plus the reads of `PdfReads`. No mutations |
| `pdf_reads.dart` | `PdfReads` | Every read, declared once. `PdfDocument` and the object `Pdf.readOnce` returns extend it |
| `pdf_editor.dart` | `PdfEditor`, `PdfEditorHandle`, `PdfFormFieldEditor` | Only what a one-call edit object cannot offer: reads of the staged state, the queued redactions, the form fields. Only the handle saves and disposes |
| `pdf_edits.dart` | `PdfEdits` | Every edit, declared once. `PdfEditor` and the object `Pdf.editOnce` returns extend it |
| `pdf_builder.dart` | `PdfBuilder`, `PdfBuilderHandle`, `PdfPageBuilder` | Create from scratch. No reading of existing PDFs |
| `pdf_jobs.dart` | `merge`, `split` and `compress` bodies | The doors that open several handles |

**Where reads live.** `PdfDocument` answers questions about a document as it
is. `PdfEditor` answers questions about the session's staged state:
`pageCount`, `version`, `isModified`, `metadata`, `pageInfo` and
`pageImages`. Exports (a read that writes to a sink) are reads: `extractText`,
`render`, `extractImages`, `exportFormData` and `extractAttachment`.

**Where edits live.** An edit is a member of `PdfEdits`, so it exists on
`PdfEditor` (many edits, one save) and on `Pdf.editOnce` (one edit, saved)
the moment it compiles. `Pdf.rewrite` hands its body a `PdfEditor`.

**The doors on `Pdf`.** Every call that opens a PDF takes a `PdfInput`.

| Kind | Calls | Who closes the document |
|---|---|---|
| A handle you own | `open`, `edit`, `build` | You, with `dispose()` |
| A scope | `read`, `rewrite`, `buildTo` | The library, when your function returns |
| A one-shot: one call on one document | `readOnce`, `editOnce`, `info`, `resave` | The library, after the call |
| Several documents, or a policy | `merge`, `split`, `compress`, `imagesToPdf`, `sign`, `convertToOffice`, `convertFromOffice`, `convertXfaToAcroForm` | The library, after the call |

---

## PdfDocument — reads

Each row is a member of `PdfDocument` and of the object `Pdf.readOnce`
returns, unless the Dart column says it is a field read at open.

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Page count | `open_document` | `pageCount` | DONE — read at open |
| Version | `open_document` | `version` | DONE — read at open |
| Metadata (title, author, subject, keywords, creator, producer, dates) | `open_document` | `metadata` | DONE — a `PdfMetadata`; `creationDate` is a UTC `DateTime?` |
| Encryption info | `open_document` | `encryption` | DONE — a `PdfEncryptionInfo` (`algorithm`, `permissions`, `requiresPassword`); `null` when the document is not encrypted |
| Is tagged | `open_document` | `isTagged` | DONE |
| Page list (size, crop box, rotation, label) | `page_info` | `pageInfo` | DONE — `label` is `null` when the document has no `/PageLabels`; `cropBox` is `null` when the page has none |
| Extract text to a sink | `extract_text_framed` | `extractText` | DONE — page by page, the library never holds the document's text whole |
| Extract text as a string | `extract_text_framed` | `text` | DONE — `extractText` into a `MemorySink` |
| Search text | `search_text` | `search` | DONE |
| Render pages | `render_pages_streamed` | `render` | DONE |
| Render one page to bytes | `render_pages_streamed` | `bitmap` | DONE — `render` of one page into a `MemorySink` |
| Extract images | `extract_images_streamed` | `extractImages` | DONE |
| Get signatures | `get_signatures` | `signatures` | DONE |
| Verify signatures | `verify_signatures` | `verifySignatures` | DONE — digest over the ByteRange and signer key; no trust chain or revocation |
| Validate PDF/A | `validate_pdf_a` | `validatePdfA` | DONE |
| Validate PDF/UA | `validate_pdf_ua` | `validatePdfUa` | DONE |
| Classify pages | `classify_page` | `classifyPages` | DONE |
| Classify document | `classify_document` | `classifyDocument` | DONE |
| Page range of each bookmark | `plan_split_by_bookmarks` | `bookmarkRanges` | DONE |
| Analyze XFA | `doc_xfa` | `xfa` | DONE — `null` on `minimalPdf`; on `xfaFormPdf` `fieldCount` is 2, `pageCount` 1 and `fieldTypes` `['Checkbox', 'Text']`, the analyzer's own names |
| List attachments | `doc_attachments` | `attachments` | DONE — on `attachmentPdf` one entry with name, description, mime `text/plain` and declared size 11; empty on `minimalPdf` |
| Extract one attachment | `doc_extract_attachment` | `extractAttachment` | DONE — the bytes written equal `hello notes`; an unknown name throws `PdfNotFound` and writes nothing |
| List annotations of a page | `doc_annotations` | `annotations` | DONE — the batteries assert "gone after flatten" and the subtype of each link, stamp and free text |
| Get form fields | `doc_form_fields` | `formFields` | DONE — on `fFormFields` the field names and the text field's default value match the fixture truths; on `fFormFieldsProps` `maxLength`/`tooltip` match. A field by name is a search of this list |
| Export form data | `doc_export_form_data` | `exportFormData` | DONE — `FdfWriter::from_fields` for `PdfFormDataFormat.fdf` and `XfdfWriter::from_fields` for `xfdf`; `fullname` filled with `'Ada'`, saved, reopened and exported, then `/T (fullname) /V (Ada)` is parsed from the decoded FDF text and the `<field name="fullname"><value>Ada</value></field>` pair out of the XML, not substring-matched |

---

## PdfEdits — edits

Each row is a member of `PdfEdits`: on `PdfEditor` and on the object
`Pdf.editOnce` returns. A call that takes pages checks all of them before it
changes one.

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Set metadata | `edit_set_metadata` | `setMetadata` | DONE — replaces the standard info fields: a field left `null` is removed, so `setMetadata(await editor.metadata())` changes nothing |
| Rotate pages | `edit_rotate_pages` | `rotatePages` | DONE — relative to the current rotation |
| Set rotation | `edit_set_rotation` | `setRotation` | DONE — absolute (unlike `rotatePages`): setting 90 twice leaves the page at 90, not 180 |
| Set media box | `edit_set_media_box` | `setMediaBox` | DONE — round-tripped through save and `pageInfo` |
| Set crop box | `edit_set_crop_box` | `setCropBox` | DONE — round-tripped through save and `pageInfo`; `null` removes the crop box |
| Remove pages | `edit_delete_pages` | `removePages` | DONE — removing every page throws `PdfEngineFailed` |
| Keep pages | `edit_select_pages` | `keepPages` | DONE — keeps the pages in document order; an out-of-range page throws `PdfPageOutOfRange` |
| Reorder pages | `edit_select_pages` | `reorderPages` | DONE — a `PdfPageOrder` keeps the order given; the pages it leaves out are removed |
| Move page | `edit_move_page` | `movePage` | DONE |
| Add watermark | `edit_watermark` | `addWatermark` | DONE |
| Add stamp | `edit_add_stamp` | `addStamp` | DONE — a `PdfStamp` in `bounds` at an `opacity` |
| Add image stamp | `edit_add_image_stamp` | `addImageStamp` | DONE — the pages share one copy of the image |
| Flatten forms | `edit_flatten_forms` | `flattenForms` | DONE — no `pages` flattens every page; flattening page 0 of a two-page form bakes page 0's field text into content while page 1's field stays an unflattened annotation |
| Flatten annotations | `edit_flatten_annotations` | `flattenAnnotations` | DONE — same page-scoping proof as `flattenForms`, on stamped pages |
| Convert to PDF/A | `edit_convert_to_pdf_a` | `convertToPdfA` | DONE — takes a `PdfALevel`, `a2` by default |
| Redact now | `edit_redact` | `redact` | DONE — removes the content under the regions for good, draws a `PdfRedactionOverlay` (a black box by default; `.none()` draws none) and returns a `PdfRedactionReport`; marks queued earlier stay queued. It is also the erase of a region: `redact(page, regions, overlay: const PdfRedactionOverlay.none())` |
| Reduce images by policy | `edit_reduce_images` | `reduceImages` | DONE — presets `screen`/`ebook`/`print`/`lossless`; typed `PdfImageReport`, one row per image XObject |
| Reduce images inside annotation appearances, patterns and inline images | — | — | PLANNED — `reduceImages` visits page content and Form XObjects only; appearance streams, pattern content and `BI … EI` images are left as stored |
| Unembed standard fonts | `edit_unembed_standard_fonts` | `unembedStandardFonts` | DONE |
| Add attachment | `edit_embed_file` | `addAttachment` | DONE — after save, `attachments` reads back the name, the description, the mime type and the declared size, and `extractAttachment` returns its bytes |
| Crop margins | `edit_crop_margins` | `cropMargins` | DONE |
| Set image bounds | `edit_set_image_bounds` | `setImageBounds` | DONE — `pageImages` supplies the XObject name and the same `PdfRect`; an unknown name throws `PdfNotFound`; verified through `pageImages` after save |
| Sanitize document | `edit_sanitize` | `sanitize` | DONE — `stripMetadata`, `stripJavaScript` and `stripAttachments`; the scrub of the metadata alone is `sanitize(stripJavaScript: false)` |

---

## PdfEditor — what only an editor adds

The editor reads the staged state, appends pages from another document,
queues redactions and edits form fields. The handle saves and disposes.

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Page count | `edit_get_metadata` | `pageCount` | DONE |
| Version | `edit_get_metadata` | `version` | DONE |
| Is modified | `edit_is_modified` | `isModified` | DONE |
| Metadata | `edit_get_metadata` | `metadata` | DONE |
| Page facts (size, crop box, rotation, label) | `edit_page_info` | `pageInfo` | DONE — the handwritten `croppedPagePdf` truth (x 72, y 72, w 468, h 648) proves `cropBox` is the page's own and not the inherited MediaBox; `null` when the page has no `/CropBox` |
| List images on a page | `edit_page_images` | `pageImages` | DONE — name, placement bounds and the full transform per image XObject on a page |
| Append pages of another PDF | `merge_document` | `appendPages` | DONE — the appended document's `/AcroForm /Fields` come with its pages, so `formFields` lists `fullname` and `agree` after appending a form to `minimalPdf`; appending page `[1]` of `fThreePageMarkers` to `minimalPdf` gives 2 pages whose page 1 extracts `MARKER-TWO` and neither other marker; an out-of-range index throws `PdfEngineFailed` |
| Mark a redaction | `edit_add_redaction` | `markRedaction` | DONE — queues one mark per region |
| Count queued marks of a page | `edit_redaction_count` | `redactionCount` | DONE |
| Apply queued redactions | `edit_apply_redactions` | `applyRedactions` | DONE — destroys the content now and returns a `PdfRedactionReport` (`glyphsRemoved > 0` and the redacted text gone after save) |
| Clear queued redactions | `edit_clear_redactions` | `clearRedactions` | DONE — drops every queued mark and leaves what `redact` removed |
| Save | `edit_save_with_options` | `PdfEditorHandle.save` | DONE — `PdfSave.full()` by default, streamed to a `DataSink` |
| Save and encrypt or decrypt | `edit_save_with_options` | `PdfSave.full(encryption:)` | DONE — `PdfEncryption.password` encrypts, `PdfEncryption.remove()` strips, `PdfEncryption.keep()` is the default |
| Incremental save (append to the original bytes) | `host/incremental.rs` | `PdfSave.incremental()` | DONE — the output starts with the source bytes, byte for byte, then one update section in the source's xref style (table or stream); a signed source keeps every byte its signature covers. Throws `PdfIncrementalRefused`, writing nothing, after a page removal, reorder or append, redaction or sanitize, a content, annotation or flatten edit, an added or removed form field, an attachment, and on an encrypted, repaired or DocMDP-certified source the change is not allowed on |
| Incremental save with a fallback | `host/incremental.rs`, `edit_save_with_options` | `PdfSave.incrementalOrRewrite()` | DONE — appends when it can and writes the file again when it cannot; the rewrite breaks a signature, as any full save does |
| Dispose | — | `PdfEditorHandle.dispose` | DONE |

### PdfFormFieldEditor — `editor.field(name)`

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Set a text value | `edit_set_form_field_value` | `setValue(PdfFormFieldValue.text(...))` | DONE |
| Set a checkbox | `edit_set_checkbox_field_value` | `setValue(PdfFormFieldValue.checked(...))` | DONE |
| Set a choice | `edit_set_form_field_choice` | `setValue(PdfFormFieldValue.choice(...))` | DONE — a choice that is not an option of the field throws `PdfEngineFailed` and changes nothing |
| Set several choices | `edit_set_form_field_choices` | `setValue(PdfFormFieldValue.choices(...))` | DONE — a multi-select list box |
| Clear the value | `edit_clear_form_field_value` | `setValue(PdfFormFieldValue.none())` | DONE |
| Remove the field | `edit_remove_form_field` | `remove` | DONE — removed from `/AcroForm /Fields` and every page's `/Annots` on save; `formFields` no longer lists it on reopen, the other fields stay listed |
| Set read-only | `edit_set_form_field_readonly` | `setReadOnly` | DONE — `isReadOnly` reads back `true` after save + reopen |
| Set required | `edit_set_form_field_required` | `setRequired` | DONE — `isRequired` reads back `true` after save + reopen |
| Set tooltip | `edit_set_form_field_tooltip` | `setTooltip` | DONE — `tooltip` reads back the new `/TU` after save + reopen; `null` removes it |
| Set bounds | `edit_set_form_field_bounds` | `setBounds` | DONE — `bounds` reads back the new `/Rect` after save + reopen |
| Set max length | `edit_set_form_field_max_length` | `setMaxLength` | DONE — `maxLength` reads back the new `/MaxLen`, not the fixture default, after save + reopen; `null` removes it |
| Set alignment | `edit_set_form_field_alignment` | `setAlignment` | DONE — `alignment` reads back `PdfAlignment.center` after save + reopen |
| Set background color | `edit_set_form_field_background_color` | `setBackgroundColor` | DONE — proven with border color/width and appearance: the field still flattens, `text` shows the filled value, and the rendered page differs from the unstyled render |
| Set border color | `edit_set_form_field_border_color` | `setBorderColor` | DONE — same proof as background color |
| Set border width | `edit_set_form_field_border_width` | `setBorderWidth` | DONE — same proof as background color |
| Set default appearance | `edit_set_form_field_appearance` | `setAppearance` | DONE — dispatch builds the `/{font} {size} Tf {r} {g} {b} rg` DA string; same proof as background color |
| Set flags | `edit_set_form_field_flags` | `setFlags` | DONE — a `Set<PdfFormFieldFlag>`; `{readOnly, required}` reads back both flags through `formFields`, and `{multiline}` leaves the field listed and still flattenable |

---

## PdfBuilder — create from scratch

`PdfBuilder` is the view a `buildTo` body gets. `PdfBuilderHandle` is the one
`Pdf.build` returns, and the only one that saves and disposes. A page ends
when the next is added or the document is saved.

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Create builder | `handle_builder_create` | `Pdf.build` | DONE |
| Set metadata | `handle_builder_set_metadata` | `setMetadata` | DONE — title, author, subject, keywords, creator, producer and the two dates |
| Add page | `handle_builder_add_page` | `addPage` | DONE — A4 by default; `PdfPageSize` also names Letter, Legal, A3, A5 or a custom size |
| Add image page | `handle_builder_add_image_page` | `addImagePage` | DONE — the page takes the image's own size, one point per pixel, unless a `PdfImagePage` names a size |
| Save | `handle_builder_save` | `PdfBuilderHandle.save` | DONE — Flate-compresses by default (`deflate: false` keeps streams plain) |
| Dispose | — | `PdfBuilderHandle.dispose` | DONE |

### PdfPageBuilder — content of a page

Each row reaches the engine as a `PageOp` applied by `apply_page_op`.

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Font | `PageOp::Font` | `setFont` | DONE |
| Move the cursor | `PageOp::At` | `moveTo` | DONE — takes a `PdfPoint` |
| Text | `PageOp::Text`, `PageOp::StyledText` | `text` | DONE — with a `PdfTextStyle`, and a `PdfLink` or `PdfMarkup` for the text |
| Append text | `PageOp::Inline`, `PageOp::StyledText` | `appendText` | DONE — same arguments as `text` |
| Heading | `PageOp::Heading` | `heading` | DONE |
| Paragraph | `PageOp::Paragraph` | `paragraph` | DONE |
| Space | `PageOp::Space` | `space` | DONE |
| Newline | `PageOp::Newline` | `newline` | DONE |
| Horizontal rule | `PageOp::HorizontalRule` | `horizontalRule` | DONE |
| Text in a box | `PageOp::TextInRect` | `textIn` | DONE |
| Image | `PageOp::Image`, `PageOp::ImageArtifact` | `image` | DONE — with `altText` the image is a figure that carries it; without, an artifact |
| Rectangle | `PageOp::FilledRect`, `PageOp::StrokeRect` | `rect` | DONE — filled, stroked or both |
| Line | `PageOp::StrokeLine` | `line` | DONE |
| Watermark | `PageOp::Watermark` | `watermark` | DONE |
| Stamp | `PageOp::Stamp` | `stamp` | DONE |
| Sticky note | `PageOp::StickyNote` | `stickyNote` | DONE |
| Free text | `PageOp::FreeText` | `freeText` | DONE |
| Footnote | `PageOp::Footnote` | `footnote` | DONE |
| Columns | `PageOp::Columns` | `columns` | DONE |
| Table | `PageOp::StreamingTableBlock` | `table` | DONE — a malformed table throws `ArgumentError` before any I/O |
| Text field | `PageOp::TextField` | `textField` | DONE — `value` and `PdfFieldScripts` |
| Checkbox | `PageOp::Checkbox` | `checkbox` | DONE |
| Combo box | `PageOp::ComboBox` | `comboBox` | DONE |
| Radio group | `PageOp::RadioGroup` | `radioGroup` | DONE |
| Push button | `PageOp::PushButton` | `pushButton` | DONE |
| Signature field | `PageOp::SignatureField` | `signatureField` | DONE |
| Page open action | `PageOp::OnOpen` | `onOpen` | DONE |
| Page close action | `PageOp::OnClose` | `onClose` | DONE |

---

## Pdf — the doors

Each row is a member of `Pdf`.

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Open a document for many reads | `open_document` | `open` | DONE — a `PdfDocumentHandle` you dispose |
| Open an editor | `handle_editor_open` | `edit` | DONE — a `PdfEditorHandle` you save and dispose |
| Create a builder | `handle_builder_create` | `build` | DONE — a `PdfBuilderHandle` you save and dispose |
| Read in a scope | `open_document` | `read` | DONE — the library closes the document, also after an error or a cancel |
| Edit in a scope | `edit_save_with_options` | `rewrite` | DONE — parses once, saves once; a body that throws writes nothing |
| Build in a scope | `handle_builder_save` | `buildTo` | DONE |
| One read | `open_document` | `readOnce` | DONE — a second call throws `StateError` |
| One edit | `edit_save_with_options` | `editOnce` | DONE — a second call throws `StateError` |
| Document info | `open_document` | `info` | DONE — page count, version, metadata and encryption without keeping the file open |
| Save again with no edit | `edit_save_with_options` | `resave` | DONE — encrypt, decrypt or re-deflate through `PdfSave` |
| Merge | `merge_document` | `merge` | DONE — the first input is the base and the others are appended whole |
| Split | `handle_editor_extract_pages` | `split` | DONE — `PdfSplitRule.every`, `.maxBytes` and `.bookmarks`; each part has a sink from `sinkFor`, and the parts delivered before a failure stand |
| Compress | `edit_reduce_images`, `edit_save_with_options` | `compress` | DONE — `PdfImagePolicy.lossless` unless you pass another; never makes a file larger |
| Images to PDF | `handle_builder_add_image_page` | `imagesToPdf` | DONE — one image per page, through `buildTo` |
| Sign (PKCS12 / PEM) | `handle_sign` | `sign` | DONE — keeps the catalog and earlier signatures; update section in the source's xref style; an encrypted input throws `PdfUnsupported` |
| Convert PDF to Office (DOCX/PPTX/XLSX) | `convert_to_format_writer` | `convertToOffice` | DONE |
| Convert Office to PDF | `convert_from_format_writer` | `convertFromOffice` | DONE |
| Convert XFA to AcroForm | `convert_xfa_to_acroform` | `convertXfaToAcroForm` | DONE — converting `xfaFormPdf` yields a document that opens with a page and whose `formFields` lists `given` and `agree`; a document without XFA throws `PdfEngineFailed` |
| Register a fallback font | `register_fallback_font` | `registerFallbackFont` | DONE — a CJK or emoji face, read whole, for baking form values the field's font cannot render |

---

## Runtime — the executor pool

| Capability | Status |
|---|---|
| Per-op cancellation (`PdfTask.cancel()` on every engine method) | DONE |
| Instant dispose (end every lane, return every executor, no joins) | DONE |
| Budgets — queue, never fail (`kMaxExecutors` executors per pool; `MAX_EXECUTORS_GLOBAL` threads process-wide on native; first-in-first-out waiters) | DONE |
| Executors reset and leased again under churn (both platforms) | DONE |
| Fire-and-forget error rules (cancelled silent, real failures loud) | DONE |
| Four web I/O modes (Atomics / JSPI / OPFS / Asyncify), identical suite | DONE |
| Item streaming over the job port (multi-result ops without buffering) | PLANNED — the job's result port is already a message channel: promote it to N interim messages + 1 terminal message. Inherits per-job cancel, ending the lane, and the post-driven cleanup protocol unchanged; symmetric on web (`postMessage` to the executor). Never add a second transport method for this. |

---

## Summary — API surface

Totals for the consumer surfaces above. Runtime, build, and roadmap
capabilities are tracked in their own sections.

| Category | Done | Planned |
|---|---|---|
| PdfDocument | 25 | 0 |
| PdfEdits | 22 | 1 |
| PdfEditor (with `PdfFormFieldEditor`) | 33 | 0 |
| PdfBuilder (with `PdfPageBuilder`) | 34 | 0 |
| Pdf (the doors) | 19 | 0 |
| **Total** | **133** | **1** |

---

## Build & distribution

| Capability | Status | Notes |
|---|---|---|
| Native binary resolution (5-step waterfall) | DONE | cached → download → compile → submodule → error |
| Web asset resolution (same waterfall) | DONE | WASM + JS glue, hash-verified |
| `setup web` (default) | DONE | Downloads or compiles web assets, hash-verified |
| `setup <target>` | DONE | Triggers `flutter build` to cache native binary |
| `setup --force web` | DONE | Re-download web assets (debugging) |
| SHA-256 hash verification (all assets) | DONE | Native + web, stale detection on setup |
| `build.json` | DONE | Single source of truth for crate, repo, features, web assets |
| Link hook (`hook/link.dart`) | DONE | Forwards assets; on release drives the RecordUse trim path (see **Binary size — trim**) |
| Automatic web setup via build hook | BLOCKED | See details below |

### Automatic web setup — what's blocking, what to track

The web setup step (`flutter pub run pdf_manipulator:setup`) exists
because Flutter's build hook system only supports native code assets
(`CodeAsset`). WASM modules and JS workers need dedicated asset types
that don't exist yet.

**What will solve it:**
[`WasmAsset` / `JsAsset`](https://github.com/dart-lang/native/issues/988) —
dedicated web asset types where the framework (Flutter, Dart, Jaspr)
handles bundling and exposes a runtime URI. Web workers and WASM
modules need URLs, not raw bytes — a plain data asset can't provide that.
Same problem affects [drift](https://github.com/simolus3/drift/issues/3770)
and [sqlite3](https://github.com/simolus3/sqlite3.dart) — all waiting
on the same feature.

**What's ready on our side:**
`hook/build.dart` already implements `resolveWeb()` with the full
5-step waterfall. When the Dart SDK adds the asset type and Flutter
wires the trigger, `main()` adds one call to `resolveWeb()` and
`setup` becomes optional. Zero new logic needed.

**Track:** [dart-lang/native#988](https://github.com/dart-lang/native/issues/988)
(P3, Native Assets v1.x milestone, no ETA).

---

## Binary size — trim

Shipped: the trim system (issue #167). Consumer docs in the README;
durable architecture in [`ARCHITECTURE.md`](ARCHITECTURE.md) §Trim;
size-measuring recipe in [`UPDATING.md`](UPDATING.md) §S5b.

| Piece | Status | Notes |
|---|---|---|
| Capability vocabulary + `keep:` grammar (`auto` / `[list]` / loud errors) | DONE | `lib/src/keep/capabilities.dart` |
| Text-scan detector (dependency-free, fail closed, over-keep only) | DONE | `lib/src/keep/detector.dart` — replaced the resolved-AST scan: package:analyzer in a runtime package's deps fights the app's own tooling (#171) |
| Native trim via `hooks: user_defines:` | DONE | build hook; custom sets compile locally, cargo cache |
| Web trim via `hooks: user_defines:` (same block as native) | DONE | `setup` reads the app pubspec (`lib/src/hook/user_defines.dart`) and runs the shared `resolveKeepPlan`; no flags |
| Engine `build:` — speed / size / debug (web + native) | DONE | `lib/src/hook/engine_build.dart`; rides cargo `release` via `CARGO_PROFILE_RELEASE_*` (keeps `panic=unwind` — native-safe); size/debug compile from source |
| Mismatch-proof config (`keep` / `detector` / `build`, one parser) | DONE | `lib/src/hook/pdf_config.dart` — one parser both callers share; unknown keys + bad values + the `detector`-without-`keep: auto` cross-axis mismatch all fail LOUDLY. Invalid configs unrepresentable by design |
| `make shake-audit` verifier | DONE | symbols + size ceiling + typed-error probes |
| Trimmed example shell (`example_trimmed/`) | DONE | Config-shell over example/ with `keep: [render]`; `make test-example-trimmed` asserts the contract on a real trimmed binary (core + kept work, excluded ops answer the typed error). Not in CI yet — needs a Rust-provisioned macOS row in full-test.yml. |
| Op-unit dispatch layer (entry + handler + linker anchor per op) | DONE | `vendor/pdf_oxide/src/host/ops/`; registry backend swappable |
| RecordUse drive path (build full → link hook trims on release) | DONE, dormant | Activates itself when the SDK experiment records; fixture-tested today |
| `panic=abort` size lever | WONT_DO | Native executor isolation is `catch_unwind` (one bad PDF → typed error, engine survives); abort would crash the whole app. Wasm already ships abort via `release-small` — the Web Worker boundary isolates there. Nothing left to win. |
| Writer monomorphization dedup | WONT_DO | Measured honestly: the dedupable Boxed-vs-Seek writer pairs are ~150-200 KB (2% of core) for dyn dispatch in the hottest safety-critical loop plus invasive upstream surgery. Bad trade. |
| `opt-level=z` size option | DONE | Asked for → shipped as `build: size` (opt-level z on the release profile). Never the default (it costs CPU on every op); opt-in per app via the pubspec `build:` key. |
| `extract` capability | DONE | Extraction + search + classification (+ CJK CID tables). Cut via root gates — three dispatch fns gated, LTO deleted the 2.1 MB web with zero document.rs surgery. Core promise is now parse/write/edit/forms/builder. `office` requires `extract` engine-side. All remaining per-op cuts measured at 10-60 KB each: not worth their surface. |
| Lazy fallback-font loading (runtime memory, not binary size) | PLANNED, deprioritized | `registerFallbackFont(bytes)` keeps the font resident: one per executor. Bytes-in is the primitive, so a lazy `DataSource` registration (load on first CJK/emoji bake, per executor) and an optional release call can layer on later without breaking the API. Pick up only on real memory-pressure reports from web apps — and design the API surface carefully at that point, not before. |

### When the futures arrive — tracked triggers + exact approach

| Trigger to watch | Signal | Approach when it lands |
|---|---|---|
| RecordUse experiment stabilizes (dart-lang/native#2902 for instance methods; the SDK record-use experiment flag) | recordings appear in release link hooks | Nothing to build — the detector self-activates. Validate with `trim-detector: compare` on example/ (diff recorded vs scan keep-sets). When instance methods land: delete the `record_use_shim*.dart` files, annotate the ops directly. Separately: `record_use` is pinned `^0.6.0` because ≥1.0.0 needs `meta ^1.19` and Flutter stable pins meta 1.18 — bump the pin when a Flutter stable ships meta ≥1.19 (same API for our use). When trust is earned: consider promoting the default detector. |
| Dart static linking (dart-lang/sdk#49418; `StaticLinking` in code_assets implemented) | hooks accept static libs; SDK defines symbol/asset-tag references | 1) Wire Dart-side references to the `pdf_op_<name>_anchor` symbols (one per public op, using whatever asset-tag syntax ships). 2) Swap `ops/registry.rs` from the explicit table to link-section collection. 3) Add a gc-sections assertion to shake-audit. The detector becomes a cross-check; the linker becomes the mechanism. Units don't change. |
| Web build hooks (dart-lang/native#988) | hooks run for web targets | Web already reads the same `keep:`/`build:` pubspec block native does (via `setup` → `user_defines.dart` → `resolveKeepPlan`). Remaining win: fold that into an automatic web build hook so the manual `setup` step disappears entirely — wasm gains the same no-command auto flow as native. |
| Wasm component model in dart2wasm (dart-lang/sdk#56366) | dart2wasm emits/links components | Express op units as WIT interface functions (one-to-one mapping already); component-level linking replaces export-root trimming on web. Furthest future — the scan covers web until then. |

## Built in the engine, not shipped

These exist in the Rust core (`pdf_oxide`) behind cargo features but are excluded from the shipped `pdf_manipulator` feature sets (`build.json`) — they pull the ONNX runtime and large model files that would bloat every install. No Dart op exposes them today; with the feature off, the FFI returns `_ERR_UNSUPPORTED`. A size tradeoff, not a missing capability.

| Capability | Engine support | Why not shipped |
|---|---|---|
| OCR (scanned-page text) | PaddleOCR — DBNet++ detect → SVTR recognize — via ONNX; auto-detects scanned vs native pages; gated on `ocr` / `ocr-tract` / `wasm-ocr` | Adds the ONNX runtime + ~12.5 MB of models per install |
| Table extraction | `table_extractor` + `spatial_table_detector`, gated on `table-ml` / `ml` | Same ML stack (ONNX + model weight) |

The planned route to expose these without bloating the default is per-feature opt-in via `user_defines` (see [Binary size — trim](#binary-size--trim)): the consumer enables the cargo feature in their own pubspec and accepts the ONNX runtime + model download. It's deliberately opt-in, not default — and still extra setup on the consumer's side, not a clean built-in. Open an issue to push it forward; demand is what decides priority.

---

## Not planned — open an issue if you need it

| Feature | Where it stands |
|---|---|
| PDF viewer widget | `doc.render(...)` already gives you page images, so a viewer is *buildable* on top — but [pdfx](https://pub.dev/packages/pdfx) and [flutter_pdfview](https://pub.dev/packages/flutter_pdfview) already do viewing well, and this package's focus is manipulation, not UI. Not prioritized; open an issue if a first-party viewer would help. |

---

## Deferred — surfaced by the test overhaul

| Capability | Status | Why |
|---|---|---|
| Builder output hardening (compress streams by default; richer typesetting) | DONE | `PdfBuilderHandle.save` Flate-compresses by default (`deflate: false` keeps streams plain); the page builder reaches every page operation the engine's builder has, and a wire-sync test fails when the engine adds one Dart lacks. |
| Attachment-listing read API | DONE | `PdfDocument.attachments` and `PdfDocument.extractAttachment` — `addAttachment` is proven by listing the saved file and round-tripping its bytes. |
| Annotation-listing read API | DONE | `PdfDocument.annotations` — a battery asserts "gone after flatten" directly, as `formFields` does for forms. |
| Typed wire error codes (PdfWrongPassword, PdfCorrupted, … from Rust) | DONE | Every engine failure reaches Dart as its `PdfError` subtype; kinds on the wire. |

---

## Test infrastructure gaps

| Gap | Detail | Status |
|---|---|---|
| Field-action and link no-op | The field scripts ride with the field (`scripts:`) and the link or markup rides with the text (`link:`, `markup:`), so neither can name a page that lacks its target; the engine still refuses an unattached `PageOp` with an error instead of succeeding without effect. | DONE |
| True O(1) memory verification | `make test-memory` (tool/memprobe) measures the Rust peak of every wire op on a 10,000-page input, 64 MiB streams and the other probe inputs, each against its row in `limit()`. Every op stays under 16 MiB; the exceptions, an image decoded at its full size, the patched XMP packet and the Office import, are in ARCHITECTURE.md "Memory". | DONE |
