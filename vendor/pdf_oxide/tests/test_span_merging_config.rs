//! Integration tests for the `merge_tm_tj_runs` opt-out flag on `SpanMergingConfig` (#488).
//!
//! These tests open a real PDF from `tests/fixtures/` and verify that:
//! - `merge_tm_tj_runs: true` (default) returns a span count at most as large as
//!   `merge_tm_tj_runs: false`.
//! - Disabling the flag never loses text content.

use pdf_oxide::document::PdfDocument;
use pdf_oxide::extractors::SpanMergingConfig;

/// Open `tests/fixtures/1.pdf` (a multi-page PDF with real text runs) and
/// verify that disabling `merge_tm_tj_runs` yields at least as many spans as
/// the default (merging-on) configuration.
///
/// The invariant: merging collapses consecutive same-line Tm+Tj runs into
/// fewer spans, so the disabled count must be >= the enabled count.
#[test]
fn test_merge_tm_tj_disabled_gives_more_spans() {
    let fixture = "tests/fixtures/1.pdf";
    let doc = match PdfDocument::open(fixture) {
        Ok(d) => d,
        Err(e) => panic!("Failed to open {fixture}: {e}"),
    };

    let page_count = doc.page_count().unwrap_or(0);
    assert!(page_count > 0, "Fixture {fixture} must have at least one page");

    let config_default = SpanMergingConfig::default(); // merge_tm_tj_runs: true
    let config_no_merge = SpanMergingConfig {
        merge_tm_tj_runs: false,
        ..SpanMergingConfig::default()
    };

    let mut found_text_page = false;

    for page in 0..page_count {
        let spans_merged = doc
            .extract_spans_with_config(page, config_default.clone())
            .expect("extract_spans_with_config (merged) failed");
        let spans_split = doc
            .extract_spans_with_config(page, config_no_merge.clone())
            .expect("extract_spans_with_config (split) failed");

        if spans_merged.is_empty() {
            // Skip image-only or blank pages.
            continue;
        }
        found_text_page = true;

        // Text content must be identical regardless of merge flag.
        let text_merged: String = spans_merged.iter().map(|s| s.text.as_str()).collect();
        let text_split: String = spans_split.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(
            text_merged, text_split,
            "Page {page}: text content must not change when merge_tm_tj_runs is toggled"
        );

        // With merging disabled every Tm starts a fresh span, so the count
        // must be >= the merged count (merging can only reduce the number of spans).
        assert!(
            spans_split.len() >= spans_merged.len(),
            "Page {page}: expected spans_split ({}) >= spans_merged ({}) \
             but merge_tm_tj_runs=false produced fewer spans — this is a regression",
            spans_split.len(),
            spans_merged.len()
        );
    }

    assert!(
        found_text_page,
        "Fixture {fixture} had no pages with extractable text — pick a different fixture"
    );
}
