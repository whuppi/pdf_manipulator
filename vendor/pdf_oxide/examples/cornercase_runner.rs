//! Corner-case fixture runner for v0.3.37 HTML+CSS→PDF.
//!
//! Iterates /tmp/html_corpus_v2/*.html, renders each via
//! `Pdf::from_html_css`, re-extracts text + spans via `PdfDocument`,
//! and reports per-fixture counts to stdout. Used by the cron-fired
//! validation pass; pass `--write-pdfs` to also drop PDFs alongside.

use pdf_oxide::api::Pdf;
use pdf_oxide::PdfDocument;
use std::fs;

const DEJAVU: &[u8] = include_bytes!("../tests/fixtures/fonts/DejaVuSans.ttf");

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/html_corpus_v2".to_string());
    let write_pdfs = std::env::args().any(|a| a == "--write-pdfs");
    let out_dir = format!("{dir}/out");
    if write_pdfs {
        let _ = fs::create_dir_all(&out_dir);
    }

    let mut entries: Vec<_> = fs::read_dir(&dir)
        .expect("read corpus dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("html"))
        .collect();
    entries.sort_by_key(|e| e.path());

    println!("# corner-case run on {dir}");
    println!("name\tstatus\tbytes\tpages\tspans\textract_chars\tnotes");

    for e in entries {
        let path = e.path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let html = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) => {
                println!("{name}\tREAD_ERR\t0\t0\t0\t0\t{err}");
                continue;
            },
        };
        let result = std::panic::catch_unwind(|| Pdf::from_html_css(&html, "", DEJAVU.to_vec()));
        let pdf = match result {
            Ok(Ok(p)) => p,
            Ok(Err(err)) => {
                println!("{name}\tBUILD_ERR\t0\t0\t0\t0\t{err}");
                continue;
            },
            Err(_) => {
                println!("{name}\tPANIC\t0\t0\t0\t0\trender panicked");
                continue;
            },
        };
        let bytes = pdf.into_bytes();
        let nbytes = bytes.len();
        if write_pdfs {
            let _ = fs::write(format!("{out_dir}/{name}.pdf"), &bytes);
        }
        let doc = match PdfDocument::from_bytes(bytes) {
            Ok(d) => d,
            Err(err) => {
                println!("{name}\tREOPEN_ERR\t{nbytes}\t0\t0\t0\t{err}");
                continue;
            },
        };
        let pages = doc.page_count().unwrap_or(0);
        let mut spans = 0usize;
        let mut chars = 0usize;
        let mut notes = String::new();
        for i in 0..pages {
            match doc.extract_text(i) {
                Ok(t) => chars += t.chars().count(),
                Err(e) => notes.push_str(&format!("text_p{i}_err:{e};")),
            }
            match doc.extract_spans(i) {
                Ok(s) => spans += s.len(),
                Err(e) => notes.push_str(&format!("spans_p{i}_err:{e};")),
            }
        }
        let status = if pages == 0 {
            "EMPTY"
        } else if chars == 0 && spans == 0 {
            "NO_TEXT"
        } else {
            "OK"
        };
        if notes.is_empty() {
            notes.push('-');
        }
        println!("{name}\t{status}\t{nbytes}\t{pages}\t{spans}\t{chars}\t{notes}");
    }
}
