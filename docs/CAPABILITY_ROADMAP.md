# Capability Roadmap

Every Rust engine capability mapped to its Dart surface.

**Bounded-buffer I/O — non-negotiable.** Every shipped op streams
through fixed-size buffers (64KB read, 256KB write); every PLANNED op
must too. The test guards (TestSource 64KB, TestSink 256KB) enforce the
*transport* limits mechanically — see the test architecture in
[`ARCHITECTURE.md`](ARCHITECTURE.md). Full peak-memory verification of
Rust-internal processing is itself a tracked gap (see **Test
infrastructure gaps**).

Five files, strict rules:

| File | Role | Rule |
|---|---|---|
| `pdf_doc.dart` | Read-only queries | No mutations |
| `pdf_editor.dart` | Mutations only | No read/export ops even if Rust has them on editor |
| `pdf_builder.dart` | Create from scratch | No reading existing PDFs |
| `pdf_standalone.dart` | Source in, sink out, no handle | Non-mutating one-shot ops |
| `pdf_sugar.dart` | Convenience wrappers | Over editor/builder only, never standalone (rare exception allowed) |

---

## PdfDoc — read-only queries

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Page count | `current_page_count` | `pageCount` | DONE |
| Version | `version` | `version` | DONE |
| Page list (dimensions, rotation) | `get_page_media_box`, `get_page_rotation` | `pages` (decoded on open) | DONE |
| Title / Author / Subject / Keywords | `title`, `author`, `subject`, `keywords` | decoded on open | DONE |
| Encryption info | via open result | `isEncrypted`, `encryptionAlgorithm`, `permissions` | DONE |
| Is tagged | via open result | `isTagged` | DONE |
| Extract text | `extract_page_text` | `extract()` | DONE |
| Search text | `search_text` | `search()` | DONE |
| Render pages | `render_pages_streamed` | `render()` | DONE |
| Extract images | `extract_images_streamed` | `extractImages()` | DONE |
| Get signatures | `get_signatures` | `getSignatures()` | DONE |
| Verify signatures | `verify_signatures` | `verifySignatures()` | DONE |
| Validate PDF/A | `validate_pdfa` | `validatePdfA()` | DONE |
| Validate PDF/UA | `validate_pdfua` | `validatePdfUa()` | DONE |
| Classify page | `classify_page` | `classifyPage()` | DONE |
| Classify document | `classify_document` | `classifyDocument()` | DONE |
| Plan split by bookmarks | `plan_split_by_bookmarks` | `planSplitByBookmarks()` | DONE |
| Get page crop box | `get_page_crop_box` | — | PLANNED |
| Has XFA forms | `has_xfa` | — | PLANNED |
| Analyze XFA | `analyze_xfa` | — | PLANNED |
| Get form fields (list all) | `get_form_fields` | — | PLANNED |
| Get form field value | `get_form_field_value` | — | PLANNED |
| Has form field | `has_form_field` | — | PLANNED |
| Get page images (list metadata) | `get_page_images` | — | PLANNED |
| Producer / Creator metadata | `producer`, `creator` | `producer`, `creator` (decoded on open) | DONE |
| Creation date | `creation_date` | `creationDate` (decoded on open) | DONE |

---

