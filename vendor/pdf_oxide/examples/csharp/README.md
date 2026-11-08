# PDF Oxide — C# / .NET Examples

```bash
dotnet add package PdfOxide
dotnet run --project examples/csharp/01-extract-text/ -- document.pdf
```

| Example | Description |
|---------|-------------|
| [01-extract-text](01-extract-text/Program.cs) | Open PDF, print page count, extract text per page |
| [02-convert-formats](02-convert-formats/Program.cs) | Convert pages to Markdown, HTML, plain text |
| [03-create-pdf](03-create-pdf/Program.cs) | Create PDFs from Markdown, HTML, and text |
| [04-search-text](04-search-text/Program.cs) | Full-text search across all pages |
| [05-extract-structured](05-extract-structured/Program.cs) | Words with bounding boxes, text lines, tables |
| [06-edit-document](06-edit-document/Program.cs) | Modify metadata, delete pages, merge PDFs |
| [07-forms-annotations](07-forms-annotations/Program.cs) | Extract form fields and annotations |
| [08-batch-processing](08-batch-processing/Program.cs) | Concurrent PDF processing with Task.WhenAll |
