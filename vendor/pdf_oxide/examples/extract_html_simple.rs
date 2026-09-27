//! Simple HTML extraction tool for benchmarking.
//!
//! Mirrors `extract_markdown_simple.rs` so the regression harness can
//! compare pdf_oxide's HTML extraction path against baseline and across
//! releases. Pairs with #326 / #327 quality audit — the HTML pipeline
//! is a separate algorithm from the plain-text pipeline and needs its
//! own regression coverage.
//!
//! Usage: extract_html_simple <pdf_file> [page_range]
//!   page_range: single page (e.g. 0) or range (e.g. 0-5)
//! Environment variables:
//!   NO_IMAGES=1  — skip image embedding in the HTML output

use pdf_oxide::converters::ConversionOptions;
use pdf_oxide::document::PdfDocument;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <pdf_file> [page_range]", args[0]);
        eprintln!("  page_range: single page (e.g. 0) or range (e.g. 0-5)");
        std::process::exit(1);
    }

    let pdf_path = &args[1];
    let doc = PdfDocument::open(pdf_path)?;
    let page_count = doc.page_count()?;
    let no_images = env::var("NO_IMAGES").is_ok();
    let options = ConversionOptions {
        include_images: !no_images,
        ..ConversionOptions::default()
    };

    let (start, end) = if let Some(range_arg) = args.get(2) {
        if let Some(dash_pos) = range_arg.find('-') {
            let s = range_arg[..dash_pos].parse::<usize>().unwrap_or(0);
            let e = range_arg[dash_pos + 1..]
                .parse::<usize>()
                .unwrap_or(page_count)
                .min(page_count);
            (s, e)
        } else {
            let p = range_arg.parse::<usize>().unwrap_or(0);
            (p, (p + 1).min(page_count))
        }
    } else {
        (0, page_count)
    };

    for page_idx in start..end {
        let html = doc.to_html(page_idx, &options)?;
        println!("{}", html);
    }

    Ok(())
}
