# pdf_manipulator — Capabilities

What's shipped, what's planned, what's out of scope.

For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md).

---

## Infrastructure

| Capability | Status |
|---|:---:|
| Four-layer architecture (Consumer API / Transport / Host / Engine) | ✓ |
| Shared dispatch (`host/dispatch.rs`) — all ops (read + edit) go through one brain | ✓ |
| Symmetric file naming (bridge↔bridge, coordinator↔coordinator, wire↔wire, ffi_api↔wasm_api, ffi_encode↔wasm_encode) | ✓ |
| DataSource (random-access reader) + DataSink (sequential writer) | ✓ |
| Sealed PdfPages, PdfError, PdfEncryption types | ✓ |
| PdfSaveMode (fullRewrite / incremental) | ✓ |
| Rust thread pool (native) + Web Worker pool (web) — off main thread | ✓ |
| Condvar streaming I/O (native) + Atomics/OPFS (web) | ✓ |
| Stream\<T\> for render and extractImages (one item at a time) | ✓ |
| Arena allocator per operation (bumpalo) | ✓ |
| Build hook: compile from source (contributors) or download pre-built (consumers) | ✓ |
| Pre-built binary caching with SHA256 verification (offline builds work) | ✓ |
| Web setup script (`dart run pdf_manipulator:setup`) with version guard | ✓ |
| CI/CD: cross-compile 13 native targets + WASM | ✓ |
| Wire sync test (catches native/web parity drift) | ✓ |
| Resource pruning on GC save (scan content streams, prune unused Resources) | ✓ |

---

## Operations — Pdf (standalone)

| Operation | Status |
|---|:---:|
| open (page count, version, dimensions, metadata, encryption, permissions) | ✓ |
| extract (text / markdown / html, per-page or all) | ✓ |
| search (query + PdfPages → SearchResult with x,y,w,h) | ✓ |
| render (PdfPages → Stream\<RenderedPage\>) | ✓ |
| extractImages (PdfPages → Stream\<PdfImage\>) | ✓ |
| getSignatures / verifySignatures | ✓ |
| validatePdfA / validatePdfUa | ✓ |
| classifyPage / classifyDocument | ✓ |
| planSplitByBookmarks | ✓ |
| sign (PKCS12 / PEM) | ✓ |
| imagesToPdf | ✓ |
| convertTo (PDF → DOCX/PPTX/XLSX) / convertToPdf (reverse) | ✓ |

---

## Operations — PdfEditor (batch edit)

| Operation | Status |
|---|:---:|
| openEditor (persistent handle, streaming reader) | ✓ |
| selectPages / deletePage / movePage | ✓ |
| rotatePage / rotateAllPages | ✓ |
| setTitle / setAuthor / setSubject / setKeywords (get + set) | ✓ |
| mergeFrom (DataSource) | ✓ |
| addWatermark (PdfWatermarkStyle + sealed PdfWatermarkPosition + PdfWatermarkLayer) | ✓ |
| Sealed PdfWatermarkPosition (center / corner / tiled / exact — engine resolves per-page) | ✓ |
| PdfWatermarkLayer (foreground: annotation / background: content-stream behind page content) | ✓ |
| addStamp / addImageStamp | ✓ |
| embedFile / eraseRegions | ✓ |
| flattenForms / flattenAllAnnotations | ✓ |
| setFormFieldValue | ✓ |
| cropMargins / convertToPdfA / resizeImage | ✓ |
| unembedStandardFonts / optimizeImages | ✓ |
| addRedaction / redactionCount / applyRedactions / scrubMetadata | ✓ |
| save (DataSink + PdfSaveOptions: mode, compression, GC, encryption) | ✓ |
| getPageMediaBox / pageCount / version / isModified | ✓ |

---

## Operations — PdfBuilder (create from scratch)

| Operation | Status |
|---|:---:|
| setTitle / setAuthor / setSubject / setKeywords | ✓ |
| addA4Page / addLetterPage / addPage(custom size) | ✓ |
| text, heading, paragraph, space, horizontalRule, image, watermark | ✓ |
| textField, checkbox, comboBox, pushButton, signatureField, radioGroup | ✓ |
| fieldKeystroke, fieldFormat, fieldValidate, fieldCalculate | ✓ |
| linkUrl, linkPage, footnote, columns, newline, newPageSameSize | ✓ |
| save (DataSink) | ✓ |

