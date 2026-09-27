# PDF Creation Guide

This guide covers how to create and edit PDF documents using pdf_oxide's creation API.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [High-Level API](#high-level-api)
- [PdfBuilder Options](#pdfbuilder-options)
- [Low-Level API](#low-level-api)
- [Image Embedding](#image-embedding)
- [PDF Editing](#pdf-editing)
- [Advanced Features](#advanced-features)
- [Python Bindings](#python-bindings)

## Overview

pdf_oxide provides multiple APIs for PDF creation:

| API Level | Use Case | Flexibility |
|-----------|----------|-------------|
| `Pdf::from_markdown()` | Quick document creation | Simple |
| `PdfBuilder` | Customized documents | Medium |
| `DocumentBuilder` | Full control over layout | High |
| `PdfWriter` | Low-level PDF structure | Maximum |

## Quick Start

### Creating a PDF from Markdown

```rust
use pdf_oxide::api::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf = Pdf::from_markdown("# Hello World\n\nThis is my PDF.")?;
    pdf.save("output.pdf")?;
    Ok(())
}
```

### Creating a PDF from HTML

```rust
use pdf_oxide::api::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf = Pdf::from_html("<h1>Hello</h1><p>World</p>")?;
    pdf.save("output.pdf")?;
    Ok(())
}
```

### Creating a PDF from Plain Text

```rust
use pdf_oxide::api::Pdf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf = Pdf::from_text("Line 1\nLine 2\nLine 3")?;
    pdf.save("output.pdf")?;
    Ok(())
}
```

## High-Level API

### The `Pdf` Type

The `Pdf` type is the simplest way to create PDFs:

```rust
use pdf_oxide::api::Pdf;

// Create from various sources
let pdf1 = Pdf::from_markdown("# Title")?;
let pdf2 = Pdf::from_html("<h1>Title</h1>")?;
let pdf3 = Pdf::from_text("Plain text")?;

// Get the PDF bytes
let bytes = pdf1.as_bytes();

// Save to file
pdf1.save("document.pdf")?;
```

### Supported Markdown Features

- **Headings**: `# H1`, `## H2`, `### H3`, `#### H4`
- **Paragraphs**: Blank lines separate paragraphs
- **Bold/Italic**: `**bold**`, `*italic*`, `__bold__`, `_italic_`
- **Lists**: `- item` or `* item`
- **Blockquotes**: `> quoted text`
- **Code Blocks**: Triple backticks for code
- **Tables**: GFM-style tables with column alignment

#### GFM Tables

Tables use the GitHub-Flavored Markdown syntax:

```markdown
| Name | Age | City |
|:-----|:---:|-----:|
| Alice | 30 | NYC |
| Bob | 25 | LA |
```

Column alignment is specified in the separator row:
- `:---` or `---` = left-aligned
- `:---:` = center-aligned
- `---:` = right-aligned

## PdfBuilder Options

For more control, use `PdfBuilder`:

```rust
use pdf_oxide::api::PdfBuilder;
use pdf_oxide::writer::PageSize;

let pdf = PdfBuilder::new()
    // Document metadata
    .title("My Document")
    .author("John Doe")
    .subject("Important Report")
    .keywords("rust, pdf, document")

    // Page settings
    .page_size(PageSize::A4)       // A4, Letter, Legal, A3, or Custom
    .margin(72.0)                   // 1 inch margins (all sides)
    .margins(50.0, 50.0, 72.0, 72.0) // left, right, top, bottom

    // Typography
    .font_size(11.0)               // Base font size
    .line_height(1.5)              // Line spacing multiplier

    // Build from content
    .from_markdown("# Content")?;

pdf.save("customized.pdf")?;
```

### Page Sizes

| Size | Dimensions (points) | Dimensions (inches) |
|------|---------------------|---------------------|
| `PageSize::Letter` | 612 × 792 | 8.5 × 11 |
| `PageSize::A4` | 595 × 842 | 8.27 × 11.69 |
| `PageSize::Legal` | 612 × 1008 | 8.5 × 14 |
| `PageSize::A3` | 842 × 1190 | 11.69 × 16.54 |
| `PageSize::Custom(w, h)` | w × h | - |

### Points vs Inches

PDF uses points as the unit of measurement: **72 points = 1 inch**

## Low-Level API

### DocumentBuilder

For full control over page layout:

```rust
use pdf_oxide::writer::{DocumentBuilder, PageSize, DocumentMetadata};

let mut builder = DocumentBuilder::new();

// Set metadata
builder = builder.metadata(
    DocumentMetadata::new()
        .title("My Document")
        .author("Author Name")
);

// Add pages with content
builder.page(PageSize::Letter)
    .at(72.0, 720.0)              // Position cursor
    .heading(1, "Chapter 1")      // Add heading
    .space(20.0)                  // Vertical space
    .paragraph("This is a paragraph with automatic word wrapping.")
    .horizontal_rule()
    .font("Helvetica-Bold", 14.0)
    .text("Bold text")
    .done();

// Build the PDF
let pdf_bytes = builder.build()?;
```

### PdfWriter (Lowest Level)

For maximum control over PDF structure:

```rust
use pdf_oxide::writer::{PdfWriter, PdfWriterConfig};

let mut writer = PdfWriter::with_config(
    PdfWriterConfig::default()
        .with_title("Document")
        .with_compress(true)  // Enable FlateDecode compression
);

let mut page = writer.add_letter_page();
page.add_text("Hello, World!", 72.0, 720.0);
page.finish();

let pdf_bytes = writer.finish()?;
```

## Image Embedding

### Using ImageData

```rust
use pdf_oxide::writer::{ColorSpace, ImageData, ImageManager};

// Create an image from raw pixel data
let width = 100;
let height = 100;
let pixels = vec![255u8; width * height * 3]; // RGB data

let image = ImageData::new(
    width as u32,
    height as u32,
    ColorSpace::DeviceRGB,
    pixels,
);

// Register with image manager
let mut images = ImageManager::new();
let img_id = images.register("my_image", image);

// Use img_id when drawing
```

### Loading Images from Files

```rust
use pdf_oxide::writer::ImageData;

// Load JPEG
let jpeg_data = std::fs::read("photo.jpg")?;
let jpeg_image = ImageData::from_jpeg(jpeg_data)?;

// Load PNG
let png_data = std::fs::read("graphic.png")?;
let png_image = ImageData::from_png(&png_data)?;

// Auto-detect format
let image = ImageData::from_file("image.png")?;
```

### Drawing Images

```rust
use pdf_oxide::writer::ContentStreamBuilder;

let mut content = ContentStreamBuilder::new();

// Draw image at position with size
content.draw_image(
    &img_id,     // Resource ID from ImageManager
    72.0,        // X position (left edge)
    500.0,       // Y position (bottom edge)
    200.0,       // Width
    150.0,       // Height
);
```

### Color Spaces

| Color Space | Components | Use Case |
|-------------|------------|----------|
| `DeviceRGB` | 3 | Photos, graphics |
| `DeviceGray` | 1 | Grayscale images |
| `DeviceCMYK` | 4 | Print documents |

## PDF Editing

### Opening and Modifying PDFs

```rust
use pdf_oxide::api::Pdf;
use pdf_oxide::editor::{DocumentEditor, EditableDocument};

// Open existing PDF
let mut editor = DocumentEditor::open("existing.pdf")?;

// Read metadata
let info = editor.get_info()?;
println!("Current title: {:?}", info.title);

// Modify metadata
editor.set_title("New Title");
editor.set_author("New Author");
editor.set_subject("Updated subject");
editor.set_keywords("new, keywords");

// Page operations
let page_count = editor.page_count()?;
println!("Pages: {}", page_count);

// Get page info
let page_info = editor.get_page_info(0)?;
println!("Page 1: {}x{} points", page_info.width, page_info.height);

// Duplicate a page
let new_idx = editor.duplicate_page(0)?;

// Move a page
editor.move_page(new_idx, 1)?;

// Remove a page
editor.remove_page(2)?;

// Save
editor.save("modified.pdf")?;
```

### Save Options

```rust
use pdf_oxide::editor::SaveOptions;

// Full rewrite (recommended for most cases)
editor.save_with_options("output.pdf", SaveOptions::full_rewrite())?;

// Incremental update (preserves signatures)
editor.save_with_options("output.pdf", SaveOptions::incremental())?;

// Custom options
let options = SaveOptions {
    incremental: false,
    compress: true,
    linearize: false,
    garbage_collect: true,
};
editor.save_with_options("output.pdf", options)?;
```

## Advanced Features

### Bookmarks/Outlines

```rust
use pdf_oxide::writer::{OutlineBuilder, OutlineItem, OutlineDestination, FitMode};

let mut outline = OutlineBuilder::new();

outline.add(OutlineItem::new("Chapter 1")
    .destination(OutlineDestination::page(0, FitMode::Fit))
    .children(vec![
        OutlineItem::new("Section 1.1")
            .destination(OutlineDestination::page(0, FitMode::FitH(500.0))),
        OutlineItem::new("Section 1.2")
            .destination(OutlineDestination::page(1, FitMode::Fit)),
    ]));
```

### Tables

```rust
use pdf_oxide::writer::{Table, TableRow, TableCell, TableStyle, ColumnWidth};

let table = Table::new()
    .style(TableStyle::default())
    .columns(vec![
        ColumnWidth::Fixed(100.0),
        ColumnWidth::Auto,
        ColumnWidth::Percent(30.0),
    ])
    .header(TableRow::new(vec![
        TableCell::text("Name"),
        TableCell::text("Description"),
        TableCell::text("Price"),
    ]))
    .row(TableRow::new(vec![
        TableCell::text("Item 1"),
        TableCell::text("First item"),
        TableCell::text("$10.00"),
    ]));
```

### Gradients and Patterns

```rust
use pdf_oxide::writer::{LinearGradientBuilder, GradientStop};

let gradient = LinearGradientBuilder::new()
    .start(0.0, 0.0)
    .end(100.0, 100.0)
    .add_stop(GradientStop::new(0.0, [1.0, 0.0, 0.0]))  // Red
    .add_stop(GradientStop::new(1.0, [0.0, 0.0, 1.0])); // Blue
```

### Extended Graphics State

```rust
use pdf_oxide::writer::{ExtGStateBuilder, BlendMode};

let state = ExtGStateBuilder::new()
    .fill_alpha(0.5)
    .stroke_alpha(0.8)
    .blend_mode(BlendMode::Multiply);
```

## Python Bindings

pdf_oxide provides Python bindings for PDF creation:

```python
from pdf_oxide import Pdf, PdfBuilder, PageSize

# Simple creation
pdf = Pdf.from_markdown("# Hello World")
pdf.save("output.pdf")

# With options
pdf = (PdfBuilder()
    .title("My Document")
    .author("Author")
    .page_size(PageSize.A4)
    .from_markdown("# Content"))

# Save
pdf.save("document.pdf")

# Get bytes
data = pdf.to_bytes()
```

## Best Practices

1. **Use the simplest API that meets your needs**
   - Start with `Pdf::from_markdown()` for simple documents
   - Move to `PdfBuilder` for customization
   - Use `DocumentBuilder` or `PdfWriter` only when necessary

2. **Enable compression for smaller files**
   ```rust
   PdfWriterConfig::default().with_compress(true)
   ```

3. **Set metadata for searchability**
   ```rust
   .title("Document Title")
   .author("Author Name")
   .keywords("relevant, keywords")
   ```

4. **Use appropriate page sizes**
   - `Letter` for US documents
   - `A4` for international documents

5. **Handle errors properly**
   ```rust
   let pdf = Pdf::from_markdown(content)?; // Use ? for error propagation
   ```

## Examples

See the `examples/` directory for complete working examples:

- `examples/create_pdf_from_markdown.rs` - Markdown to PDF
- `examples/create_pdf_with_images.rs` - Embedding images
- `examples/edit_existing_pdf.rs` - Modifying PDFs

Run examples with:
```bash
cargo run --example create_pdf_from_markdown
cargo run --example create_pdf_with_images
cargo run --example edit_existing_pdf
```
