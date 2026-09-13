//! Cross-language benchmark baseline — Rust native.
//!
//! Measures four operations per fixture (open, extract first page, extract
//! all pages, search) and emits a single JSON line per fixture so results can
//! be diffed against the Go / C# / JS/TS bindings.
//!
//! Run with:
//!   cargo run --release --bin rust_bench -- bench_fixtures/tiny.pdf bench_fixtures/small.pdf ...

use std::env;
use std::path::Path;
use std::time::Instant;

use pdf_oxide::search::{SearchOptions, TextSearcher};
use pdf_oxide::PdfDocument;

const ITERATIONS: u32 = 5;

#[derive(Debug)]
struct FixtureResult {
    language: &'static str,
    fixture: String,
    size_bytes: u64,
    open_ns: u128,
    extract_page0_ns: u128,
    extract_all_ns: u128,
    search_ns: u128,
    page_count: usize,
    text_len: usize,
}

fn bench_fixture(path: &Path) -> Result<FixtureResult, Box<dyn std::error::Error>> {
    let size_bytes = std::fs::metadata(path)?.len();
    let fixture = path.file_name().unwrap().to_string_lossy().into_owned();

    // Warm-up pass (not measured) — exercises every code path we're about to
    // measure so per-call JIT / lazy init is amortized away.
    let warm_opts = SearchOptions::default();
    {
        let doc = PdfDocument::open(path)?;
        let _ = doc.extract_text(0)?;
        let _ = TextSearcher::search(&doc, "the", &warm_opts)?;
    }

    // Open (average across ITERATIONS).
    let mut open_total: u128 = 0;
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let _ = PdfDocument::open(path)?;
        open_total += start.elapsed().as_nanos();
    }
    let open_ns = open_total / ITERATIONS as u128;

    // Extract text from page 0 (average across ITERATIONS on a single open doc).
    let doc = PdfDocument::open(path)?;
    let page_count = doc.page_count()?;
    let mut p0_total: u128 = 0;
    let mut text_len = 0;
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let text = doc.extract_text(0)?;
        p0_total += start.elapsed().as_nanos();
        text_len = text.len();
    }
    let extract_page0_ns = p0_total / ITERATIONS as u128;

    // Extract text from all pages (single run — expensive for large docs).
    let start = Instant::now();
    for i in 0..page_count {
        let _ = doc.extract_text(i)?;
    }
    let extract_all_ns = start.elapsed().as_nanos();

    // Search all pages for a common word (single run).
    let search_opts = SearchOptions {
        case_insensitive: false,
        ..Default::default()
    };
    let start = Instant::now();
    let _ = TextSearcher::search(&doc, "the", &search_opts)?;
    let search_ns = start.elapsed().as_nanos();

    Ok(FixtureResult {
        language: "rust",
        fixture,
        size_bytes,
        open_ns,
        extract_page0_ns,
        extract_all_ns,
        search_ns,
        page_count,
        text_len,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: rust_bench <fixture.pdf>...");
        std::process::exit(1);
    }

    for path_str in &args {
        let path = Path::new(path_str);
        match bench_fixture(path) {
            Ok(r) => {
                // Emit one JSON object per line (NDJSON) — easy to aggregate.
                println!(
                    r#"{{"language":"{}","fixture":"{}","sizeBytes":{},"openNs":{},"extractPage0Ns":{},"extractAllNs":{},"searchNs":{},"pageCount":{},"textLen":{}}}"#,
                    r.language,
                    r.fixture,
                    r.size_bytes,
                    r.open_ns,
                    r.extract_page0_ns,
                    r.extract_all_ns,
                    r.search_ns,
                    r.page_count,
                    r.text_len
                );
            },
            Err(e) => {
                eprintln!("rust_bench failed for {}: {}", path_str, e);
                std::process::exit(2);
            },
        }
    }
    Ok(())
}
