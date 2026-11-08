# PDF Oxide Go Bindings - Quick Reference

## Installation

```bash
go get github.com/yfedoseev/pdf_oxide/go
```

Import as:

```go
import pdfoxide "github.com/yfedoseev/pdf_oxide/go"
```

## Basic Usage

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
            log.Fatal("file not found")
        }
        log.Fatal(err)
    }
    defer doc.Close()

    count, _ := doc.PageCount()
    major, minor, _ := doc.Version()
    fmt.Printf("%d pages, PDF %d.%d\n", count, major, minor)

    text, _ := doc.ExtractText(0)
    fmt.Println(text)

    hits, _ := doc.SearchAll("keyword", false)
    fmt.Printf("Found %d matches\n", len(hits))

    info, _ := doc.PageInfo(0)
    fmt.Printf("Page 0: %.0f x %.0f pt\n", info.Width, info.Height)
}
```

---

## API Overview

### Opening Documents

```go
doc, err := pdfoxide.Open("file.pdf")
doc, err := pdfoxide.OpenFromBytes(data)
doc, err := pdfoxide.OpenWithPassword("file.pdf", "pw")
doc, err := pdfoxide.OpenReader(reader) // any io.Reader
defer doc.Close()
```

### Text Extraction

```go
// Single page
text, err     := doc.ExtractText(pageIndex)
markdown, err := doc.ToMarkdown(pageIndex)
html, err     := doc.ToHtml(pageIndex)
plain, err    := doc.ToPlainText(pageIndex)

// Whole document
allText, err     := doc.ExtractAllText()
allMarkdown, err := doc.ToMarkdownAll()
allHtml, err     := doc.ToHtmlAll()
allPlain, err    := doc.ToPlainTextAll()

// Structured
words, err := doc.ExtractWords(pageIndex)       // []Word
lines, err := doc.ExtractTextLines(pageIndex)   // []TextLine
chars, err := doc.ExtractChars(pageIndex)       // []Char
tables, err := doc.ExtractTables(pageIndex)     // []Table
paths, err  := doc.ExtractPaths(pageIndex)      // []Path

// Region-based
region, err := doc.ExtractTextInRect(pageIndex, x, y, w, h)
rWords, err := doc.ExtractWordsInRect(pageIndex, x, y, w, h)
nImg, err   := doc.ExtractImagesInRect(pageIndex, x, y, w, h)
```

### Search

```go
// Search a single page
pageHits, err := doc.SearchPage(pageIndex, "term", caseSensitive)

// Search the whole document
allHits, err := doc.SearchAll("term", caseSensitive)

// SearchResult fields: Text, Page, X, Y, Width, Height
for _, r := range allHits {
    fmt.Printf("page %d: %s @ (%.0f, %.0f)\n", r.Page, r.Text, r.X, r.Y)
}
```

### Page Information

```go
info, err := doc.PageInfo(pageIndex)
// PageInfo{Width, Height, Rotation, MediaBox, CropBox, ArtBox, BleedBox, TrimBox}

count, err   := doc.PageCount()
maj, min, _  := doc.Version()
hasSt, err   := doc.HasStructureTree()
```

### Resources

```go
fonts, err       := doc.Fonts(pageIndex)        // []Font
images, err      := doc.Images(pageIndex)       // []Image
annotations, err := doc.Annotations(pageIndex)  // []Annotation
elements, err    := doc.PageElements(pageIndex) // []Element
formFields, err  := doc.FormFields()            // []FormField
```

### Rendering

```go
// Format: 0 = PNG, 1 = JPEG
img, err := doc.RenderPage(pageIndex, 0)
defer img.Close()
img.SaveToFile("page.png")
raw := img.Data() // []byte

zoomed, _ := doc.RenderPageZoom(pageIndex, 2.0, 0)
defer zoomed.Close()

thumb, _ := doc.RenderThumbnail(pageIndex, 200, 0)
defer thumb.Close()
```

### Editing

```go
editor, err := pdfoxide.OpenEditor("in.pdf")
if err != nil {
    log.Fatal(err)
}
defer editor.Close()

