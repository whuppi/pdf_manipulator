//! Tests that `extract_text` does not panic when span positions contain NaN
//! values that would violate total-order constraints in sort comparators.
//!
//! `extract_text` on page 15 (0-indexed: 14) of `1008.3918v2.pdf`
//! panicked with "user-provided comparison function does not correctly
//! implement a total order" from the stdlib's smallsort. Some sort
//! comparator in the reading-order / span-merging pipeline returned a
//! result that is not a total order (typically `partial_cmp` on NaN
//! positions collapsed via `unwrap_or(Equal)`).
//!
//! This test opens the attached PDF and extracts page 15. The
//! pre-fix behaviour is a thread panic from `sort` / `sort_unstable`;
//! with the fix the call completes without panicking.

use pdf_oxide::PdfDocument;

const FIXTURE: &str = "tests/fixtures/1008.3918v2.pdf";

#[test]
fn extract_text_does_not_panic_on_nan_span_positions() {
    let doc = PdfDocument::open(FIXTURE).expect("open fixture");
    let page_count = doc.page_count().expect("page_count");
    assert!(page_count >= 15, "fixture should have at least 15 pages, got {}", page_count);

    // Page 15 (1-indexed in the bug report) → 0-indexed 14.
    // The extraction must not panic. We don't assert on specific
    // content — the bug is the panic itself.
    let result = doc.extract_text(14);
    assert!(result.is_ok(), "extract_text(14) returned an error: {:?}", result.err());
}