## PdfEditor — mutations only

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Set title | `set_title` | `setTitle()` | DONE |
| Set author | `set_author` | `setAuthor()` | DONE |
| Set subject | `set_subject` | `setSubject()` | DONE |
| Set keywords | `set_keywords` | `setKeywords()` | DONE |
| Get title | `title` | `getTitle()` | DONE |
| Get author | `author` | `getAuthor()` | DONE |
| Get subject | `subject` | `getSubject()` | DONE |
| Get keywords | `keywords` | `getKeywords()` | DONE |
| Get producer | `producer` | `getProducer()` | DONE |
| Get creation date | `creation_date` | `getCreationDate()` | DONE |
| Scrub metadata | via bridge | `scrubMetadata()` | DONE |
| Rotate page | `rotate_page_by` | `rotatePage()` | DONE |
| Rotate all pages | `rotate_all_pages` | `rotateAllPages()` | DONE |
| Delete page | via bridge | `deletePage()` | DONE |
| Move page | via bridge | `movePage()` | DONE |
| Select pages | `select_pages` | `selectPages()` | DONE |
| Merge from another PDF | `merge_from_reader` | `mergeFrom()` | DONE |
| Optimize images | via bridge | `optimizeImages()` | DONE |
| Unembed standard fonts | via bridge | `unembedStandardFonts()` | DONE |
| Add watermark | via bridge | `addWatermark()` | DONE |
| Add stamp | via bridge | `addStamp()` | DONE |
| Add image stamp | via bridge | `addImageStamp()` | DONE |
| Embed file | `embed_file` | `embedFile()` | DONE |
| Erase regions | `erase_regions` | `eraseRegions()` | DONE |
| Flatten forms | `flatten_forms` | `flattenForms()` | DONE |
| Flatten all annotations | `flatten_all_annotations` | `flattenAllAnnotations()` | DONE |
| Set form field value | `set_form_field_value` | `setFormFieldValue()` | DONE |
| Crop margins | `crop_margins` | `cropMargins()` | DONE |
| Resize image | `resize_image` | `resizeImage()` | DONE — untestable from the public surface: no API lists image XObject names, so a caller cannot know a valid name to pass. Add an image-name listing (e.g. on `extractImages`) to make this testable and usable. |
| Convert to PDF/A | via bridge | `convertToPdfA()` | DONE |
| Add redaction | `add_redaction` | `addRedaction()` | DONE |
| Redaction count | `redaction_count` | `redactionCount()` | DONE |
| Apply redactions | `apply_all_redactions` | `applyRedactions()` | DONE |
| Get page media box | `get_page_media_box` | `getPageMediaBox()` | DONE |
| Is modified | `is_modified` | `isModified` | DONE |
| Page count | `current_page_count` | `pageCount` | DONE |
| Version | `version` | `version` | DONE |
| Save | `write_full_to_writer` | `save()` | DONE |
| Set producer | `set_producer` | `setProducer()` | DONE |
| Set creation date | `set_creation_date` | `setCreationDate()` | DONE |
| Set page media box | `set_page_media_box` | — | PLANNED |
| Set page crop box | `set_page_crop_box` | — | PLANNED |
| Set page rotation | `set_page_rotation` | — | PLANNED |
| Flatten forms on single page | `flatten_forms_on_page` | — | PLANNED |
| Flatten annotations on single page | `flatten_page_annotations` | — | PLANNED |
| Clear erase regions | `clear_erase_regions` | — | PLANNED |
| Merge selective pages from | `merge_pages_from` | — | PLANNED |
| Apply redactions destructive | `apply_redactions_destructive` | — | PLANNED |
| Sanitize document | `sanitize_document` | — | PLANNED |
| Reposition image | `reposition_image` | — | PLANNED |
| Set image bounds | `set_image_bounds` | — | PLANNED |
| Remove form field | `remove_form_field` | — | PLANNED |
| Set form field readonly | `set_form_field_readonly` | — | PLANNED |
| Set form field required | `set_form_field_required` | — | PLANNED |
| Set form field tooltip | `set_form_field_tooltip` | — | PLANNED |
| Set form field rect | `set_form_field_rect` | — | PLANNED |
| Set form field max length | `set_form_field_max_length` | — | PLANNED |
| Set form field alignment | `set_form_field_alignment` | — | PLANNED |
| Set form field background color | `set_form_field_background_color` | — | PLANNED |
| Set form field border color | `set_form_field_border_color` | — | PLANNED |
| Set form field border width | `set_form_field_border_width` | — | PLANNED |
| Set form field default appearance | `set_form_field_default_appearance` | — | PLANNED |
| Set form field flags | `set_form_field_flags` | — | PLANNED |
| Convert XFA to AcroForm | `convert_xfa_to_acroform` | — | PLANNED |
| Embed file with options | `embed_file_with_options` | — | PLANNED |
| Export form data FDF | `export_form_data_fdf` | — | PLANNED |
| Export form data XFDF | `export_form_data_xfdf` | — | PLANNED |

