// Extract text from every page of a PDF and print it.
// Run: cargo run --example tutorial_extract_text -- tests/fixtures/simple.pdf

use pdf_oxide::PdfDocument;
use std::{env, process};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: tutorial_extract_text <file.pdf>");
        process::exit(1);
    });

    let doc = PdfDocument::open(&path).unwrap_or_else(|e| {
        eprintln!("Failed to open {}: {}", path, e);
        process::exit(1);
    });

    let pages = doc.page_count().unwrap_or(0);
    println!("Opened: {}", path);
    println!("Pages: {}\n", pages);

    for i in 0..pages {
        let text = doc.extract_text(i).unwrap_or_default();
        println!("--- Page {} ---", i + 1);
        println!("{}\n", text);
    }
}
