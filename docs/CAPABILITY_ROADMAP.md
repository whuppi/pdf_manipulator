# pdf_manipulator — Capabilities

What's shipped, what's next, what's out of scope.

For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md).
For bridge internals see [`BRIDGE_ARCHITECTURE.md`](BRIDGE_ARCHITECTURE.md).
For public API see [`API_GOLD.md`](API_GOLD.md).

---

## Infrastructure

| Capability | Status |
|---|:---:|
| Three-layer architecture (API / Bridge / Engine) | ✓ |
| PdfBridge abstract interface — NativeBridge + WebBridge implementations | ✓ |
| Sealed PdfPages type for page-scoped operations | ✓ |
| PdfSource (random-access reader) + PdfSink (sequential writer) | ✓ |
| Rust thread pool (raw pthreads, `available_parallelism() / 2`) | ✓ |
| CallbackReader: condvar + NativeCallable.listener (engine reads ranges on demand) | ✓ |
| CallbackWriter: condvar + NativeCallable.listener (engine writes chunks as produced) | ✓ |
| bumpalo arena allocator per operation (drop arena = free ALL memory) | ✓ |
| allo-isolate for result posting (Dart_PostCObject from any thread) | ✓ |
| Shared buffer layout (defined once in Rust, mirrored in Dart) | ✓ |
| Web Worker pool (hardwareConcurrency / 2) with session-based ops | ✓ |
| OPFS SyncAccessHandle (JsCallbackReader reads from disk, not RAM) | ✓ |
| OPFS cleanup registry (tracks temp files, cleans on error/dispose) | ✓ |
| Stream\<T\> for extractImages and render (per-item, one at a time) | ✓ |
| Cooperative cancel (flag + condvar signal) + force-kill (pthread_cancel / Worker.terminate) | ✓ |
| Instant dispose (cancel all + kill isolate/workers + arena drop + buffer free) | ✓ |
| Read timeout (pthread_cond_timedwait 30s) | ✓ |
| Conditional import dispatch (bridge_factory.dart) | ✓ |
| Build hook with Rust source in dependencies (auto-recompile on Rust changes) | ✓ |
| Pre-compiled binaries (macOS, iOS, Android, Linux, Windows) | ✓ |
| CI/CD pipeline (cross-compile 13 targets + WASM → GitHub Releases) | ✓ |
| Instance-based API: `Pdf()` with `dispose()` | ✓ |
| Sealed PdfError hierarchy | ✓ |

---

## Operations — Pdf class (one-shot)

| Operation | Status |
|---|:---:|
| open (inspect: page count, version, dimensions, metadata, encryption) | ✓ |
| merge N PDFs | ✓ |
| split by page count | ✓ |
| splitBySize | ✓ |
| extractPages | ✓ |
| deletePages | ✓ |
| reorderPages | ✓ |
| movePage | ✓ |
| rotatePages (per-page) | ✓ |
| rotateAllPages | ✓ |
| flattenForms | ✓ |
| applyRedactions | ✓ |
| embedFile | ✓ |
| eraseRegions | ✓ |
| compress (stream recompression + GC + image optimization) | ✓ |
| extract (text / markdown, via PdfExtractionFormat) | ✓ |
| search (query + PdfPages) | ✓ |
| watermark (text, positioned, styled) | ✓ |
| encrypt (PdfEncryptionConfig with algorithm + permissions) | ✓ |
| decrypt | ✓ |
| sign (PKCS12 certificate) | ✓ |
| addStamp (standard stamp annotations) | ✓ |
| addImageStamp | ✓ |
| imagesToPdf | ✓ |
| render (PdfPages + PdfRenderSize → Stream\<PdfRenderedPage\>) | ✓ |
| extractImages (PdfPages → Stream\<PdfImage\>) | ✓ |
| getSignatures | ✓ |
| verifySignatures | ✓ |
| validatePdfA | ✓ |
| validatePdfUa | ✓ |

---

## Operations — PdfEditor (batch edit)

| Operation | Status |
|---|:---:|
| openEditor (persistent handle, read source via streaming) | ✓ |
| setTitle / setAuthor / setSubject / setKeywords | ✓ |
| getTitle / getAuthor / getSubject / getKeywords | ✓ |
| rotatePage / rotateAllPages | ✓ |
| deletePage | ✓ |
| movePage | ✓ |
| mergeFrom (PdfSource) | ✓ |
| extractPages (PdfSink) | ✓ |
| optimizeImages | ✓ |
| unembedStandardFonts | ✓ |
| addWatermark (with PdfWatermarkStyle + PdfWatermarkPosition) | ✓ |
| addStamp / addImageStamp | ✓ |
| embedFile / eraseRegions | ✓ |
| flattenForms / flattenAllAnnotations | ✓ |
| setFormFieldValue | ✓ |
| cropMargins | ✓ |
| convertToPdfA | ✓ |
| resizeImage | ✓ |
| save (PdfSink + PdfSaveOptions with optional encryption) | ✓ |
| getPageMediaBox | ✓ |
| pageCount / version / isModified | ✓ |

---

## Operations — PdfBuilder (create from scratch)

| Operation | Status |
|---|:---:|
| createBuilder | ✓ |
| setTitle / setAuthor / setSubject / setKeywords | ✓ |
| addA4Page / addLetterPage / addPage(custom size) | ✓ |
| Page: font, at, text, heading, paragraph, space, horizontalRule | ✓ |
| Page: image, watermark | ✓ |
| Page: textField, checkbox, comboBox, pushButton, signatureField, radioGroup | ✓ |
| Page: fieldKeystroke, fieldFormat, fieldValidate, fieldCalculate | ✓ |
| Page: linkUrl, linkPage | ✓ |
| Page: footnote, columns, newline, newPageSameSize | ✓ |
| save (PdfSink + PdfSaveOptions) | ✓ |

---

## Tests

| Suite | Count |
|---|:---:|
| Rust bridge (unit + integration) | 27 |
| Dart native bridge e2e | 44 |
| Dart Layer 1 API (Pdf + PdfEditor + PdfBuilder) | 44 |
| **Total** | **115** |

---

## Next up

| Task | Status |
|---|---|
| Web e2e tests (asset server for worker.js + WASM in `dart test -p chrome`) | Blocked |
| Rewrite example app + integration test for new API | Not started |
| Final fork audit (diff against upstream, clean stale patches) | Not started |
| README rewrite with new API examples | Not started |

---

## Planned (engine doesn't support yet)

| Feature | Why deferred |
|---|---|
| addRedaction, redactionCount, scrubMetadata | Needs Rust bridge wiring for redaction tracking |
| planSplitByBookmarks, splitByBookmarks | pdf_oxide has bookmarks but no split-by-bookmark API |
| convertTo (PDF → DOCX/PPTX/XLSX) | pdf_oxide v0.3.48+, not shipped upstream |
| convertToPdf (DOCX/PPTX/XLSX → PDF) | Same |
| classifyPage, classifyDocument | Not in pdf_oxide, needs ML/heuristic engine |

---

## Out of scope

| Feature | Why not |
|---|---|
| OCR | Requires Tesseract or similar — not a PDF primitive |
| Table extraction | Heuristic-heavy — better served by dedicated libraries |
| Barcode/QR generation | Not a PDF concern — compose with a barcode package |
