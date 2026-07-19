//! Tests for blank pages — missing /Contents, null content references.
//! Covers Issue #48 (missing Contents) and Issue #53 (null stream references).

use pdf_oxide::document::PdfDocument;
use std::io::Write;

/// Build a minimal PDF with a page that has no /Contents entry.
fn build_pdf_no_contents() -> Vec<u8> {
    b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj

3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>
endobj

xref
0 4
0000000000 65535 f \r
0000000009 00000 n \r
0000000058 00000 n \r
0000000115 00000 n \r
trailer
<< /Size 4 /Root 1 0 R >>
startxref
193
%%EOF
"
    .to_vec()
}

/// Write bytes to a temp file and return the path.
fn write_temp_pdf(data: &[u8], name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("pdf_oxide_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

#[test]
fn test_page_without_contents_returns_empty() {
    let data = build_pdf_no_contents();
    let path = write_temp_pdf(&data, "no_contents.pdf");
    let doc = PdfDocument::open(&path).expect("Should parse minimal PDF");
    let content = doc
        .get_page_content_data(0)
        .expect("Should not error on missing Contents");
    assert!(content.is_empty(), "Expected empty content for page without /Contents");
}

#[test]
fn test_extract_text_blank_page() {
    let data = build_pdf_no_contents();
    let path = write_temp_pdf(&data, "no_contents_text.pdf");
    let doc = PdfDocument::open(&path).expect("Should parse minimal PDF");
    let text = doc.extract_text(0).expect("Should not error on blank page");
    assert!(text.trim().is_empty(), "Expected empty text for blank page");
}
