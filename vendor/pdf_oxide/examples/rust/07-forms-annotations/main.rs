// Extract annotations from a PDF page.
// (Form fields are accessible via DocumentEditor — see the editor examples.)
// Run: cargo run --example tutorial_forms_annotations -- tests/fixtures/simple.pdf

use pdf_oxide::PdfDocument;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .expect("Usage: tutorial_forms_annotations <file.pdf>");
    let doc = PdfDocument::open(&path)?;
    println!("Opened: {}", path);

    let pages = doc.page_count()?;
    for page in 0..pages {
        let annotations = doc.get_annotations(page)?;
        if !annotations.is_empty() {
            println!("\n--- Annotations (page {}) ---", page + 1);
            for a in &annotations {
                println!(
                    "  Type: {:<14}  Contents: \"{}\"",
                    format!("{:?}", a.annotation_type),
                    a.contents.as_deref().unwrap_or("")
                );
            }
        }
    }
    println!("Done.");
    Ok(())
}
