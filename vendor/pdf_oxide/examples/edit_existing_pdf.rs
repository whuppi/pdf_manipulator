//! Example: Edit an existing PDF document
//!
//! This example demonstrates how to use the DocumentEditor API to:
//! - Open an existing PDF
//! - Read and modify metadata
//! - Reorder pages
//! - Save the modified document
//!
//! # Usage
//!
//! ```bash
//! cargo run --example edit_existing_pdf
//! ```

use pdf_oxide::api::PdfBuilder;
use pdf_oxide::editor::{DocumentEditor, EditableDocument, SaveOptions};
use pdf_oxide::writer::PageSize;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("PDF Editing Example\n");
    println!("===================\n");

    // First, create a sample PDF to edit
    println!("Step 1: Creating a sample PDF to edit...");
    create_sample_pdf()?;
    println!("   Created: sample_document.pdf\n");

    // Open the PDF for editing
    println!("Step 2: Opening PDF for editing...");
    let mut editor = DocumentEditor::open("sample_document.pdf")?;

    // Display current document info
    println!("\n   Current Document Info:");
    display_info(&mut editor)?;

    // Read current metadata
    println!("\nStep 3: Reading current metadata...");
    let info = editor.get_info()?;
    println!("   Title:    {:?}", info.title);
    println!("   Author:   {:?}", info.author);
    println!("   Subject:  {:?}", info.subject);
    println!("   Keywords: {:?}", info.keywords);
    println!("   Creator:  {:?}", info.creator);
    println!("   Producer: {:?}", info.producer);

    // Modify metadata
    println!("\nStep 4: Modifying metadata...");
    editor.set_title("Edited Document Title");
    editor.set_author("PDF Editor");
    editor.set_subject("Demonstrates PDF editing capabilities");
    editor.set_keywords("pdf, editing, rust, pdf_oxide");
    println!("   Modified title, author, subject, and keywords");

    // Check page count
    println!("\nStep 5: Checking page information...");
    let page_count = editor.page_count()?;
    println!("   Document has {} page(s)", page_count);

    for i in 0..page_count {
        let page_info = editor.get_page_info(i)?;
        println!(
            "   Page {}: {}x{} points (rotation: {}°)",
            i + 1,
            page_info.width,
            page_info.height,
            page_info.rotation
        );
    }

    // Demonstrate page operations (if document has multiple pages)
    if page_count > 1 {
        println!("\nStep 6: Page manipulation...");
        println!("   Document has multiple pages - demonstrating page operations");

        // Duplicate first page
        let new_page_idx = editor.duplicate_page(0)?;
        println!("   Duplicated page 1 -> new page at index {}", new_page_idx);

        // Move page
        editor.move_page(new_page_idx, 1)?;
        println!("   Moved duplicated page to position 2");

        println!("   Current page count: {}", editor.current_page_count());
    }

    // Check if document was modified
    println!("\nStep 7: Checking modification status...");
    println!("   Document modified: {}", editor.is_modified());

    // Save with options
    println!("\nStep 8: Saving modified PDF...");

    // Option A: Full rewrite (recommended for most cases)
    let save_opts = SaveOptions::full_rewrite();
    editor.save_with_options("edited_document.pdf", save_opts)?;
    println!("   Saved as: edited_document.pdf (full rewrite)");

    // Verify the changes by opening the edited file
    println!("\nStep 9: Verifying changes...");
    let mut verify = DocumentEditor::open("edited_document.pdf")?;
    let new_info = verify.get_info()?;
    println!("   New Title:    {:?}", new_info.title);
    println!("   New Author:   {:?}", new_info.author);
    println!("   New Subject:  {:?}", new_info.subject);
    println!("   New Keywords: {:?}", new_info.keywords);
    println!("   Page Count:   {}", verify.page_count()?);

    // Demonstrate different save options
    demonstrate_save_options()?;

    println!("\n===================");
    println!("PDF editing complete!\n");
    println!("Generated files:");
    println!("  - sample_document.pdf  (original document)");
    println!("  - edited_document.pdf  (modified document)");

    Ok(())
}

/// Create a sample PDF document for editing.
fn create_sample_pdf() -> Result<(), Box<dyn Error>> {
    let content = r#"
# Sample Document

This is a sample PDF created for demonstrating editing capabilities.

## Chapter 1: Introduction

Welcome to the PDF editing demonstration. This document will be modified
using the pdf_oxide library's editing features.

## Chapter 2: Content

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod
tempor incididunt ut labore et dolore magna aliqua.

### Section 2.1

Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.

### Section 2.2

Duis aute irure dolor in reprehenderit in voluptate velit esse cillum.

## Chapter 3: Conclusion

This document demonstrates the PDF creation and editing capabilities
of the pdf_oxide library.
"#;

    let mut pdf = PdfBuilder::new()
        .title("Sample Document")
        .author("Original Author")
        .subject("Sample Content")
        .keywords("sample, test, pdf")
        .page_size(PageSize::Letter)
        .from_markdown(content)?;

    pdf.save("sample_document.pdf")?;
    Ok(())
}

/// Display document information.
fn display_info(editor: &mut DocumentEditor) -> Result<(), Box<dyn Error>> {
    let version = editor.version();
    println!("   PDF Version: {}.{}", version.0, version.1);
    println!("   Source: {}", editor.source_path());
    println!("   Page Count: {}", editor.page_count()?);
    println!("   Modified: {}", editor.is_modified());
    Ok(())
}

/// Demonstrate different save options.
fn demonstrate_save_options() -> Result<(), Box<dyn Error>> {
    println!("\nSave Options Reference:");
    println!("------------------------");

    let full_rewrite = SaveOptions::full_rewrite();
    println!(
        "\nSaveOptions::full_rewrite():\n  \
        - incremental: {}\n  \
        - compress: {}\n  \
        - linearize: {}\n  \
        - garbage_collect: {}",
        full_rewrite.incremental,
        full_rewrite.compress,
        full_rewrite.linearize,
        full_rewrite.garbage_collect
    );

    let incremental = SaveOptions::incremental();
    println!(
        "\nSaveOptions::incremental():\n  \
        - incremental: {}\n  \
        - compress: {}\n  \
        - linearize: {}\n  \
        - garbage_collect: {}",
        incremental.incremental,
        incremental.compress,
        incremental.linearize,
        incremental.garbage_collect
    );

    println!("\nRecommendation: Use full_rewrite() for most cases.");
    println!("Use incremental() only when appending to signed documents.");

    Ok(())
}
