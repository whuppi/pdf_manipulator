# Getting Started with PDF Oxide Go Bindings

## Prerequisites

- **Go 1.21+**
- **One of:**
  - **CGo + C compiler** (gcc, clang, or MSVC) — default, full API coverage.
  - **CGO_ENABLED=0** — pure-Go build via the [purego](https://github.com/ebitengine/purego) backend (read-side API only).

No Rust toolchain required — the native library is downloaded from GitHub Releases on demand.

## Installation

```bash
go get github.com/yfedoseev/pdf_oxide/go

# CGo flow (default, full API) — downloads the staticlib and prints CGO_* env vars:
go run github.com/yfedoseev/pdf_oxide/go/cmd/install@latest

# Pure-Go flow (CGO_ENABLED=0) — downloads the shared lib and prints PDF_OXIDE_LIB_PATH:
go run github.com/yfedoseev/pdf_oxide/go/cmd/install@latest -shared
```

The installer drops the library under `os.UserCacheDir()/pdf_oxide/v<ver>/`
— `~/.cache/pdf_oxide/` on Linux, `~/Library/Caches/pdf_oxide/` on macOS,
`%LocalAppData%\pdf_oxide\` on Windows. Override with `-dir <path>`.

Import as:

```go
import pdfoxide "github.com/yfedoseev/pdf_oxide/go"
```

## Your First Program

```go
package main

import (
    "errors"
    "fmt"
    "log"

    pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func main() {
    doc, err := pdfoxide.Open("document.pdf")
    if err != nil {
        if errors.Is(err, pdfoxide.ErrDocumentNotFound) {
            log.Fatalf("file not found")
        }
        log.Fatal(err)
    }
    defer doc.Close()

    count, _ := doc.PageCount()
    major, minor, _ := doc.Version()
    fmt.Printf("%d pages, PDF %d.%d\n", count, major, minor)

    // Extract text from the first page
    text, err := doc.ExtractText(0)
    if err != nil {
        log.Fatal(err)
    }
    fmt.Println(text)
}
```

## Basic Usage

### Open a PDF

```go
doc, err := pdfoxide.Open("document.pdf")
if err != nil {
    log.Fatal(err)
}
defer doc.Close()
```

Additional openers:

```go
doc, err := pdfoxide.OpenFromBytes(data)         // []byte
doc, err := pdfoxide.OpenWithPassword(path, pw)  // encrypted PDFs
doc, err := pdfoxide.OpenReader(r)               // any io.Reader
```

### Extract Text

The API is flat — methods live directly on `*PdfDocument`, not on sub-managers.

```go
// Plain text for page 0
text, err := doc.ExtractText(0)

// Markdown for page 0
md, err := doc.ToMarkdown(0)

// HTML for page 0
html, err := doc.ToHtml(0)

// Whole-document variants
allText, err     := doc.ExtractAllText()
allMarkdown, err := doc.ToMarkdownAll()
allHtml, err     := doc.ToHtmlAll()
```

### Structured Extraction

```go
// Words with bounding boxes
words, _ := doc.ExtractWords(0)
for _, w := range words {
    fmt.Printf("%q @ (%.1f, %.1f)\n", w.Text, w.X, w.Y)
}

// Lines
lines, _ := doc.ExtractTextLines(0)

// Characters
chars, _ := doc.ExtractChars(0)

// Tables
tables, _ := doc.ExtractTables(0)

// Text inside a rectangle
region, _ := doc.ExtractTextInRect(0, 100, 100, 400, 200)
```

### Search

```go
// Case-insensitive search of a single page
pageHits, err := doc.SearchPage(0, "invoice", false)

// Case-sensitive search of the whole document
allHits, err := doc.SearchAll("Invoice", true)
for _, h := range allHits {
    fmt.Printf("page %d: %s\n", h.Page, h.Text)
}
```

`SearchResult` has `Text`, `Page`, `X`, `Y`, `Width`, `Height`.

### Page Information

```go
info, err := doc.PageInfo(0)
if err != nil {
    log.Fatal(err)
}
fmt.Printf("%.0f x %.0f pt, rotation %d\n", info.Width, info.Height, info.Rotation)
fmt.Printf("MediaBox: %+v\n", info.MediaBox)
fmt.Printf("CropBox:  %+v\n", info.CropBox)
```

### Fonts, Images, Annotations, Elements

```go
fonts, _ := doc.Fonts(0)
for _, f := range fonts {
    fmt.Println(f.Name, f.Type, f.IsEmbedded)
}

images, _ := doc.Images(0)
for _, img := range images {
    fmt.Printf("%dx%d %s\n", img.Width, img.Height, img.Format)
}

anns, _ := doc.Annotations(0)
for _, a := range anns {
    fmt.Printf("%s by %s: %s\n", a.Subtype, a.Author, a.Content)
}

elements, _ := doc.PageElements(0)
for _, e := range elements {
    fmt.Printf("%s: %q\n", e.Type, e.Text)
}
```

## Editing a Document

`OpenEditor` gives you a mutable handle:

```go
editor, err := pdfoxide.OpenEditor("input.pdf")
if err != nil {
    log.Fatal(err)
}
defer editor.Close()

// Apply several metadata fields in one call
if err := editor.ApplyMetadata(pdfoxide.Metadata{
    Title:  "My Report",
    Author: "Jane Doe",
}); err != nil {
    log.Fatal(err)
}

// Or individual setters
_ = editor.SetSubject("Quarterly results")

// Page operations
_ = editor.SetPageRotation(0, 90)
_ = editor.DeletePage(5)
_ = editor.MovePage(3, 0)
_ = editor.CropMargins(36, 36, 36, 36)
_ = editor.FlattenAllAnnotations()

// Save
if err := editor.Save("output.pdf"); err != nil {
    log.Fatal(err)
}
```

## Creating PDFs

```go
md, _ := pdfoxide.FromMarkdown("# Hello\n\nBody paragraph.")
defer md.Close()
_ = md.Save("hello.pdf")

html, _ := pdfoxide.FromHtml("<h1>Hello</h1>")
defer html.Close()

txt, _ := pdfoxide.FromText("Hello, world.")
defer txt.Close()

img, _ := pdfoxide.FromImage("photo.jpg")
defer img.Close()
```

## Rendering

```go
// Format: 0 = PNG, 1 = JPEG
img, err := doc.RenderPage(0, 0)
if err != nil {
    log.Fatal(err)
}
defer img.Close()
_ = img.SaveToFile("page0.png")
```

`RenderPageZoom(page, zoom, format)` and `RenderThumbnail(page, size, format)` are also
available.

## Concurrency

`*PdfDocument` protects reads with an internal `sync.RWMutex`, so you can use a single
document from many goroutines:

```go
var wg sync.WaitGroup
pageCount, _ := doc.PageCount()

for i := 0; i < pageCount; i++ {
    wg.Add(1)
    go func(page int) {
        defer wg.Done()
        text, _ := doc.ExtractText(page)
        _ = text
    }(i)
}
wg.Wait()
```

`DocumentEditor` serializes writes, but don't pipeline edits from multiple goroutines —
mutate from a single goroutine.

## Error Handling

Every operation returns an error. Use `errors.Is` to compare against sentinel values:

```go
text, err := doc.ExtractText(0)
if err != nil {
    switch {
    case errors.Is(err, pdfoxide.ErrDocumentClosed):
        log.Print("document is closed")
    case errors.Is(err, pdfoxide.ErrInvalidPageIndex):
        log.Print("invalid page index")
    default:
        log.Printf("unexpected: %v", err)
    }
}
```

Available sentinels:

```
ErrInvalidPath        ErrDocumentNotFound   ErrInvalidFormat
ErrExtractionFailed   ErrParseError         ErrInvalidPageIndex
ErrSearchFailed       ErrInternal           ErrDocumentClosed
ErrEditorClosed       ErrCreatorClosed      ErrIndexOutOfBounds
ErrEmptyContent
```

The concrete type is `*pdfoxide.Error`, carrying a numeric `Code` and `Message`:

```go
var e *pdfoxide.Error
if errors.As(err, &e) {
    fmt.Printf("code=%d message=%s\n", e.Code, e.Message)
}
```

## Examples

See the `examples/` directory at the repository root for runnable programs covering
extraction, search, editing, rendering, and creation.

## Testing

```bash
# Run unit tests
go test ./...

# With race detector
go test ./... -race

# With coverage
go test ./... -cover

# Single test
go test ./... -run TestExtractText
```

## Vet

```bash
go vet ./...
```

## Troubleshooting

### "Failed to load library"

Make sure you've run the installer. The library lives under the user cache dir:

```bash
# Linux
ls -la ~/.cache/pdf_oxide/v*/lib/*/libpdf_oxide.*
# macOS
ls -la ~/Library/Caches/pdf_oxide/v*/lib/*/libpdf_oxide.*
# Windows (PowerShell)
dir $env:LocalAppData\pdf_oxide\v*\lib\*\*
```

If missing, run `go run github.com/yfedoseev/pdf_oxide/go/cmd/install@latest`
(or `-shared` if you're using the purego backend).

### CGo compilation errors

Install a C toolchain:

- **Linux**: `sudo apt-get install build-essential`
- **macOS**: `xcode-select --install`
- **Windows**: Visual Studio Build Tools

### macOS Gatekeeper

macOS may require code signing on the bundled dylib:

```bash
codesign -f -s - /path/to/libpdf_oxide.dylib
```

## Next Steps

1. **Read `README.md`** for a full API tour.
2. **Check `QUICK_REFERENCE.md`** for a condensed cheat sheet.
3. **Browse `examples/`** for runnable programs.
4. **Open pkg.go.dev** for GoDoc-generated reference: `https://pkg.go.dev/github.com/yfedoseev/pdf_oxide/go`.

## Support

For issues or questions, please open an issue on the main PDF Oxide GitHub repository.
