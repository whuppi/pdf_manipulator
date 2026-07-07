// Process multiple PDFs and summarise page/word counts.
// With the `parallel` feature, processing uses rayon for concurrency.
// Run: cargo run --example tutorial_batch_processing -- tests/fixtures/simple.pdf

use pdf_oxide::PdfDocument;
use std::{env, time::Instant};

fn process(path: &str) -> String {
    let doc = match PdfDocument::open(path) {
        Ok(d) => d,
        Err(e) => return format!("[{}] ERROR: {}", path, e),
    };
    let pages = doc.page_count().unwrap_or(0);
    let mut total_words = 0usize;
    for p in 0..pages {
        if let Ok(words) = doc.extract_words(p) {
            total_words += words.len();
        }
    }
    format!("[{}]  pages={}  words={}", path, pages, total_words)
}

fn main() {
    let paths: Vec<String> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("Usage: tutorial_batch_processing <file1.pdf> [file2.pdf ...]");
        std::process::exit(1);
    }

    println!("Processing {} PDFs...", paths.len());
    let start = Instant::now();

    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        let results: Vec<_> = paths.par_iter().map(|p| process(p)).collect();
        for r in &results {
            println!("{}", r);
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        for path in &paths {
            println!("{}", process(path));
        }
    }

    println!("\nDone in {:.2}s", start.elapsed().as_secs_f64());
}
