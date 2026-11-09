# pdf_manipulator example

A Flutter app that uses every feature of `pdf_manipulator`. Pick a
built-in sample, tap an operation and see the result. The package's own
builder makes the samples in memory, so the app also shows the builder,
and nothing touches the file system. It runs on macOS, iOS, Android,
Windows, Linux and the web.

## Run

```bash
cd example

# native
fvm flutter run

# web (setup copies JS + WASM into web/pdf_manipulator/)
fvm flutter pub run pdf_manipulator:setup --force web
fvm flutter run -d chrome
```

## Integration tests

```bash
# native (macOS)
cd example
fvm flutter test integration_test/pdf_smoke_test.dart -d macos

# web — all 4 I/O modes (JSPI, Atomics, OPFS, Asyncify), from the package root:
cd ..
make test-example-web
```

The smoke test drives every part of the API through the same demo
samples. Force a specific web I/O mode with
`--dart-define=PDF_IO_MODE=jspi|atomics|opfs|asyncify`.

## What's inside

Seven tabs, one per area of the API:

| Tab | API | What it covers |
|---|---|---|
| **Runtime** | `Pdf` / `PdfTask` | The runtime live: the detected I/O mode, parallel operations on one instance, cancellation of one operation, instant dispose in the middle of a job |
| **Doc** | `PdfDocument` | Reads: page info, text (plain, Markdown, HTML), search, render, extract images, signatures, PDF/A and PDF/UA validation, classification, bookmark ranges |
| **Sugar** | `editOnce`, `resave`, `split`, `compress`, `imagesToPdf` | The one-shot doors: split by pages, by size and by bookmarks, keep, remove, reorder, move, rotate, compress, watermark (all positions and layers), encrypt and decrypt with `PdfSave`, flatten, redact, attach, stamp, image stamp, PDF/A, images to PDF |
| **Standalone** | `sign`, `convertToOffice`, `convertFromOffice` | The doors that open no handle: sign (PEM), PDF to DOCX, PPTX and XLSX, and DOCX back to PDF |
| **Editor** | `PdfEditor` | Several edits, one parse, one save: metadata, rotate, remove, move, keep, append pages, media box, reduce images, unembed fonts, watermark, stamp, image stamp, attach, redact, crop, flatten forms and annotations, form field values, the queued redactions, PDF/A, and the save options (full, incremental, encrypted, encryption removed) |
| **Builder** | `PdfBuilder` | A4, Letter and custom pages, text, headings, paragraphs, fonts, space, horizontal rules, columns, tables, shapes, watermark, image, every form field (text field, checkbox, combo box, push button, signature field, radio group) with JavaScript actions, links, footnotes |
| **Merge** | `Pdf.merge` | Pick N samples, drag them into an order, merge |

## One file on purpose

The whole app lives in `lib/main.dart` because pub.dev renders that
file as the package's Example tab — splitting it would hide
everything else from that page.

## No `dart:io`

Every sample is built in memory and every result stays in memory.
The same code compiles unchanged for native and web.
