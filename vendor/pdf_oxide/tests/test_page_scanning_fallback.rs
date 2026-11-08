//! Tests for page scanning fallback (Issues #54, #57).
//! Verifies that get_page() falls back to scanning on various error types.

use pdf_oxide::document::PdfDocument;
use std::io::Write;

fn write_temp_pdf(data: &[u8], name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("pdf_oxide_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

/// Page tree root is a string instead of a dictionary — triggers InvalidObjectType.
/// The actual Page object (obj 3) is still valid, so scanning should find it.
#[test]
fn test_malformed_page_tree_not_a_dict() {
    let data = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
(this is a string not a pages dict)
endobj

3 0 obj
<< /Type /Page /MediaBox [0 0 612 792] /Contents 4 0 R >>
endobj

4 0 obj
<< /Length 0 >>
stream

endstream
endobj

xref
0 5
0000000000 65535 f \r
0000000009 00000 n \r
0000000058 00000 n \r
0000000106 00000 n \r
0000000186 00000 n \r
trailer
<< /Size 5 /Root 1 0 R >>
startxref
239
%%EOF
";
    let path = write_temp_pdf(data, "page_tree_not_dict.pdf");
    let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
    // Page tree node is invalid, but scanning should find the Page object
    let result = doc.extract_spans(0);
    assert!(result.is_ok(), "Fallback scanning should find the page");
}

/// Page dict missing /Type entry — should be found by heuristic scanning.
#[test]
fn test_page_without_type_entry() {
    let data = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj

3 0 obj
<< /MediaBox [0 0 612 792] /Contents 4 0 R >>
endobj

4 0 obj
<< /Length 0 >>
stream

endstream
endobj

xref
0 5
0000000000 65535 f \r
0000000009 00000 n \r
0000000058 00000 n \r
0000000115 00000 n \r
0000000182 00000 n \r
trailer
<< /Size 5 /Root 1 0 R >>
startxref
235
%%EOF
";
    let path = write_temp_pdf(data, "page_without_type.pdf");
    let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
    // Page object lacks /Type but has /MediaBox — heuristic should find it
    let result = doc.extract_spans(0);
    // The key test is that get_page doesn't error with "Page index not found"
    assert!(
        result.is_ok(),
        "Page without /Type entry should still be found: {:?}",
        result.err()
    );
}
