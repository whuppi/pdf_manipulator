# pdf_manipulator example

A Flutter app exercising every `pdf_manipulator` feature. Pick a
built-in sample, tap an operation, see the result. Samples are
generated in memory by the package's own builder — the demo doubles
as a live builder showcase, and nothing touches the filesystem. Runs
on macOS, iOS, Android, Windows, Linux, and web.

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

# web — all 3 I/O modes (JSPI, Atomics, OPFS), from the package root:
cd ..
make test-example-web
```

The smoke test drives every API surface through the same demo
samples. Force a specific web I/O mode with
`--dart-define=PDF_IO_MODE=jspi|atomics|opfs`.

## What's inside

Seven tabs, one per area of the API:

| Tab | API | What it covers |
|---|---|---|
| **Runtime** | `Pdf` / `PdfTask` | The lane architecture live: detected I/O mode, parallel ops on one instance, per-op cancellation, instant dispose mid-flight |
| **Doc** | `PdfDoc` | Open, page info, extract text (plain/markdown/html), search, render, extract images, signatures, validate PDF/A + PDF/UA, classify, bookmarks |
| **Sugar** | `PdfSugar` | Every one-shot convenience method — split, splitBySize, splitByBookmarks, extract, delete, reorder, move, rotate, compress, watermark (all positions + layers), encrypt, decrypt, flatten, redact, embed, erase, stamp, image stamp, PDF/A, images→PDF |
| **Standalone** | `PdfStandalone` | Sign (PEM), convertTo (DOCX/PPTX/XLSX), convertToPdf (round-trip), extractPages |
| **Editor** | `PdfEditor` | Metadata get/set, scrub, rotate, delete, move, select, merge, mediaBox, optimize images, unembed fonts, watermark, stamp, image stamp, embed, erase, crop, flatten forms + annotations, form field value, redaction lifecycle, PDF/A, save options (full/incremental/encrypted/remove), chained mutations |
| **Builder** | `PdfBuilder` | A4/Letter/custom pages, text/heading/paragraph, font, space, horizontalRule, columns, newline, newPageSameSize, watermark, image, all form fields (textField/checkbox/comboBox/pushButton/signatureField/radioGroup), field JavaScript (keystroke/format/validate/calculate), links (URL/page), footnotes |
| **Merge** | `PdfSugar.merge` | Pick N samples, drag to reorder, merge |

## One file on purpose

The whole app lives in `lib/main.dart` because pub.dev renders that
file as the package's Example tab — splitting it would hide
everything else from that page.

## No `dart:io`

Every sample is built in memory and every result stays in memory.
The same code compiles unchanged for native and web.
