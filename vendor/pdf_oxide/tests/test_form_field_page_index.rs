//! Regression test for form field page index tracking.
//!
//! Form field page indices were hardcoded to 0 instead of being resolved
//! from the widget annotation's /P entry. Fields on page 1+ were
//! incorrectly reported as being on page 0.

use pdf_oxide::editor::{DocumentEditor, FormFieldValue};
use tempfile::tempdir;

/// Create a 2-page PDF with AcroForm fields on different pages.
/// Field "name" is on page 0, field "email" is on page 1.
/// Widget annotations use /P to reference their page.
fn create_pdf_with_form_fields_on_two_pages() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

    let off1 = pdf.len();
    pdf.extend_from_slice(
        b"1 0 obj\n<</Type /Catalog /Pages 2 0 R /AcroForm <</Fields [5 0 R 6 0 R]>>>>\nendobj\n",
    );

    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<</Type /Pages /Kids [3 0 R 4 0 R] /Count 2>>\nendobj\n");

    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<</Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [5 0 R]>>\nendobj\n",
    );

    let off4 = pdf.len();
    pdf.extend_from_slice(
        b"4 0 obj\n<</Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [6 0 R]>>\nendobj\n",
    );

    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<</Type /Annot /Subtype /Widget /FT /Tx /T (name) /V (Alice) /Rect [100 700 300 720] /P 3 0 R>>\nendobj\n",
    );

    let off6 = pdf.len();
    pdf.extend_from_slice(
        b"6 0 obj\n<</Type /Annot /Subtype /Widget /FT /Tx /T (email) /V (alice@example.com) /Rect [100 700 300 720] /P 4 0 R>>\nendobj\n",
    );

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 7\n");
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &[off1, off2, off3, off4, off5, off6] {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }

    pdf.extend_from_slice(b"trailer\n<</Size 7 /Root 1 0 R>>\n");
    pdf.extend_from_slice(format!("startxref\n{}\n%%EOF", xref_offset).as_bytes());

    pdf
}

#[test]
fn test_page_index_resolved_from_widget_annotation() {
    let dir = tempdir().unwrap();
    let pdf_path = dir.path().join("form_pages.pdf");

    std::fs::write(&pdf_path, create_pdf_with_form_fields_on_two_pages()).unwrap();

    let mut editor = DocumentEditor::open(&pdf_path).unwrap();
    let fields = editor.get_form_fields().unwrap();

    assert_eq!(fields.len(), 2, "Should have 2 form fields");

    let name_field = fields.iter().find(|f| f.name() == "name").unwrap();
    let email_field = fields.iter().find(|f| f.name() == "email").unwrap();

    assert_eq!(name_field.page_index(), 0, "'name' field should be on page 0");
    assert_eq!(
        email_field.page_index(),
        1,
        "'email' field should be on page 1 (was hardcoded to 0)"
    );
}

#[test]
fn test_form_field_values_read_correctly() {
    let dir = tempdir().unwrap();
    let pdf_path = dir.path().join("form_values.pdf");

    std::fs::write(&pdf_path, create_pdf_with_form_fields_on_two_pages()).unwrap();

    let mut editor = DocumentEditor::open(&pdf_path).unwrap();
    let fields = editor.get_form_fields().unwrap();

    let name_field = fields.iter().find(|f| f.name() == "name").unwrap();
    let email_field = fields.iter().find(|f| f.name() == "email").unwrap();

    assert_eq!(name_field.value(), FormFieldValue::Text("Alice".to_string()));
    assert_eq!(email_field.value(), FormFieldValue::Text("alice@example.com".to_string()));
}
