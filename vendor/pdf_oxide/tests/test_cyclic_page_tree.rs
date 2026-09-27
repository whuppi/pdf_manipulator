//! Test for cyclic page tree detection (Issue #51).
//! Verifies that circular Kids references produce an error instead of stack overflow.

use pdf_oxide::document::PdfDocument;
use std::io::Write;

/// Build a PDF where the Pages node's Kids array references itself.
fn build_pdf_cyclic_page_tree() -> Vec<u8> {
    // Object 2 (Pages) has Kids = [2 0 R], creating a self-referencing cycle.
    b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

2 0 obj
<< /Type /Pages /Kids [2 0 R] /Count 1 >>
endobj

xref
0 3
0000000000 65535 f \r
0000000009 00000 n \r
0000000058 00000 n \r
trailer
<< /Size 3 /Root 1 0 R >>
startxref
120
%%EOF
"
    .to_vec()
}

fn write_temp_pdf(data: &[u8], name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("pdf_oxide_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

#[test]
fn test_cyclic_page_tree_no_stack_overflow() {
    let data = build_pdf_cyclic_page_tree();
    let path = write_temp_pdf(&data, "cyclic_page_tree.pdf");
    let doc = PdfDocument::open(&path).expect("Should parse PDF structure");
    // Attempting to get page 0 should return an error, not stack overflow.
    let result = doc.get_page_content_data(0);
    assert!(result.is_err(), "Expected error for cyclic page tree");
}
