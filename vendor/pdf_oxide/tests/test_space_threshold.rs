//! Regression test: Character fragmentation (spurious spaces).
//!
//! Verifies that the space threshold increase from 0.25 to 0.33 reduces
//! spurious space insertion between characters with small positioning gaps.

use pdf_oxide::document::PdfDocument;

#[test]
fn test_no_excessive_fragmentation_in_outline_pdf() {
    let doc = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let page_count = doc.page_count().unwrap();

    for i in 0..page_count {
        let text = doc.extract_text(i).unwrap();
        if text.is_empty() {
            continue;
        }

        // Count ratio of spaces to non-space characters
        let spaces = text.chars().filter(|c| *c == ' ').count();
        let non_spaces = text.chars().filter(|c| !c.is_whitespace()).count();

        if non_spaces > 10 {
            let ratio = spaces as f64 / non_spaces as f64;
            // A well-formatted text should have space ratio < 0.5
            assert!(
                ratio < 0.5,
                "Page {}: Space ratio {:.2} too high (spaces={}, non_spaces={})",
                i,
                ratio,
                spaces,
                non_spaces
            );
        }
    }
}
