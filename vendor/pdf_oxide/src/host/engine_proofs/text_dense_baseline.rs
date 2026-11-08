//! Text extraction of a page of one dense baseline takes time linear in its
//! characters.
//!
//! The page is the worst shape for every pass that compares spans with the
//! others on their line: each character is its own run, set 5 points after the
//! last (about a glyph's width, so neighbours overprint a little), all on one
//! baseline.

use super::support::{pdf_from_objects, stream_object, thread_cpu_secs};
use crate::document::PdfDocument;

/// A one-page document of `chars` single-character runs on one baseline.
fn dense_baseline_page(chars: usize) -> Vec<u8> {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut content = String::from("BT /F1 10 Tf 10 99000000 Td\n");
    for _ in 0..chars {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        content.push_str(&format!("({}) Tj 5 0 Td\n", (b'a' + (state % 26) as u8) as char));
    }
    content.push_str("ET\n");
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100000000 100000000] /Contents 4 0 R \
          /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", content.as_bytes()),
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ])
}

/// The CPU seconds `extract_text` and `extract_words` take on a fresh
/// document, on this thread.
fn extraction_time(chars: usize) -> (f64, usize) {
    let doc = PdfDocument::from_bytes(dense_baseline_page(chars)).unwrap();
    let started = thread_cpu_secs();
    let text = doc.extract_text(0).unwrap();
    let words = doc.extract_words(0).unwrap();
    assert!(text.chars().filter(|c| !c.is_whitespace()).count() >= chars * 9 / 10, "the page's text comes back");
    assert!(!words.is_empty());
    (thread_cpu_secs() - started, text.len())
}

/// No Dart battery sees it: a battery asserts a page's text, and a page this
/// large would run for hours in the old passes and still pass; only a
/// comparison of two sizes shows the growth.
#[test]
fn four_times_the_characters_on_a_baseline_take_under_six_times_as_long() {
    let (small_chars, large_chars) = (15_000, 60_000);
    // The quicker of two runs, so a cold cache does not stand for the size.
    let best = |chars| (0..2).map(|_| extraction_time(chars).0).fold(f64::MAX, f64::min);
    let small = best(small_chars);
    let large = best(large_chars);
    // A floor under the small time keeps a clock tick from inflating the ratio.
    let ratio = large / small.max(0.02);
    assert!(
        ratio < 6.0,
        "{small_chars} characters took {small:.3} s and {large_chars} took {large:.3} s ({ratio:.1}x)"
    );
}

#[cfg(not(debug_assertions))]
/// No Dart battery sees it, as above. Compiled for release only: a million
/// characters is seconds there and minutes under the debug profile.
#[test]
fn a_million_characters_on_one_baseline_extract_in_seconds() {
    let (seconds, bytes) = extraction_time(1_000_000);
    eprintln!("1,000,000 characters on one baseline: {seconds:.2} s, {bytes} bytes of text");
    assert!(seconds < 10.0, "took {seconds:.1} s");
}
