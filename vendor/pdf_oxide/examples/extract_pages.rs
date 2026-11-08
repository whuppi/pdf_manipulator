//! Batch page text extractor for regression testing.
//!
//! Usage:
//!   cargo run --release --example extract_pages \
//!       -- <input.pdf> <output_dir> [max_pages]
//!
//! Writes page_001.md, page_001.html, page_001.txt to <output_dir>.
//! Writes a SKIP sentinel for encrypted / corrupt PDFs.

use pdf_oxide::converters::ConversionOptions;
use pdf_oxide::document::PdfDocument;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: extract_pages <input.pdf> <output_dir> [max_pages]");
        std::process::exit(1);
    }
    let input = &args[1];
    let out_dir = std::path::Path::new(&args[2]);
    let max_pages: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(2);

    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!("ERROR: cannot create {}: {}", out_dir.display(), e);
        std::process::exit(1);
    }

    let doc = match PdfDocument::open(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("SKIP {}: {}", input, e);
            let _ = std::fs::write(out_dir.join("SKIP"), format!("{}", e));
            std::process::exit(0);
        },
    };

    let page_count = match doc.page_count() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("SKIP {} (page_count): {}", input, e);
            let _ = std::fs::write(out_dir.join("SKIP"), format!("{}", e));
            std::process::exit(0);
        },
    };

    let opts = ConversionOptions {
        include_images: false,
        ..ConversionOptions::default()
    };
    let to_extract = page_count.min(max_pages);

    for page_idx in 0..to_extract {
        let label = format!("page_{:03}", page_idx + 1);

        match doc.extract_text(page_idx) {
            Ok(txt) => {
                let _ = std::fs::write(out_dir.join(format!("{}.txt", label)), txt);
            },
            Err(e) => eprintln!("WARN txt p{} {}: {}", page_idx + 1, input, e),
        }

        match doc.to_markdown(page_idx, &opts) {
            Ok(md) => {
                let _ = std::fs::write(out_dir.join(format!("{}.md", label)), md);
            },
            Err(e) => eprintln!("WARN md  p{} {}: {}", page_idx + 1, input, e),
        }

        match doc.to_html(page_idx, &opts) {
            Ok(html) => {
                let _ = std::fs::write(out_dir.join(format!("{}.html", label)), html);
            },
            Err(e) => eprintln!("WARN html p{} {}: {}", page_idx + 1, input, e),
        }

        eprintln!("OK  {}/{}", label, input);
    }
}
