//! Example: Edit text content in a PDF
//!
//! This example demonstrates in-place text editing:
//! 1. Creating a PDF with original text
//! 2. Finding and modifying text elements
//! 3. Saving the modified PDF
//!
//! # Usage
//!
//! ```bash
//! cargo run --example edit_text_content
//! ```

use pdf_oxide::api::PdfBuilder;
use pdf_oxide::editor::{DocumentEditor, EditableDocument, SaveOptions};
use pdf_oxide::writer::PageSize;
use pdf_oxide::PdfDocument;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Text Content Editing Example ===\n");

    // Step 1: Create a PDF with "Sample text"
    println!("Step 1: Creating PDF with 'Sample text'...");
    let content = "Sample text";
    let mut pdf = PdfBuilder::new()
        .title("Text Edit Demo")
        .page_size(PageSize::Letter)
        .from_markdown(content)?;
    pdf.save("text_before.pdf")?;
    println!("   Created: text_before.pdf");

    // Verify original content
    let doc = PdfDocument::open("text_before.pdf")?;
    let original_text = doc.extract_text(0)?;
    println!("   Original text: '{}'", original_text.trim());

    // Step 2: Open for editing and modify text in-place
    println!("\nStep 2: Editing - changing 'Sample text' to 'SaMple TeXt'...");
    let mut editor = DocumentEditor::open("text_before.pdf")?;

    // Use edit_page to find and modify text
    editor.edit_page(0, |page| {
        // Find all text elements containing "Sample"
        let text_elements = page.find_text_containing("Sample");
        println!("   Found {} text element(s) containing 'Sample'", text_elements.len());

        for elem in text_elements {
            let current_text = elem.text();
            println!("   Current text: '{}'", current_text);

            // Modify the text - change casing
            let new_text = current_text.replace("Sample text", "SaMple TeXt");
            page.set_text(elem.id(), &new_text)?;
            println!("   Changed to: '{}'", new_text);
        }
        Ok(())
    })?;

    // Step 3: Save the edited PDF
    println!("\nStep 3: Saving edited PDF...");
    editor.save_with_options("text_after.pdf", SaveOptions::full_rewrite())?;
    println!("   Saved: text_after.pdf");

    // Step 4: Verify the change by reading back
    println!("\nStep 4: Verification...");
    let edited_doc = PdfDocument::open("text_after.pdf")?;
    let edited_text = edited_doc.extract_text(0)?;
    println!("   Before: '{}'", original_text.trim());
    println!("   After:  '{}'", edited_text.trim());

    println!("\n=== Done! ===");
    println!("\nOpen both PDFs to compare:");
    println!("  - text_before.pdf (original)");
    println!("  - text_after.pdf  (edited)");

    Ok(())
}
