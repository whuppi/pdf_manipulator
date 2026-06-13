# Prerelease Changelog

<!-- Prereleases. Add ## heading at top, CI handles the rest. -->

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

- Fixed release build routing for consumer builds ([PR #80](https://github.com/whuppi/pdf_manipulator/pull/80), [@Binary-Parse](https://github.com/Binary-Parse))
- Fixed Windows NDK linker `.cmd` extension for Android cross-compilation ([PR #81](https://github.com/whuppi/pdf_manipulator/pull/81), [@Binary-Parse](https://github.com/Binary-Parse))
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

Complete ground-up rewrite. New engine, new API, cross-platform. See the [migration guide](docs/MIGRATION.md) for upgrading from the old Android-only version.

### What changed

- **Engine:** pdf_oxide (Rust, MIT/Apache-2.0) replaces the previous Android-only backend
- **Targets:** iOS, Android, macOS, Windows, Linux, Web — previously Android only
- **API:** Instance-based `Pdf()` with `dispose()`. Batch editing via `pdf.edit(source)`. Create from scratch via `pdf.build()`
- **I/O:** `DataSource` in, `DataSink` out — no file paths, no `dart:io`. Same code on every target. Engine reads only what it needs, never the full file
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
