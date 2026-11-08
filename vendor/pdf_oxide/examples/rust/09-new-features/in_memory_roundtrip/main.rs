// In-memory round-trip: build() → bytes → PdfDocument::from_bytes()
//
// Demonstrates building a PDF entirely in memory (never touching the
// filesystem mid-way) and re-opening it from raw bytes for text extraction.
//
// Run: cargo run --example showcase_in_memory_roundtrip

use pdf_oxide::{error::Result, writer::DocumentBuilder, PdfDocument};
use std::path::PathBuf;

fn main() -> Result<()> {
    let out_dir = PathBuf::from("target/examples_output/in_memory_roundtrip");
    std::fs::create_dir_all(&out_dir)?;

    // Build PDF entirely in memory.
    let mut builder = DocumentBuilder::new();
    builder
        .letter_page()
        .font("Helvetica", 12.0)
        .at(72.0, 720.0)
        .heading(1, "In-Memory Round-Trip")
        .at(72.0, 690.0)
        .paragraph("This PDF was built in memory, never written to disk mid-way.")
        .done();

    let pdf_bytes = builder.build()?;

    // Re-open from bytes — no filesystem path involved.
    let doc = PdfDocument::from_bytes(pdf_bytes.clone())?;
    let text = doc.extract_all_text()?;
    println!("Extracted {} chars from in-memory PDF", text.len());
    assert!(text.contains("In-Memory"), "round-trip text missing");

    let out = out_dir.join("in_memory_roundtrip.pdf");
    std::fs::write(&out, &pdf_bytes)?;
    println!("Written: {}", out.display());
    Ok(())
}
