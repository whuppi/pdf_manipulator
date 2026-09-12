//! Simple text extraction tool

use pdf_oxide::document::PdfDocument;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <pdf_file>", args[0]);
        std::process::exit(1);
    }

    let pdf_path = &args[1];
    let doc = PdfDocument::open(pdf_path)?;
    let page_count = doc.page_count()?;

    for page_idx in 0..page_count {
        let text = doc.extract_text(page_idx)?;
        println!("{}", text);
    }

    Ok(())
}
