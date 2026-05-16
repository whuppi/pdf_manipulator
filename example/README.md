# pdf_manipulator example

A Flutter app that exercises every `pdf_manipulator` feature — pick a PDF, tap an operation, see the result. Runs on macOS, iOS, Android, Windows, Linux, and web.

## Run

```bash
# Native (macOS, Windows, Linux)
cd example
flutter run

# Web
cd example
dart run pdf_manipulator:setup   # one-time — installs WASM assets
flutter run -d chrome
```

## What's inside

Four tabs, each testing a different surface:

| Tab | What it tests |
|---|---|
| **Operations** | Pick one PDF, then run any method on it — split, merge, rotate, compress, watermark, encrypt, decrypt, extract text, search, render, validate, and more |
| **Merge** | Pick multiple PDFs, drag to reorder, merge into one |
| **Images → PDF** | Pick images, convert to a single PDF |
| **Editor** | `PdfEditor` batch mutations — metadata, rotation, watermark, compress, flatten, encrypt, all chained on one parse-save cycle |

## No `dart:io`

The example uses `file_picker` with `withData: true` everywhere. File bytes come from the picker, not from `File()`. Same code compiles for native and web.

## Web setup

The WASM binary and Web Worker need to be in `web/pdf_manipulator/`. Run once:

```bash
dart run pdf_manipulator:setup
```

After that, `flutter run -d chrome` works.
