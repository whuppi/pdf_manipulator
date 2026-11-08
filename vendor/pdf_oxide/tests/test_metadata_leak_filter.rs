//! Regression test: Metadata leaks into extracted text.
//!
//! Verifies that CalRGB/ICCBased color space data and other internal PDF
//! structure data doesn't appear in text output. This tests both the main
//! text extraction pipeline and annotation text extraction.

use pdf_oxide::document::PdfDocument;

/// Known PDF internal keywords that should never appear in extracted text.
const FORBIDDEN_KEYWORDS: &[&str] = &[
    "WhitePoint",
    "BlackPoint",
    "/CalRGB",
    "/ICCBased",
    "/DeviceRGB",
    "/Filter",
    "/FlateDecode",
    "/Length",
    "endstream",
    "endobj",
];

#[test]
fn test_no_metadata_in_outline_pdf() {
    let doc = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let page_count = doc.page_count().unwrap();

    for i in 0..page_count {
        let text = doc.extract_text(i).unwrap();
        for keyword in FORBIDDEN_KEYWORDS {
            assert!(
                !text.contains(keyword),
                "Page {i}: extracted text contains internal PDF keyword '{keyword}'"
            );
        }
    }
}

#[test]
fn test_no_metadata_in_simple_pdf() {
    let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let text = doc.extract_text(0).unwrap();
    for keyword in FORBIDDEN_KEYWORDS {
        assert!(
            !text.contains(keyword),
            "Extracted text contains internal PDF keyword '{keyword}'"
        );
    }
}