---

## Operations — PdfOperations (21 sugar methods)

| Operation | Status |
|---|:---:|
| merge, split, splitBySize, splitByBookmarks | ✓ |
| extractPages, deletePages, reorderPages, movePage | ✓ |
| rotatePages, rotateAllPages | ✓ |
| flattenForms, applyRedactions | ✓ |
| embedFile, eraseRegions | ✓ |
| compress, watermark | ✓ |
| encrypt, decrypt | ✓ |
| addStamp, addImageStamp, convertToPdfA | ✓ |

---

## Dispatch coverage — zero violations

**Every** operation goes through `dispatch.rs` on both platforms. No exceptions.

| Category | Through dispatch | Rule enforced in |
|---|---|---|
| Read ops (open, extract, search, validate, classify, render, extractImages) | `dispatch::*` → ffi_encode / wasm_encode | wasm_api.rs, ffi_api.rs headers |
| Edit ops (select, delete, rotate, merge, watermark, compress, etc.) | `dispatch::edit_*` | wasm_api.rs `dispatchEdit*` methods |
| Editor queries (pageCount, isModified, pageMediaBox, redactionCount) | `dispatch::edit_get_metadata` / `edit_is_modified` / `edit_page_media_box` | wasm_api.rs, ffi_api.rs `bridge_editor_query` |
| Editor save | `dispatch::edit_save_with_options` / `edit_save_encrypted` | wasm_api.rs `dispatchEditSave*` |
| Sign | `dispatch::sign_via_editor` | Both platforms call same function |
| Convert (DOCX/PPTX/XLSX) | `dispatch::convert_to_format_writer` / `convert_from_format_writer` | wasm_api.rs `dispatchConvertTo/FromFormat` |
| Images to PDF | `dispatch::images_to_pdf_writer` / `images_to_pdf_bytes` | wasm_api.rs `dispatchImagesToPdf` |
| Builder metadata (title, author, etc.) | `dispatch::builder_set_title` etc. | wasm_api.rs `dispatchSet*` |
| Builder save | `dispatch::builder_save` / `builder_save_to_writer` | wasm_api.rs `dispatchBuild` |
| Builder page ops (font, text, image, etc.) | `dispatch::PageOp` enum + `dispatch::replay_page_ops` | wasm_api.rs `DispatchPageBuilder` |

worker.js calls ONLY `dispatch*` methods. ffi_api.rs calls ONLY `dispatch::*` functions. Each file has a rule header documenting violations. See individual file headers for the full rule.

---

## Bugs — FIXED (all 9 from behavioral test rewrite)

| Bug | Fix | Status |
|---|---|---|
| `redactionCount` always returns 0 | `bridge_editor_query` on thread pool (query code 3) | FIXED |
| `optimizeImages` always returns 0 | `bridge_editor_query` (query code 4) + `dispatch::edit_optimize_images` | FIXED |
| `unembedStandardFonts` always returns 0 | `bridge_editor_query` (query code 5) + `dispatch::edit_unembed_standard_fonts` | FIXED |
| Source page tree deadlock | All editor queries through thread pool, never sync FFI | FIXED |
| `getPageMediaBox` silent fallback to A4 | Through thread pool, returns real values | FIXED |
| `scrubMetadata` doesn't remove metadata | Changed to call `sanitize_document` instead of `apply_redactions_destructive` | FIXED |
| `getSignatures` can't find sign() output | `sign_pdf_streaming_with_field` writes AcroForm + field + widget + page annotation. `enumerate_signatures` extracts signer CN from CMS blob (strips zero-padding) | FIXED |
| `convertToPdfA` empty/invalid output | `convert_with_editor` (no commit_in_place materialization). Bundled Liberation fonts for WASM (no system font dependency) | FIXED |
| `bookmarkedPdf` fixture missing font | Rebuilt with proper `/Resources << /Font << /F1 >> >>`. Photo PNG fixture for optimizeImages tests | FIXED |

## Planned

Rewrite example app and verify README examples compile.

---

## Out of scope

| Feature | Why not |
|---|---|
| OCR | Requires Tesseract or similar — not a PDF primitive |
| Table extraction | Heuristic-heavy — better served by dedicated libraries |
| PDF viewer widget | Use pdfx or flutter_pdfview — they're built for viewing, we're built for manipulation |
