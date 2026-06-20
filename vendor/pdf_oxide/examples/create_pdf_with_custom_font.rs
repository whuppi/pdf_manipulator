//! Example: Create a PDF with an embedded custom TrueType font.
//!
//! Demonstrates the v0.3.35 writer-side font subsystem end-to-end:
//!
//!   1. Load a TTF from disk into [`EmbeddedFont`].
//!   2. Register it with [`PdfWriter::register_embedded_font`] —
//!      gets back an `"EFn"` resource name.
//!   3. Add Unicode text on a page via
//!      [`PageBuilder::add_embedded_text`] — encodes glyphs as
//!      Identity-H hex through the font's cmap, emits Tj.
//!   4. `finish()` writes the five PDF objects required for embedding
//!      (Type 0, CIDFontType2, FontDescriptor, FontFile2 stream,
//!      ToUnicode CMap stream — ISO 32000-1 §9.6.4 / §9.7.4 / §9.8 /
//!      §9.9 / §9.10.2).
//!
//! Round-trip: re-open the PDF with `PdfDocument`, extract text from
//! page 0, and assert each input string survives.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example create_pdf_with_custom_font
//! ```
//!
//! Writes `custom_font_demo.pdf` to the current directory.

use pdf_oxide::writer::{EmbeddedFont, PdfWriter};
use pdf_oxide::PdfDocument;
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    // The crate ships a permissive-licence DejaVu Sans for tests
    // (tests/fixtures/fonts/DejaVuSans.ttf). Use it from the repo
    // checkout when running this example.
    let font_path = Path::new("tests/fixtures/fonts/DejaVuSans.ttf");
    if !font_path.exists() {
        eprintln!(
            "Run this example from the repository root so the example \
             can find {}.",
            font_path.display()
        );
        std::process::exit(1);
    }

    let font_bytes = std::fs::read(font_path)?;
    println!("Loaded {} bytes of DejaVuSans.ttf", font_bytes.len());

    // ── Build a PDF with three lines of multi-script text ────────────
    let mut writer = PdfWriter::new();
    let font = EmbeddedFont::from_data(Some("DejaVuSans".to_string()), font_bytes)
        .map_err(|e| format!("font parse: {e}"))?;
    let resource_name = writer.register_embedded_font(font);

    let lines = [
        "Hello, World!",  // Latin
        "café déjà vu",   // Latin Extended
        "Привет, мир!",   // Cyrillic
        "Καλημέρα κόσμε", // Greek
        "שלום עולם",      // Hebrew (visual order — no BiDi yet)
    ];

    {
        let mut page = writer.add_letter_page();
        let mut y = 720.0_f32;
        for line in &lines {
            page.add_embedded_text(line, 72.0, y, &resource_name, 14.0);
            y -= 30.0;
        }
    }

    let pdf_bytes = writer.finish()?;
    let out_path = "custom_font_demo.pdf";
    std::fs::write(out_path, &pdf_bytes)?;
    println!("Wrote {} ({} bytes)", out_path, pdf_bytes.len());

    // ── Round-trip via extract_text ─────────────────────────────────
    let doc = PdfDocument::from_bytes(pdf_bytes)?;
    let extracted = doc.extract_text(0)?;
    println!("\nExtracted text from page 0:\n{extracted}");

    let mut all_present = true;
    for line in &lines {
        if extracted.contains(line) {
            println!("  ✓ round-tripped: {line}");
        } else {
            println!("  ✗ MISSING:       {line}");
            all_present = false;
        }
    }

    if !all_present {
        return Err("round-trip failed: at least one input line was not recovered \
             from extract_text — embedded-font ToUnicode CMap is wrong"
            .into());
    }

    Ok(())
}
