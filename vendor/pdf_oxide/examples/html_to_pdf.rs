//! Example: render HTML + CSS to a paginated PDF (v0.3.35, issue #248).
//!
//! Walks the full v0.3.35 HTML→PDF pipeline:
//!
//!   HTML + CSS  →  pdf_oxide::api::Pdf::from_html_css(...)  →  PDF
//!
//! Uses the bundled DejaVu Sans test fixture for body text.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example html_to_pdf
//! ```
//!
//! Writes `html_to_pdf_demo.pdf` to the current directory.

use pdf_oxide::api::Pdf;
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let font_path = Path::new("tests/fixtures/fonts/DejaVuSans.ttf");
    if !font_path.exists() {
        eprintln!("Run this example from the repo root so it can find {}.", font_path.display());
        std::process::exit(1);
    }
    let font_bytes = std::fs::read(font_path)?;

    let html = r#"<html>
<body>
    <h1>v0.3.35 — HTML + CSS → PDF</h1>
    <p>This page was rendered by pdf_oxide's new HTML→PDF pipeline.
       Every word you see here flowed through:</p>
    <ul>
        <li>HTML5 tokenizer + arena DOM</li>
        <li>Hand-rolled CSS engine (tokenizer / parser / selectors /
            cascade / calc / var / typed values / at-rules / counters)</li>
        <li>Box tree + Taffy block/flex/grid layout</li>
        <li>Inline formatting with UAX #14 line breaks</li>
        <li>Page fragmentation</li>
        <li>PDF emission with subsetted CIDFontType2 + ToUnicode</li>
    </ul>
    <p>Issue #248 — closed.</p>
</body>
</html>"#;

    let css = "h1 { color: blue } p { color: gray } li { color: black }";

    let mut pdf = Pdf::from_html_css(html, css, font_bytes)?;
    let out_path = "html_to_pdf_demo.pdf";
    pdf.save(out_path)?;
    println!("Wrote {out_path}");

    Ok(())
}
