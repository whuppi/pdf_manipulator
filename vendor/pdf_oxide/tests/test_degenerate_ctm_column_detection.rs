//! Regression test for degenerate CTM in column detection.
//!
//! Pages with rotated or degenerate coordinate transforms can produce
//! text spans with astronomically large bounding boxes (e.g., 19 quadrillion
//! points wide). detect_page_columns() allocated a histogram proportional
//! to page_width / 2.0, causing a multi-petabyte allocation and SIGABRT.

use pdf_oxide::document::PdfDocument;

/// Verify that extracting text from a page with degenerate CTM coordinates
/// does not crash with an out-of-memory allocation error.
///
/// The test PDF has Page rot: 90 and dvips-generated content that produces
/// enormous CTM-transformed coordinates on page 72. Before the fix,
/// this caused a 38 petabyte allocation in detect_page_columns().
#[test]
fn test_extract_text_degenerate_ctm_no_oom() {
    // Create a synthetic PDF with a page that has a degenerate CTM
    // producing extremely wide span coordinates.
    let pdf_bytes = create_pdf_with_degenerate_ctm();
    let doc = PdfDocument::from_bytes(pdf_bytes).unwrap();

    // This must not panic or SIGABRT — should gracefully fall back
    let result = doc.extract_text(0);
    assert!(result.is_ok(), "extract_text should not crash on degenerate CTM");
}

/// Create a minimal PDF where the content stream has a CTM that scales
/// coordinates to enormous values, simulating the degenerate transform
/// seen in rotated dvips PDFs.
fn create_pdf_with_degenerate_ctm() -> Vec<u8> {
    // Content stream with a 1e9 cm scale factor and 100pt text offset,
    // expanding coordinates to ~1e11 in user space.
    let content: &[u8] = b"q 1000000000 0 0 1000000000 0 0 cm BT /F1 12 Tf 100 100 Td (X) Tj ET Q";
    let cs_header = format!("<</Length {}>>", content.len());

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<</Type /Catalog /Pages 2 0 R>>\nendobj\n");

    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<</Type /Pages /Kids [3 0 R] /Count 1>>\nendobj\n");

    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<</Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources <</Font <</F1 5 0 R>>>>>>\nendobj\n",
    );

    let off4 = pdf.len();
    pdf.extend_from_slice(format!("4 0 obj\n{}\nstream\n", cs_header).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>\nendobj\n",
    );

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n");
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &[off1, off2, off3, off4, off5] {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }

    pdf.extend_from_slice(b"trailer\n<</Size 6 /Root 1 0 R>>\n");
    pdf.extend_from_slice(format!("startxref\n{}\n%%EOF", xref_offset).as_bytes());

    pdf
}