// Metadata
title, _  := editor.Title()
author, _ := editor.Author()
_ = editor.SetTitle("New title")
_ = editor.SetAuthor("Jane Doe")

// Or apply several fields at once
_ = editor.ApplyMetadata(pdfoxide.Metadata{
    Title:   "Report",
    Author:  "Jane Doe",
    Subject: "Q4",
})

// Page operations
_ = editor.SetPageRotation(0, 90)
_ = editor.MovePage(2, 0)
_ = editor.DeletePage(5)
_ = editor.CropMargins(36, 36, 36, 36)
_ = editor.EraseRegion(0, 100, 100, 200, 50)

// Annotations and forms
_ = editor.FlattenAnnotations(0)
_ = editor.FlattenAllAnnotations()
_ = editor.FlattenForms()
_ = editor.SetFormFieldValue("name", "Jane")

// Merging
_, _ = editor.MergeFrom("append-me.pdf")

// Save
_ = editor.Save("out.pdf")
_ = editor.SaveEncrypted("secret.pdf", "user", "owner")
```

### Creating PDFs

```go
md, _ := pdfoxide.FromMarkdown("# Hello\n\nBody text.")
defer md.Close()
_ = md.Save("out.pdf")

html, _ := pdfoxide.FromHtml("<h1>Hello</h1><p>Body.</p>")
defer html.Close()

txt, _ := pdfoxide.FromText("Hello, world.")
defer txt.Close()

img, _ := pdfoxide.FromImage("photo.jpg")
defer img.Close()

imgB, _ := pdfoxide.FromImageBytes(rawBytes)
defer imgB.Close()

mergedBytes, _ := pdfoxide.Merge([]string{"a.pdf", "b.pdf"})
```

### Barcodes

```go
qr, _ := pdfoxide.GenerateQRCode("https://example.com", 0, 256)
defer qr.Close()
png := qr.PNGData()

bc, _ := pdfoxide.GenerateBarcode("123456789", 0, 128)
defer bc.Close()
```

### Validation

```go
result, _ := doc.ValidatePdfA(2)       // level 1/2/3
ok, issues, _ := doc.ValidatePdfUa()
okX, issuesX, _ := doc.ValidatePdfX(4)
```

---

## Error Handling

```go
import "errors"

text, err := doc.ExtractText(0)
if err != nil {
    switch {
    case errors.Is(err, pdfoxide.ErrDocumentClosed):
        log.Print("document is closed")
    case errors.Is(err, pdfoxide.ErrInvalidPageIndex):
        log.Print("invalid page index")
    case errors.Is(err, pdfoxide.ErrExtractionFailed):
        log.Print("extraction failed")
    default:
        log.Printf("unexpected: %v", err)
    }
}
```

### Sentinels

```
ErrInvalidPath        ErrDocumentNotFound   ErrInvalidFormat
ErrExtractionFailed   ErrParseError         ErrInvalidPageIndex
ErrSearchFailed       ErrInternal           ErrDocumentClosed
ErrEditorClosed       ErrCreatorClosed      ErrIndexOutOfBounds
ErrEmptyContent
```

The concrete type is `*pdfoxide.Error`, carrying a numeric `Code` and `Message`. Use
`errors.As` if you need those fields:

```go
var e *pdfoxide.Error
if errors.As(err, &e) {
    fmt.Printf("code=%d message=%s\n", e.Code, e.Message)
}
```

---

## Concurrent Operations

Read operations on a `PdfDocument` are protected by an internal `sync.RWMutex`, so multiple
goroutines can safely read the same document in parallel.

```go
import "sync"

var wg sync.WaitGroup
pageCount, _ := doc.PageCount()
results := make(chan string, pageCount)

for i := 0; i < pageCount; i++ {
    wg.Add(1)
    go func(page int) {
        defer wg.Done()
        text, err := doc.ExtractText(page)
        if err != nil {
            return
        }
        results <- text
    }(i)
}

go func() {
    wg.Wait()
    close(results)
}()

