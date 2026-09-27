# PDF Oxide — Go Examples

```bash
go get github.com/yfedoseev/pdf_oxide/go
go run examples/go/01-extract-text/main.go document.pdf
```

| Example | Description |
|---------|-------------|
| [01-extract-text](01-extract-text/main.go) | Open PDF, print page count, extract text per page |
| [02-convert-formats](02-convert-formats/main.go) | Convert pages to Markdown, HTML, plain text |
| [03-create-pdf](03-create-pdf/main.go) | Create PDFs from Markdown, HTML, and text |
| [04-search-text](04-search-text/main.go) | Full-text search across all pages |
| [05-extract-structured](05-extract-structured/main.go) | Words with bounding boxes, text lines, tables |
| [06-edit-document](06-edit-document/main.go) | Modify metadata, delete pages, merge PDFs |
| [07-forms-annotations](07-forms-annotations/main.go) | Extract form fields and annotations |
| [08-batch-processing](08-batch-processing/main.go) | Concurrent PDF processing with goroutines |
