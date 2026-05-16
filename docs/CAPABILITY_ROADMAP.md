# pdf_manipulator — Capabilities

What's shipped, what's next, what's deliberately out of scope. For the architectural map, see [`ARCHITECTURE.md`](ARCHITECTURE.md). For update procedures, see [`UPDATING.md`](UPDATING.md).

---

## Infrastructure

| Capability | Status |
|---|:---:|
| Cross-platform architecture (conditional export: native FFI / web WASM / stub) | ✓ |
| PdfPlatform abstract interface (40+ methods, zero platform imports) | ✓ |
| PdfEditorHandle / PdfBuilderHandle / PdfPageBuilderHandle interfaces | ✓ |
| PdfEditor fully async (native FFI / web WASM) | ✓ |
| PdfBuilder fully async (native FFI / web WASM) | ✓ |
| Native: worker isolate + FFI, typed message protocol (no closures cross boundary) | ✓ |
| Native: TransferableTypedData (one memcpy + O(1) transfer — Dart's theoretical minimum) | ✓ |
| Map-based args protocol (no numbered field limits, self-documenting keys) | ✓ |
| Native: persistent handle map in worker isolate for editor/builder sessions | ✓ |
| Web: Web Worker + WASM, typed message dispatch | ✓ |
| Web: ArrayBuffer.slice + postMessage transfer list (one memcpy + O(1) transfer) | ✓ |
| One barrel, one import, zero `dart:ffi` in public surface | ✓ |
| Build hook (pre-built binary download, zero Rust for consumers) | ✓ |
| Pre-compiled binaries (macOS arm64/x64, iOS arm64/sim, Android arm64/arm/x64/x86) | ✓ |
| Pre-compiled binaries (Linux x64/arm64, Windows x64) | ✓ (CI) |
| CI/CD pipeline (cross-compile 13 targets + WASM → GitHub Releases) | ✓ |
| ffigen codegen (pdf_oxide.h → native_bindings.g.dart, 329 functions) | ✓ |
| Memory-safe FFI wrappers (bindings.dart) | ✓ |
| Instance-based API: `Pdf()` per-worker instance with `kill()` for lifecycle control | ✓ |
| Sealed PdfError hierarchy (11 subtypes) | ✓ |
| Example app (Flutter, macOS + Chrome verified, zero dart:io) | ✓ |
| Cargo features: icc + legacy-crypto + rendering + signatures | ✓ |
| Automated web browser tests (8 tests, Chrome, spawnHybridUri + shelf asset server) | ✓ |
| Transfer behavior tests (6 tests proving O(1) send, original-survives, round-trip) | ✓ |

---

## Inspect

| Capability | Status |
|---|:---:|
| Open + inspect (page count, version, dimensions, rotation, metadata) | ✓ |
| Page sizes + rotation query | ✓ |
| Page media box get | ✓ |
| Validate / probe | ✓ |
| PDF/A compliance validation (with error/warning counts) | ✓ |
| PDF/UA accessibility validation | ✓ |

---

## Structural

| Capability | Status |
|---|:---:|
| Merge N PDFs | ✓ |
| Split by page count | ✓ |
| Split by byte size | ✓ |
| Split by page numbers / ranges (extractPages) | ✓ |
| Delete pages | ✓ |
| Reorder pages | ✓ |
| Rotate pages (individual + all) | ✓ |
| Crop margins | ✓ |

---

## Content

| Capability | Status |
|---|:---:|
| Watermark (text, font, rotation, opacity, color, per-page or selective) | ✓ |
| Watermark positioned (x, y, width, height, font name, font size, FixedPrint) | ✓ |
| Stamp annotations (Approved, Draft, Confidential, + 11 more + Custom) | ✓ |
| Metadata set/get (title, author, subject, keywords) | ✓ |
| Set form field values | ✓ |
| Flatten forms | ✓ |
| Flatten all annotations | ✓ |
| Apply redactions | ✓ |
| Erase rectangular regions (white-out) | ✓ |
| Embed file attachments | ✓ |
| PDF/A conversion | ✓ |

---

## Extraction

| Capability | Status |
|---|:---:|
| Extract text (per page + all) | ✓ |
| To Markdown (per page + all) | ✓ |
| To HTML | ✓ |
| To plain text | ✓ |
| Extract embedded images from pages | ✓ |

---

## Search

| Capability | Status |
|---|:---:|
| Search text (per page + all) | ✓ |

---

## Security

| Capability | Status |
|---|:---:|
| Encrypt (AES, user + owner password) | ✓ |
| Encrypt with algorithm choice (RC4-40, RC4-128, AES-128, AES-256) | ✓ |
| Encrypt with permission flags (print, print-hq, modify, copy, annotate, fill-forms, accessibility, assemble) | ✓ |
| Read permissions from existing PDF (8 flags) | ✓ |
| Read encryption algorithm from existing PDF | ✓ |
| Decrypt (open with password + save unencrypted) | ✓ |
| Encrypted save via editor (with algorithm + permissions) | ✓ |

---

## Rendering

| Capability | Status |
|---|:---:|
| Render pages to images (renderPage, renderPageFit, renderPageThumbnail, renderAllPages) | ✓ |

---

## Images

| Capability | Status |
|---|:---:|
| Images to PDF | ✓ |
| Image optimization (non-JPEG → JPEG when smaller) | ✓ |
| Image resize on page (DPI control via width/height) | ✓ |
| Compress (stream + GC + image optimization) | ✓ |
| Unembed Standard 14 fonts (Helvetica, Times, Courier, Symbol, ZapfDingbats) | ✓ |

---

## Signatures

| Capability | Status |
|---|:---:|
| Digital signatures (count, list, verify, sign) | ✓ |

---

## Builder

| Capability | Status |
|---|:---:|
| PdfBuilder — create PDFs from scratch (text, headings, paragraphs, images, watermarks, metadata, custom page sizes, encrypted build) | ✓ |
| Form field creation: text field, checkbox, combo box, radio group, push button, signature field | ✓ |
| Field JavaScript actions (keystroke, format, validate, calculate) | ✓ |
| Links (URL, page target) | ✓ |
| Layout: footnotes, multi-column text, newline, new-page-same-size | ✓ |

---

## Editor

| Capability | Status |
|---|:---:|
| Combined operations via PdfEditor chaining | ✓ |
| Round-trip integrity (open → edit → save → reopen) | ✓ |

---

## Maintenance

| Task | Cadence |
|---|---|
| Bump pdf_oxide dependency | When upstream ships new C-ABI functions that replace a patch |
| Rebuild WASM | After every pdf_oxide bump or Rust patch change |
| Re-run ffigen | After every pdf_oxide header change |
| Refresh pre-built binaries (all platforms) | After every Rust-side change; CI automates when runners exist |

---

## Next up

| Capability | Category | Work needed |
|---|---|---|
| Font subsetting (re-subset existing embedded fonts) | Content | `subsetter` crate is a dependency, used writer-side only. Needs new Rust module: scan content streams for used glyphs, re-subset each font, replace in PDF. Substantial Rust engineering. |
| iOS / Android device testing | Infrastructure | Pre-built binaries exist; untested on real devices |
| Cooperative cancellation via shared atomic flag | Infrastructure | Requires patching pdf_oxide Rust functions to accept `*const bool cancel_flag` and check periodically; future optimization for CPU savings on heavy cancelled ops |

---

## Out of scope

| Feature | Why not |
|---|---|
| OCR (text recognition from scanned images) | Requires Tesseract or similar native dep; not a PDF primitive |
| Table detection / extraction | Heuristic-heavy; better served by dedicated libraries |
| Barcode / QR code generation for placement inside PDFs | Not a PDF concern; compose with a barcode package and use `PdfBuilder.image()` |

---

## The one-line summary

> **Full feature parity plus 40+ new capabilities. Instance-based `Pdf()` API with `kill()` for lifecycle control. Encryption with 4 algorithms + 8 permissions (read + write). Positioned watermarks + image stamps + 16 stamp types. Form creation (6 field types including radio groups + JS validation scripts). Font unembedding. Page rendering, image extraction, digital signatures, PDF/A + PDF/UA validation. Typed message protocol, one-memcpy + O(1) transfer. CI/CD cross-compiles 13 native targets + WASM on tag push → GitHub Releases. All watermark/stamp/image-stamp capabilities work on web. Next: font subsetting.**