for text := range results {
    _ = text
}
```

A `DocumentEditor` serializes writes. Don't share a single `DocumentEditor` across
goroutines performing mutations.

---

## Data Types

### SearchResult

```go
type SearchResult struct {
    Text   string
    Page   int
    X      float32
    Y      float32
    Width  float32
    Height float32
}
```

### Font

```go
type Font struct {
    Name       string
    Type       string
    Encoding   string
    IsEmbedded bool
    IsSubset   bool
    Size       float32
}
```

### Image

```go
type Image struct {
    Width            int
    Height           int
    Format           string
    Colorspace       string
    BitsPerComponent int
    Data             []byte
}
```

### Annotation

```go
type Annotation struct {
    Type             string
    Subtype          string
    Content          string
    X, Y             float32
    Width, Height    float32
    Author           string
    BorderWidth      float32
    Color            uint32
    CreationDate     int64
    ModificationDate int64
    LinkURI          string
    TextIconName     string
    IsHidden         bool
    IsPrintable      bool
    IsReadOnly       bool
    IsMarkedDeleted  bool
}
```

### Element

```go
type Element struct {
    Type   string // "text", etc.
    Text   string
    X, Y   float32
    Width  float32
    Height float32
}
```

### PageInfo / Rect

```go
type Rect struct {
    X, Y, Width, Height float32
}

type PageInfo struct {
    Width    float32
    Height   float32
    Rotation int
    MediaBox Rect
    CropBox  Rect
    ArtBox   Rect
    BleedBox Rect
    TrimBox  Rect
}
```

### Metadata

```go
type Metadata struct {
    Title        string
    Author       string
    Subject      string
    Producer     string
    CreationDate string
}
```

Empty fields are treated as "do not change" by `ApplyMetadata`.

---

## Common Patterns

### Extract All Pages

```go
allText, _ := doc.ExtractAllText()
```

### Walk Pages Manually

```go
pageCount, _ := doc.PageCount()
for page := 0; page < pageCount; page++ {
    text, err := doc.ExtractText(page)
    if err != nil {
        log.Printf("page %d: %v", page, err)
        continue
    }
    fmt.Println(text)
}
```

### Search With Per-Page Counts

```go
pageCount, _ := doc.PageCount()
for page := 0; page < pageCount; page++ {
    hits, err := doc.SearchPage(page, "keyword", false)
    if err != nil {
        continue
    }
    if len(hits) > 0 {
        fmt.Printf("Page %d: %d matches\n", page, len(hits))
    }
}
```

### Batch Processing

```go
for _, file := range pdfFiles {
    doc, err := pdfoxide.Open(file)
    if err != nil {
        log.Print(err)
        continue
    }
    text, _ := doc.ExtractAllText()
    doc.Close()
    _ = text
}
```

---

## Troubleshooting

### "Failed to open document"

```go
doc, err := pdfoxide.Open("/path/to/file.pdf")
if err != nil {
    if errors.Is(err, pdfoxide.ErrDocumentNotFound) {
        log.Fatal("file not found")
    }
    if errors.Is(err, pdfoxide.ErrInvalidFormat) {
        log.Fatal("file is not a valid PDF")
    }
    log.Fatal(err)
}
```

### "Invalid page index"

```go
pageCount, _ := doc.PageCount()
if pageIndex < 0 || pageIndex >= pageCount {
    log.Fatal("invalid page index")
}
```

### "Document already closed"

```go
doc.Close()
// doc.ExtractText(0) // would return an error wrapping ErrDocumentClosed

if !doc.IsClosed() {
    text, _ := doc.ExtractText(0)
    _ = text
}
```

---

## Thread Safety

- `PdfDocument` reads are safe to call concurrently from multiple goroutines.
- `DocumentEditor` serializes writes internally, but don't pipeline independent edits from
  multiple goroutines — collect changes on one goroutine instead.
- `PdfCreator` instances are not intended to be shared across goroutines.

```go
// Safe: concurrent reads
go func() { _, _ = doc.ExtractText(0) }()
go func() { _, _ = doc.ExtractText(1) }()
go func() { _, _ = doc.SearchAll("keyword", false) }()
```

---

## Version

Go Bindings: match the Rust core version.
Go Version: 1.21+ required.
Backends: CGo (default, full API) or purego (`CGO_ENABLED=0`, read-side API).
