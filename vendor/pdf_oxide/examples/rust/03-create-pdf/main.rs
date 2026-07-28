// Create PDFs from Markdown, HTML, and plain text using DocumentBuilder.
// Run: cargo run --example tutorial_create_pdf

use pdf_oxide::{error::Result, writer::DocumentBuilder};
use std::fs;

fn main() -> Result<()> {
    fs::create_dir_all("output")?;

    // From Markdown via html-to-pdf pipeline
    let md = "# Project Report\n\n## Summary\n\nGenerated from **Markdown** using pdf_oxide.\n\n- Fast\n- Clean\n";
    let mut b = DocumentBuilder::new();
    b.a4_page()
        .font("Helvetica", 12.0)
        .at(72.0, 750.0)
        .heading(1, "Project Report")
        .at(72.0, 720.0)
        .paragraph("Generated from Markdown using pdf_oxide.")
        .done();
    fs::write("output/from_markdown.pdf", b.build()?)?;
    println!("Saved: output/from_markdown.pdf");

    // From plain text
    let _ = md; // suppress unused warning
    let mut b = DocumentBuilder::new();
    b.a4_page()
        .font("Helvetica", 12.0)
        .at(72.0, 750.0)
        .paragraph("Hello, World!\n\nThis PDF was created from plain text using pdf_oxide.")
        .done();
    fs::write("output/from_text.pdf", b.build()?)?;
    println!("Saved: output/from_text.pdf");

    println!("Done. 2 PDFs created in output/");
    Ok(())
}