---

## PdfBuilder — create from scratch

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Create builder | via bridge | `Pdf.build()` | DONE |
| Add page (custom size) | via bridge | `addPage()` | DONE |
| Add A4 page | — (dart convenience) | `addA4Page()` | DONE |
| Add Letter page | — (dart convenience) | `addLetterPage()` | DONE |
| Set title | via bridge | `setTitle()` | DONE |
| Set author | via bridge | `setAuthor()` | DONE |
| Set subject | via bridge | `setSubject()` | DONE |
| Set keywords | via bridge | `setKeywords()` | DONE |
| Text | via bridge | `page.text()` | DONE |
| Heading | via bridge | `page.heading()` | DONE |
| Paragraph | via bridge | `page.paragraph()` | DONE |
| Image | via bridge | `page.image()` | DONE |
| Watermark | via bridge | `page.watermark()` | DONE |
| Font | via bridge | `page.font()` | DONE |
| Space | via bridge | `page.space()` | DONE |
| Horizontal rule | via bridge | `page.horizontalRule()` | DONE |
| Newline | via bridge | `page.newline()` | DONE |
| New page same size | via bridge | `page.newPageSameSize()` | DONE |
| Text field | via bridge | `page.textField()` | DONE |
| Checkbox | via bridge | `page.checkbox()` | DONE |
| Combo box | via bridge | `page.comboBox()` | DONE |
| Push button | via bridge | `page.pushButton()` | DONE |
| Signature field | via bridge | `page.signatureField()` | DONE |
| Radio group | via bridge | `page.radioGroup()` | DONE |
| Field keystroke | via bridge | `page.fieldKeystroke()` | DONE |
| Field format | via bridge | `page.fieldFormat()` | DONE |
| Field validate | via bridge | `page.fieldValidate()` | DONE |
| Field calculate | via bridge | `page.fieldCalculate()` | DONE |
| Link URL | via bridge | `page.linkUrl()` | DONE |
| Link page | via bridge | `page.linkPage()` | DONE |
| Footnote | via bridge | `page.footnote()` | DONE |
| Columns | via bridge | `page.columns()` | DONE |
| Page done | via bridge | `page.done()` | DONE |
| Save | via bridge | `save()` | DONE |

---

## PdfStandalone — source in, sink out, no handle

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Sign PDF (PKCS12 / PEM) | `handle_sign` | `sign()` | DONE |
| Convert to format (DOCX/PPTX/XLSX) | `handle_convert_to` | `convertTo()` | DONE |
| Convert to PDF | `handle_convert_to_pdf` | `convertToPdf()` | DONE |
| Extract pages | `handle_editor_extract_pages` | `extractPages()` | DONE |

---

## PdfSugar — convenience wrappers over editor/builder

| Sugar method | Wraps | Status |
|---|---|---|
| `merge` | editor: edit → mergeFrom × N → save | DONE |
| `split` | editor: extractPages × N chunks | DONE |
| `splitBySize` | editor: extractPages with binary search | DONE |
| `splitByBookmarks` | doc: planSplitByBookmarks → extractPages | DONE |
| `extractPages` | editor: edit → selectPages → save | DONE |
| `deletePages` | editor: edit → deletePage × N → save | DONE |
| `reorderPages` | editor: edit → selectPages → save | DONE |
| `movePage` | editor: edit → movePage → save | DONE |
| `rotatePages` | editor: edit → rotatePage × N → save | DONE |
| `rotateAllPages` | editor: edit → rotateAllPages → save | DONE |
| `flattenForms` | editor: edit → flattenForms → save | DONE |
| `applyRedactions` | editor: edit → applyRedactions → save | DONE |
| `compress` | editor: edit → optimizeImages → save(compress) | DONE |
| `embedFile` | editor: edit → embedFile → save | DONE |
| `eraseRegions` | editor: edit → eraseRegions → save | DONE |
| `addStamp` | editor: edit → addStamp → save | DONE |
| `addImageStamp` | editor: edit → addImageStamp → save | DONE |
| `watermark` | editor: edit → addWatermark(-1) → save | DONE |
| `encrypt` | editor: edit → save(encryption) | DONE |
| `decrypt` | editor: edit(pw) → save(removeEncryption) | DONE |
| `convertToPdfA` | editor: edit → convertToPdfA → save | DONE |
| `imagesToPdf` | builder: build → addPage+image × N → save | DONE |

