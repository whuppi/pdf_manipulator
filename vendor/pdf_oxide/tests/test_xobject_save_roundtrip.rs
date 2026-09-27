//! Regression tests for XObject resource preservation during save.
//!
//! Images (XObject resources) were stripped from output when opening and
//! re-saving PDFs via DocumentEditor. The editor's write_full_to_writer
//! only serialized Font resources, skipping XObject and ExtGState entries.

use pdf_oxide::document::PdfDocument;
use pdf_oxide::editor::{DocumentEditor, EditableDocument, SaveOptions};
use tempfile::tempdir;

/// Create a minimal valid PDF containing an image XObject.
fn create_pdf_with_image_xobject() -> Vec<u8> {
    let image_data: &[u8] = &[
        0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00,
    ];
    let content_stream: &[u8] = b"q 100 0 0 100 100 600 cm /Im0 Do Q";

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<</Type /Catalog /Pages 2 0 R>>\nendobj\n");

    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<</Type /Pages /Kids [3 0 R] /Count 1>>\nendobj\n");

    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<</Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources <</XObject <</Im0 6 0 R>> /Font <</F1 7 0 R>>>>>>\nendobj\n",
    );

    let off4 = pdf.len();
    let cs_header = format!("4 0 obj\n<</Length {}>>\nstream\n", content_stream.len());
    pdf.extend_from_slice(cs_header.as_bytes());
    pdf.extend_from_slice(content_stream);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let off6 = pdf.len();
    let img_header = format!(
        "6 0 obj\n<</Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {}>>\nstream\n",
        image_data.len()
    );
    pdf.extend_from_slice(img_header.as_bytes());
    pdf.extend_from_slice(image_data);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let off7 = pdf.len();
    pdf.extend_from_slice(
        b"7 0 obj\n<</Type /Font /Subtype /Type1 /BaseFont /Helvetica>>\nendobj\n",
    );

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 8\n");
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &[off1, off2, off3, off4] {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    // Object 5 is unused — mark as free
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &[off6, off7] {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }

    pdf.extend_from_slice(b"trailer\n<</Size 8 /Root 1 0 R>>\n");
    pdf.extend_from_slice(format!("startxref\n{}\n%%EOF", xref_offset).as_bytes());

    pdf
}

#[test]
fn test_xobject_preserved_after_full_rewrite() {
    let dir = tempdir().unwrap();
    let original_path = dir.path().join("with_xobj.pdf");
    let saved_path = dir.path().join("resaved.pdf");

    let pdf_bytes = create_pdf_with_image_xobject();
    let original_content = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        original_content.contains("/Im0") && original_content.contains("/XObject"),
        "Original PDF should contain XObject resource"
    );

    std::fs::write(&original_path, &pdf_bytes).unwrap();

    let mut editor = DocumentEditor::open(&original_path).unwrap();
    editor
        .save_with_options(&saved_path, SaveOptions::full_rewrite())
        .unwrap();

    let saved_bytes = std::fs::read(&saved_path).unwrap();
    let saved_content = String::from_utf8_lossy(&saved_bytes);
    assert!(
        saved_content.contains("/Subtype /Image") || saved_content.contains("/Subtype/Image"),
        "Saved PDF should contain the Image XObject (regression: XObject save)"
    );
}

#[test]
fn test_xobject_preserved_after_metadata_edit() {
    let dir = tempdir().unwrap();
    let original_path = dir.path().join("xobj_meta.pdf");
    let saved_path = dir.path().join("xobj_meta_saved.pdf");

    std::fs::write(&original_path, create_pdf_with_image_xobject()).unwrap();

    let mut editor = DocumentEditor::open(&original_path).unwrap();
    editor.set_title("Modified Title");
    editor
        .save_with_options(&saved_path, SaveOptions::full_rewrite())
        .unwrap();

    let saved_bytes = std::fs::read(&saved_path).unwrap();
    let saved_content = String::from_utf8_lossy(&saved_bytes);
    assert!(
        saved_content.contains("/Subtype /Image") || saved_content.contains("/Subtype/Image"),
        "XObject should survive metadata edit (regression: XObject save)"
    );
}

#[test]
fn test_saved_pdf_with_xobject_is_valid() {
    let dir = tempdir().unwrap();
    let original_path = dir.path().join("valid_xobj.pdf");
    let saved_path = dir.path().join("valid_xobj_saved.pdf");

    std::fs::write(&original_path, create_pdf_with_image_xobject()).unwrap();

    let mut editor = DocumentEditor::open(&original_path).unwrap();
    editor
        .save_with_options(&saved_path, SaveOptions::full_rewrite())
        .unwrap();

    assert!(
        PdfDocument::from_bytes(std::fs::read(&saved_path).unwrap()).is_ok(),
        "Saved PDF with XObjects should be parseable"
    );
    assert!(DocumentEditor::open(&saved_path).is_ok(), "Saved PDF should be re-openable");
}
