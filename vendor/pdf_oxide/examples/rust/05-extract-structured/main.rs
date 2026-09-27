// Extract words with bounding boxes and tables from a PDF page.
// Run: cargo run --example tutorial_extract_structured -- tests/fixtures/simple.pdf

use pdf_oxide::PdfDocument;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .expect("Usage: tutorial_extract_structured <file.pdf>");
    let doc = PdfDocument::open(&path)?;
    println!("Opened: {}", path);

    let page = 0;

    // Extract words with position data
    let words = doc.extract_words(page)?;
    println!("\n--- Words (page {}) ---", page + 1);
    for w in words.iter().take(20) {
        println!(
            "{:20} x={:<7.1} y={:<7.1} w={:<7.1} h={:<7.1}",
            format!("\"{}\"", w.text),
            w.bbox.x,
            w.bbox.y,
            w.bbox.width,
            w.bbox.height,
        );
    }
    if words.len() > 20 {
        println!("... ({} more words)", words.len() - 20);
    }

    // Extract tables
    let tables = doc.extract_tables(page)?;
    println!("\n--- Tables (page {}) ---", page + 1);
    if tables.is_empty() {
        println!("(no tables found)");
    }
    for (i, table) in tables.iter().enumerate() {
        println!("Table {}: {} rows", i + 1, table.rows.len());
        for (ri, row) in table.rows.iter().take(5).enumerate() {
            let cols: Vec<_> = row
                .cells
                .iter()
                .take(6)
                .map(|c| format!("\"{}\"", c.text))
                .collect();
            println!("  Row {}: {}", ri, cols.join("  "));
        }
    }

    Ok(())
}
