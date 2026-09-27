# PDF Oxide — Rust Examples

```bash
# Run any example (from the repo root):
cargo run --example extract_text -- document.pdf
```

| Example | Description |
|---------|-------------|
| [01-extract-text](01-extract-text/main.rs) | Open PDF, print page count, extract text per page |
| [02-convert-formats](02-convert-formats/main.rs) | Convert pages to Markdown, HTML, plain text |
| [03-create-pdf](03-create-pdf/main.rs) | Create PDFs from Markdown, HTML, and text |
| [04-search-text](04-search-text/main.rs) | Full-text search across all pages |
| [05-extract-structured](05-extract-structured/main.rs) | Words with bounding boxes, text lines, tables |
| [06-edit-document](06-edit-document/main.rs) | Modify metadata, delete pages, merge PDFs |
| [07-forms-annotations](07-forms-annotations/main.rs) | Extract form fields and annotations |
| [08-batch-processing](08-batch-processing/main.rs) | Concurrent PDF processing with rayon |
