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

## Dispatch coverage

All operations — read, stream, and edit — go through `dispatch.rs` on both platforms.

---

## Planned

No planned items.

---

## Out of scope

| Feature | Why not |
|---|---|
| OCR | Requires Tesseract or similar — not a PDF primitive |
| Table extraction | Heuristic-heavy — better served by dedicated libraries |
| PDF viewer widget | Use pdfx or flutter_pdfview — they're built for viewing, we're built for manipulation |
