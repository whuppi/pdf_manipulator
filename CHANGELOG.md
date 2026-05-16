# Changelog

## 1.0.0 (2026-05-16)


### Bug Fixes

* .pubignore for vendor/ + compile WASM before web tests ([9a3dc68](https://github.com/whuppi/pdf_manipulator/commit/9a3dc68bc67091d11c6a4db313cad353c728952f))
* **ci:** checkout in workflows before local composite actions ([02e92a5](https://github.com/whuppi/pdf_manipulator/commit/02e92a59063679386e7a0deab44cb67b1a10ab95))
* WASM target + Rust warnings in release CI ([dfcd65f](https://github.com/whuppi/pdf_manipulator/commit/dfcd65fee6520d6555a81755bc3dc87e01302d3c))

## 1.0.0

Complete ground-up rewrite. New engine, new API, every platform. See the [migration guide](docs/MIGRATION.md) for upgrading from the old version.

### What changed

- **Engine:** pdf_oxide (Rust, MIT/Apache-2.0) replaces the previous Android-only backend
- **Platforms:** iOS, Android, macOS, Windows, Linux, Web — previously Android only
- **API:** Instance-based `Pdf()` with `dispose()`. Batch editing via `Pdf.edit(bytes)`. Create from scratch via `Pdf.build()`
- **I/O:** `Uint8List` in and out — no file paths, no `dart:io`
- **Errors:** Typed `PdfError` sealed class — no more `PlatformException`
- **Threading:** Every operation runs off the main thread (worker isolate on native, Web Worker on web)
- **SDK:** Requires Dart >=3.10.0

### New capabilities

- Render pages to images
- Extract embedded images
- Extract text, Markdown, HTML, plain text
- Search text with positions
- Digital signatures (inspect, verify, sign)
- PDF/A and PDF/UA validation
- PdfEditor — batch mutations (parse once, save once)
- PdfBuilder — create PDFs from scratch with text, images, form fields
- Form fields: text, checkbox, combo box, radio group, push button, signature, with JS validation
- Stamp annotations (16 types + custom + image stamps)
- Positioned watermarks with FixedPrint
- Font unembedding
- Full encryption: 4 algorithms, 8 permission flags (read + write)
- Image optimization, resize, crop margins
- Embed files, erase regions, flatten annotations, redaction
- PDF/A conversion

### All old features preserved

Merge, split, compress, rotate, reorder, delete pages, watermark, encrypt, decrypt, images to PDF, page info, validate.
