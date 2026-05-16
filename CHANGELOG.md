# Changelog

## 1.0.0

Complete ground-up rewrite. Every line of code is new. See the [migration guide](docs/MIGRATION.md) for a method-by-method mapping from the old API.

### Breaking changes

- New engine: pdf_oxide (Rust, MIT, cross-platform) replaces the previous Android-only backend
- API is now instance-based: `final pdf = Pdf(); pdf.merge(...)` — each instance owns its own worker (isolate on native, Web Worker on web)
- Call `pdf.kill()` when done to free the worker (in `dispose()`, `finally`, etc.)
- `PdfEditor` created via `PdfEditor(await pdf.openEditor(bytes))` instead of static `PdfEditor.open(bytes)`
- `PdfBuilder` created via `PdfBuilder(await pdf.createBuilder())` instead of static `PdfBuilder.create()`
- Input/output is `Uint8List` instead of file paths — no `dart:io` in the barrel
- Typed `PdfError` sealed class instead of `PlatformException`

### New capabilities (not in old package)

- Instance-based API — multiple `Pdf()` instances run in parallel, each on its own worker
- Cross-platform: iOS, Android, macOS, Windows, Linux, Web
- Render pages to images (full, fitted, thumbnail)
- Extract embedded images from pages
- Extract text, convert to Markdown / HTML / plain text
- Search text with page numbers and position rectangles
- Digital signatures (count, list, verify, sign with PKCS#12)
- PDF/A compliance validation (with error/warning counts)
- PDF/UA accessibility validation
- PdfBuilder — create PDFs from scratch (text, headings, images, watermarks)
- Flatten forms and annotations
- Redaction
- Metadata editing (title, author, subject, keywords)
- Embed file attachments
- Erase rectangular regions
- Font unembedding (`unembedStandardFonts`) — remove embedded standard 14 fonts to reduce file size
- Image optimization during compression (non-JPEG → JPEG when smaller)
- Image resize on page (DPI control) via PdfEditor
- Full encryption with 4 algorithms (RC4-40, RC4-128, AES-128, AES-256) and 8 permission flags (print, print-hq, modify, copy, annotate, fill-forms, accessibility, assemble)
- Positioned watermark with FixedPrint annotation (x, y, width, height, font name, font size)
- Web watermark — watermark now works on web via WASM (previously native-only)
- Stamp annotations (16 built-in types + custom) via PdfEditor
- Image stamp (`addImageStamp`) — stamp images (logos, signatures) onto pages
- Read encryption permissions and algorithm from existing PDFs
- Form field creation via PdfBuilder (text field, checkbox, combo box, radio group, push button, signature field)
- Radio group form fields
- Form field JavaScript actions (fieldKeystroke, fieldFormat, fieldValidate, fieldCalculate)
- comboBox takes options as positional param, pushButton takes caption as positional param
- Links in PdfBuilder (URL, page target)
- Layout: footnotes, multi-column text, newline, new-page-same-size
- Crop margins
- PDF/A conversion
- PdfEditor batch mutations (parse once, modify N times, save once)

### All old features preserved

- Merge multiple PDFs
- Split by page count, byte size, page numbers, ranges
- Delete pages
- Reorder pages
- Rotate pages (individual + all)
- Compress (stream recompression + garbage collection + image optimization)
- Watermark (text with font, rotation, opacity, color, per-page or selective)
- Encrypt (AES, user + owner password)
- Decrypt (open with password, save unencrypted)
- Images to PDF
- Page size info
- Validate / probe

### Architecture

- Cross-platform: conditional import selects native (FFI) or web (WASM) at compile time
- Each `Pdf()` instance is its own worker — isolate on native, Web Worker on web
- Native: typed message protocol with TransferableTypedData (one memcpy + O(1) transfer)
- Web: Web Worker + WASM with ArrayBuffer transfer (one memcpy + O(1) transfer)
- No closures cross isolate/worker boundaries
- Dual-path build hook — consumers get pre-built binaries from GitHub Releases (zero Rust), contributors compile from source
- CI/CD pipeline for automated builds and releases
- 329 FFI bindings generated via ffigen from pdf_oxide C header
- 29 C-ABI + 8 Rust-level patches on whuppi/pdf_oxide fork
- 302 native tests + 8 web tests, all passing
- Zero analyzer warnings
- MIT licensed (powered by pdf_oxide)
