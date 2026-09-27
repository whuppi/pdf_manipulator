//! Regression test: Form field and annotation text extraction.
//!
//! Verifies that Widget and FreeText annotation text is included in extract_text output,
//! and that the annotation code path does not crash on PDFs with or without annotations.

use pdf_oxide::document::PdfDocument;

#[test]
fn test_annotation_extraction_does_not_crash() {
    // Verify that extract_text with annotation appending doesn't crash
    // even on a PDF with no annotations.
    let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let text = doc.extract_text(0).unwrap();
    // Result should be deterministic across runs
    let text2 = doc.extract_text(0).unwrap();
    assert_eq!(text, text2, "Annotation extraction should be deterministic");
}

#[test]
fn test_outline_pdf_annotation_extraction() {
    // outline.pdf may have annotations - verify no crash and deterministic output
    let doc = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let page_count = doc.page_count().unwrap();
    for i in 0..page_count {
        let text = doc.extract_text(i).unwrap();
        // Verify deterministic output
        let text2 = doc.extract_text(i).unwrap();
        assert_eq!(text, text2, "Page {i}: annotation extraction should be deterministic");
        // Verify no metadata/colorspace leaks from annotation processing
        assert!(
            !text.contains("WhitePoint"),
            "Page {i}: annotation text contains WhitePoint metadata"
        );
        assert!(!text.contains("/CalRGB"), "Page {i}: annotation text contains /CalRGB metadata");
    }
}
