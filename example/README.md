# pdf_manipulator example

A Flutter app exercising every `pdf_manipulator` feature. Pick a PDF, tap an operation, see the result. Runs on macOS, iOS, Android, Windows, Linux, and web.

## Run

```bash
cd example

# native
fvm flutter run

# web (setup copies JS + WASM into web/pdf_manipulator/)
flutter pub run pdf_manipulator:setup --force
fvm flutter run -d chrome
```

## Integration tests

```bash
# native (macOS)
cd example
fvm flutter test integration_test/pdf_smoke_test.dart -d macos

# web — all 3 I/O modes (JSPI, Atomics, OPFS) from package root:
cd ..
make test-example-web
```

The integration test covers every API method with hardcoded minimal PDFs — no file picker needed. Force a specific web I/O mode with `--dart-define=PDF_IO_MODE=jspi|atomics|opfs`.

## What's inside

Six tabs, one per API surface:

| Tab | API | What it covers |
|---|---|---|
| **Doc** | `PdfDoc` | Open, page info, extract text (plain/markdown/html), search, render, extract images, signatures, validate PDF/A + PDF/UA, classify, bookmarks |
| **Sugar** | `PdfSugar` | Every one-shot convenience method — split, splitBySize, splitByBookmarks, extract, delete, reorder, move, rotate, compress, watermark (all positions + layers), encrypt, decrypt, flatten, redact, embed, erase, stamp, image stamp, PDF/A, images→PDF |
| **Standalone** | `PdfStandalone` | Sign (PEM), convertTo (DOCX/PPTX/XLSX), convertToPdf (round-trip), extractPages |
| **Editor** | `PdfEditor` | Metadata get/set (title/author/subject/keywords), scrub, rotate, delete, move, select, merge, mediaBox, optimize images, unembed fonts, watermark, stamp, image stamp, embed, erase, crop, flatten forms + annotations, form field value, redaction lifecycle, PDF/A, save options (full/incremental/encrypted/remove), chained mutations |
| **Builder** | `PdfBuilder` | A4/Letter/custom pages, text/heading/paragraph, font, space, horizontalRule, columns, newline, newPageSameSize, watermark, image, all form fields (textField/checkbox/comboBox/pushButton/signatureField/radioGroup), field JavaScript (keystroke/format/validate/calculate), links (URL/page), footnotes |
| **Merge** | `PdfSugar.merge` | Pick N PDFs, drag to reorder, merge |

## No `dart:io`

Uses `file_picker` with `withData: true` everywhere. Same code compiles for native and web.
