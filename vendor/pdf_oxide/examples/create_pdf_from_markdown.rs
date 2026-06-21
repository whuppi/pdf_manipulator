//! Example: Create a PDF from Markdown content
//!
//! This example demonstrates how to use the high-level API to create
//! PDFs from Markdown content with customizable options.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example create_pdf_from_markdown
//! ```

use pdf_oxide::api::{Pdf, PdfBuilder};
use pdf_oxide::writer::PageSize;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Example 1: Simple one-liner
    println!("Creating simple PDF from Markdown...");

    let mut simple_pdf = Pdf::from_markdown("# Hello, World!\n\nThis is a simple PDF.")?;
    simple_pdf.save("simple_markdown.pdf")?;
    println!("  Saved: simple_markdown.pdf");

    // Example 2: Using PdfBuilder for more control
    println!("\nCreating customized PDF with PdfBuilder...");

    let markdown_content = r#"
# PDF Creation with pdf_oxide

Welcome to **pdf_oxide** - a pure Rust PDF library!

## Features

This library supports:

- Creating PDFs from Markdown
- Creating PDFs from HTML
- Creating PDFs from plain text
- Editing existing PDFs
- Full Unicode support

## Code Example

Here's how easy it is - just a few lines of code!

## Lists

### Unordered List
- First item
- Second item
- Third item

### Blockquote

> This is a blockquote.
> It can span multiple lines.

## Conclusion

PDF creation has never been easier!
"#;

    let mut custom_pdf = PdfBuilder::new()
        .title("PDF Creation Guide")
        .author("pdf_oxide Team")
        .subject("Documentation")
        .keywords("pdf, rust, markdown, creation")
        .page_size(PageSize::A4)
        .margin(50.0) // 50pt margins all around
        .font_size(11.0)
        .line_height(1.4)
        .from_markdown(markdown_content)?;

    custom_pdf.save("custom_markdown.pdf")?;
    println!("  Saved: custom_markdown.pdf");

    // Example 3: Multiple pages with different margins
    println!("\nCreating PDF with custom margins...");

    let long_content = r#"
# Chapter 1: Introduction

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod
tempor incididunt ut labore et dolore magna aliqua.

## Section 1.1

Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
ut aliquip ex ea commodo consequat.

## Section 1.2

Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
dolore eu fugiat nulla pariatur.

# Chapter 2: Getting Started

Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia
deserunt mollit anim id est laborum.

## Section 2.1

Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium
doloremque laudantium.

### Subsection 2.1.1

Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit.

### Subsection 2.1.2

Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet.

# Chapter 3: Advanced Topics

At vero eos et accusamus et iusto odio dignissimos ducimus qui blanditiis.

## Conclusion

Thank you for reading!
"#;

    let mut margins_pdf = PdfBuilder::new()
        .title("Sample Document")
        .author("Author Name")
        .page_size(PageSize::Letter)
        .margins(72.0, 72.0, 90.0, 72.0) // Extra top margin for headers
        .font_size(12.0)
        .from_markdown(long_content)?;

    margins_pdf.save("margins_example.pdf")?;
    println!("  Saved: margins_example.pdf");

    println!("\nAll PDFs created successfully!");
    println!("\nGenerated files:");
    println!("  - simple_markdown.pdf  (basic example)");
    println!("  - custom_markdown.pdf  (with metadata and styling)");
    println!("  - margins_example.pdf  (with custom margins)");

    Ok(())
}