---

## Runtime — the lane architecture

| Capability | Status |
|---|---|
| Per-op cancellation (`PdfTask.cancel()` on every engine method) | DONE |
| Instant dispose (kill every lane, no joins, same event-loop turn) | DONE |
| Lane budgets — queue, never fail (128 native threads / 64 web workers, FIFO waiters) | DONE |
| Pristine worker recycling under create+dispose churn (web) | DONE |
| Fire-and-forget error physics (cancelled silent, real failures loud) | DONE |
| Three web I/O modes (JSPI / Atomics / OPFS), identical suite | DONE |
| Item streaming over the job port (multi-result ops without buffering) | PLANNED — the job's result port is already a message channel: promote it to N interim messages + 1 terminal message. Inherits per-job cancel, instant kill, and the post-driven cleanup protocol unchanged; symmetric on web (worker postMessage). Never add a second transport method for this. |

---

## Summary — API surface

Totals for the five consumer surfaces above. Runtime, build, and roadmap
capabilities are tracked in their own sections.

| Category | Done | Planned |
|---|---|---|
| PdfDoc | 19 | 7 |
| PdfEditor | 40 | 27 |
| PdfBuilder | 34 | 0 |
| PdfStandalone | 4 | 0 |
| PdfSugar | 22 | 0 |
| **Total** | **119** | **34** |

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
| Link hook (`hook/link.dart`) | DONE | Forwards assets; on release drives the RecordUse trim lane (see **Binary size — trim**) |
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
modules need URLs, not raw bytes — `DataAsset` can't provide that.
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
| `panic=abort` size lever | WONT_DO | Native lane isolation IS `catch_unwind` (one bad PDF → typed error, engine survives); abort would crash the whole app. Wasm already ships abort via `release-small` — the JS worker boundary isolates there. Nothing left to win. |
| Writer monomorphization dedup | WONT_DO | Measured honestly: the dedupable Boxed-vs-Seek writer pairs are ~150-200 KB (2% of core) for dyn dispatch in the hottest safety-critical loop plus invasive upstream surgery. Bad trade. |
| `opt-level=z` size option | DONE | Asked for → shipped as `build: size` (opt-level z on the release profile). Never the default (it costs CPU on every op); opt-in per app via the pubspec `build:` key. |
| `extract` capability | DONE | Extraction + search + classification (+ CJK CID tables). Cut via root gates — three dispatch fns gated, LTO deleted the 2.1 MB web with zero document.rs surgery. Core promise is now parse/write/edit/forms/builder. `office` requires `extract` engine-side. All remaining per-op cuts measured at 10-60 KB each: not worth their surface. |
| Lazy fallback-font loading (runtime memory, not binary size) | PLANNED, deprioritized | `registerFallbackFont(bytes)` keeps the font resident: one shared copy on native, one per web worker. Bytes-in is the primitive, so a lazy `DataSource` registration (load on first CJK/emoji bake, per worker) and an optional release call can layer on later without breaking the API. Pick up only on real memory-pressure reports from web apps — and design the API surface carefully at that point, not before. |

### When the futures arrive — tracked triggers + exact approach

