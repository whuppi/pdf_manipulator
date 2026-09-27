//! Diagnostic tool to analyze font encoding issues
//!
//! This tool extracts detailed font information from a PDF to help diagnose
//! character encoding problems like the EU GDPR scrambling issue.

use pdf_oxide::{PdfDocument, Result};
use std::env;

fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <pdf_file> [page_number]", args[0]);
        eprintln!("Example: {} EU_GDPR_Regulation.pdf 0", args[0]);
        std::process::exit(1);
    }

    let pdf_path = &args[1];
    let page_num = if args.len() > 2 {
        args[2].parse().unwrap_or(0)
    } else {
        0
    };

    println!("=".repeat(80));
    println!("PDF FONT ENCODING DIAGNOSTIC TOOL");
    println!("=".repeat(80));
    println!("File: {}", pdf_path);
    println!("Page: {}", page_num);
    println!();

    let mut doc = PdfDocument::open(pdf_path)?;

    // Extract spans to see what we're getting
    let spans = doc.extract_spans(page_num)?;

    println!("EXTRACTED SPANS: {} total", spans.len());
    println!("-".repeat(80));

    // Show first 10 spans with details
    for (i, span) in spans.iter().take(10).enumerate() {
        println!("Span #{}", i);
        println!("  Text: {:?}", span.text);
        println!("  Font: {}", span.font_name);
        println!("  Font Size: {:.2}", span.font_size);
        println!("  Position: ({:.2}, {:.2})", span.bbox.x, span.bbox.y);

        // Show character codes (first 20 chars)
        print!("  Char codes: ");
        for ch in span.text.chars().take(20) {
            print!("U+{:04X} ", ch as u32);
        }
        println!();

        // Show as hex bytes
        print!("  UTF-8 bytes: ");
        for byte in span.text.as_bytes().iter().take(40) {
            print!("{:02X} ", byte);
        }
        println!();
        println!();
    }

    println!("=".repeat(80));
    println!("FIRST 500 CHARACTERS OF EXTRACTED TEXT:");
    println!("-".repeat(80));

    let mut char_count = 0;
    for span in &spans {
        for ch in span.text.chars() {
            print!("{}", ch);
            char_count += 1;
            if char_count >= 500 {
                break;
            }
        }
        if char_count >= 500 {
            break;
        }
    }
    println!();
    println!();

    Ok(())
}
