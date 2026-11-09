//! Simple markdown extraction tool for benchmarking

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

    // Parse optional page range
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
        let md = doc.to_markdown(page_idx, &options)?;
        println!("{}", md);
    }

    Ok(())
}