| Trigger to watch | Signal | Approach when it lands |
|---|---|---|
| RecordUse experiment stabilizes (dart-lang/native#2902 for instance methods; the SDK record-use experiment flag) | recordings appear in release link hooks | Nothing to build — the lane self-activates. Validate with `trim-detector: compare` on example/ (diff recorded vs scan keep-sets). When instance methods land: delete the `record_use_shim*.dart` files, annotate the ops directly. Separately: `record_use` is pinned `^0.6.0` because ≥1.0.0 needs `meta ^1.19` and Flutter stable pins meta 1.18 — bump the pin when a Flutter stable ships meta ≥1.19 (same API for our use). When trust is earned: consider promoting the default detector. |
| Dart static linking (dart-lang/sdk#49418; `StaticLinking` in code_assets implemented) | hooks accept static libs; SDK defines symbol/asset-tag references | 1) Wire Dart-side references to the `pdf_op_<name>_anchor` symbols (one per public op, using whatever asset-tag syntax ships). 2) Swap `ops/registry.rs` from the explicit table to link-section collection. 3) Add a gc-sections assertion to shake-audit. The detector becomes a cross-check; the linker becomes the mechanism. Units don't change. |
| Web build hooks (dart-lang/native#988) | hooks run for web targets | Web already reads the same `keep:`/`build:` pubspec block native does (via `setup` → `user_defines.dart` → `resolveKeepPlan`). Remaining win: fold that into an automatic web build hook so the manual `setup` step disappears entirely — wasm gains the same no-command auto flow as native. |
| Wasm component model in dart2wasm (dart-lang/sdk#56366) | dart2wasm emits/links components | Express op units as WIT interface functions (one-to-one mapping already); component-level linking replaces export-root trimming on web. Furthest future — the scan covers web until then. |

## Built in the engine, not shipped

These exist in the Rust core (`pdf_oxide`) behind cargo features but are excluded from the shipped `pdf_manipulator` feature sets (`build.json`) — they pull the ONNX runtime and large model files that would bloat every install. No Dart op exposes them today; with the feature off, the FFI returns `_ERR_UNSUPPORTED`. A size tradeoff, not a missing capability.

| Capability | Engine support | Why not shipped |
|---|---|---|
| OCR (scanned-page text) | PaddleOCR — DBNet++ detect → SVTR recognize — via ONNX; auto-detects scanned vs native pages; gated on `ocr` / `ocr-tract` / `wasm-ocr` | Adds the ONNX runtime + ~12.5 MB of models per install |
| Table extraction | `table_extractor` + `spatial_table_detector`, gated on `table-ml` / `ml` | Same ML stack (ONNX + model weight) |

The planned route to expose these without bloating the default is per-feature opt-in via `user_defines` (see [Binary size — feature trimming](#binary-size--feature-trimming)): the consumer enables the cargo feature in their own pubspec and accepts the ONNX runtime + model download. It's deliberately opt-in, not default — and still extra setup on the consumer's side, not a clean built-in. Open an issue to push it forward; demand is what decides priority.

---

## Not planned — open an issue if you need it

| Feature | Where it stands |
|---|---|
| PDF viewer widget | `doc.render(...)` already gives you page images, so a viewer is *buildable* on top — but [pdfx](https://pub.dev/packages/pdfx) and [flutter_pdfview](https://pub.dev/packages/flutter_pdfview) already do viewing well, and this package's focus is manipulation, not UI. Not prioritized; open an issue if a first-party viewer would help. |

---

## Deferred — surfaced by the test overhaul (2026-06-12)

| Capability | Status | Why |
|---|---|---|
| Builder output hardening (compress streams by default; richer typesetting) | PLANNED | Builder output is valid but naively shaped vs real-world PDFs; product decision, deliberately separate from the test overhaul. |
| Attachment-listing read API | PLANNED | embedFile currently has no semantic presence proof — tests fall back to structural checks until attachments can be enumerated. |
| Typed wire error codes (PdfWrongPassword, PdfCorrupted, … from Rust) | PLANNED | Engine failures are typed as `PdfEngineError(message)` today; per-kind types need error codes on the wire protocol. |

---

## Test infrastructure gaps

| Gap | Detail | Status |
|---|---|---|
| Field-action silent no-op | `fieldKeystroke`/`fieldFormat`/`fieldValidate`/`fieldCalculate` attach to the most-recently-added field; on a page with no field the call silently does nothing. A typed error would surface caller bugs. | PLANNED |
| True O(1) memory verification | The Dart-side chunk guards catch transport violations only; Rust-internal Vec accumulation is invisible to them. Wire a tracking allocator into the Rust test harness: baseline → run op on a 50MB+ input → assert peak allocation stays under a fixed bound (~5MB). | PLANNED |
