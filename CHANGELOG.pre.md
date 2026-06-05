# Prerelease Changelog

<!--
  PRERELEASE VERSIONS ONLY. Stable releases go in CHANGELOG.md.

  How to add a prerelease:
  1. Add a new ## heading at the top with the prerelease version (e.g. 1.1.0-dev.0)
  2. Write a summary of what changed SINCE THE PREVIOUS ENTRY IN THIS FILE
     - First prerelease after a stable: changes since the last stable version
     - Subsequent prereleases: changes since the previous prerelease
  3. Commit and push to dev (via PR)
  4. CI reads the version from the top ## heading, tags, and publishes as prerelease

  Rules:
  - Version in ## heading is the source of truth for the prerelease version
  - Each entry covers changes since the PREVIOUS entry in THIS file (not CHANGELOG.md)
  - pub.dev gets a FILTERED version of this file as CHANGELOG.md:
    only versions published on pub.dev + the current one are included,
    unpublished intermediate versions are merged into collapsibles
  - When the stable release ships, write the full summary in CHANGELOG.md
    covering everything since the last stable — this file is not consulted
  - Entries here are permanent history — don't delete old entries
  - DO NOT add commit lists here — CI auto-appends them at publish time
-->

## 1.0.1-dev.0

- Added public API doc comments across all exported classes and methods
- Minimum Android API corrected from 35 to 21 (Android 5.0)
- Setup command is now `flutter pub run pdf_manipulator:setup` (avoids triggering native build hooks with `dart run`)

## 1.0.0-dev.0

Complete ground-up rewrite. New engine, new API, every platform. See the [migration guide](docs/MIGRATION.md) for upgrading from the old Android-only version.

### What changed

- **Engine:** pdf_oxide (Rust, MIT/Apache-2.0) replaces the previous Android-only backend
- **Platforms:** iOS, Android, macOS, Windows, Linux, Web — previously Android only
- **API:** Instance-based `Pdf()` with `dispose()`. Batch editing via `pdf.edit(source)`. Create from scratch via `pdf.build()`
- **I/O:** `DataSource` in, `DataSink` out — no file paths, no `dart:io`. Same code on every platform. Engine reads only what it needs, never the full file
- **Errors:** Typed `PdfError` sealed class — no more `PlatformException`
- **Performance:** Every operation runs off the main thread. Zero UI jank. No full-file buffers
- **SDK:** Requires Dart >=3.10.0

### Capabilities

- Open and inspect (page count, version, dimensions, metadata, encryption, permissions)
- Merge, split, split by size, split by bookmarks
- Extract pages, delete pages, reorder, move page
- Rotate (per-page and all pages)
- Compress with image optimization
- Watermark (styled, positioned — sealed PdfWatermarkPosition with center/corner/tiled/exact, foreground/background layer)
- Encrypt (4 algorithms, 8 permission flags) and decrypt
- Digital signatures (inspect, verify, sign via PKCS12/PEM)
- Extract text, Markdown, HTML
- Search text with bounding rectangles
- Render pages to RGBA images (Stream)
- Extract embedded images (Stream)
- PDF/A and PDF/UA validation
- Page and document classification
- Convert to/from DOCX, PPTX, XLSX
- PdfEditor — open once, mutate many, save once (full rewrite or incremental)
- PdfBuilder — create PDFs from scratch (text, headings, images, form fields, links, columns, footnotes)
- Form fields: text, checkbox, combo box, push button, signature
- Stamp annotations (14 built-in types + image stamps)
- Font unembedding, image resize, crop margins
- Embed files, erase regions, flatten forms/annotations
- Redaction (add, count, apply, scrub metadata)
- PDF/A conversion
- Resource pruning on GC save
- Images to PDF
- 21 sugar methods on PdfOperations extension
