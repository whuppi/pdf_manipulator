//! Integration tests for FDF and XFDF export functionality.
//!
//! Tests the export of form field data to FDF (Forms Data Format) and
//! XFDF (XML Forms Data Format) per ISO 32000-1:2008 Section 12.7.7.

use pdf_oxide::api::Pdf;
use pdf_oxide::editor::{DocumentEditor, EditableDocument};
use pdf_oxide::fdf::{FdfField, FdfValue, FdfWriter, XfdfWriter};
use pdf_oxide::geometry::Rect;
use pdf_oxide::writer::form_fields::{
    CheckboxWidget, ChoiceOption, ComboBoxWidget, TextFieldWidget,
};
use tempfile::tempdir;

// ============================================================================
// FdfWriter Unit Tests
// ============================================================================

#[test]
fn test_fdf_writer_basic() {
    let mut writer = FdfWriter::new();
    writer.add_field(FdfField::new("name", FdfValue::Text("John Doe".into())));
    writer.add_field(FdfField::new("email", FdfValue::Text("john@example.com".into())));

    let bytes = writer.to_bytes().unwrap();
    let content = String::from_utf8_lossy(&bytes);

    // Verify FDF structure
    assert!(content.contains("%FDF-1.2"));
    assert!(content.contains("/Fields"));
    assert!(content.contains("/T (name)"));
    assert!(content.contains("/V (John Doe)"));
    assert!(content.contains("/T (email)"));
    assert!(content.contains("/V (john@example.com)"));
    assert!(content.contains("%%EOF"));
}

#[test]
fn test_fdf_writer_with_file_spec() {
    let writer = FdfWriter::new().with_file_spec("original.pdf");
    let bytes = writer.to_bytes().unwrap();
    let content = String::from_utf8_lossy(&bytes);

    assert!(content.contains("/F (original.pdf)"));
}

#[test]
fn test_fdf_writer_boolean_values() {
    let mut writer = FdfWriter::new();
    writer.add_field(FdfField::new("agree", FdfValue::Boolean(true)));
    writer.add_field(FdfField::new("decline", FdfValue::Boolean(false)));

    let bytes = writer.to_bytes().unwrap();
    let content = String::from_utf8_lossy(&bytes);

    assert!(content.contains("/V /Yes"));
    assert!(content.contains("/V /Off"));
}

#[test]
fn test_fdf_writer_name_values() {
    let mut writer = FdfWriter::new();
    writer.add_field(FdfField::new("choice", FdfValue::Name("Option1".into())));

    let bytes = writer.to_bytes().unwrap();
    let content = String::from_utf8_lossy(&bytes);

    assert!(content.contains("/V /Option1"));
}

#[test]
fn test_fdf_writer_array_values() {
    let mut writer = FdfWriter::new();
    writer.add_field(FdfField::new(
        "multi",
        FdfValue::Array(vec!["Choice A".into(), "Choice B".into()]),
    ));

    let bytes = writer.to_bytes().unwrap();
    let content = String::from_utf8_lossy(&bytes);

    assert!(content.contains("/V [ (Choice A) (Choice B) ]"));
}

#[test]
fn test_fdf_writer_special_chars() {
    let mut writer = FdfWriter::new();
    writer.add_field(FdfField::new("note", FdfValue::Text("Hello (World)".into())));

    let bytes = writer.to_bytes().unwrap();
    let content = String::from_utf8_lossy(&bytes);

    // Parentheses should be escaped
    assert!(content.contains("/V (Hello \\(World\\))"));
}

#[test]
fn test_fdf_write_to_file() {
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("test.fdf");

    let mut writer = FdfWriter::new();
    writer.add_field(FdfField::new("test", FdfValue::Text("value".into())));
    writer.write_to_file(&output_path).unwrap();

    // Verify file was created
    assert!(output_path.exists());

    // Verify content
    let content = String::from_utf8_lossy(&std::fs::read(&output_path).unwrap()).to_string();
    assert!(content.contains("%FDF-1.2"));
    assert!(content.contains("/T (test)"));
}

// ============================================================================
// XfdfWriter Unit Tests
// ============================================================================

#[test]
fn test_xfdf_writer_basic() {
    let mut writer = XfdfWriter::new();
    writer.add_field("name", "John Doe");
    writer.add_field("email", "john@example.com");

    let xml = writer.to_xml();

    assert!(xml.contains("<?xml version=\"1.0\""));
    assert!(xml.contains("<xfdf xmlns=\"http://ns.adobe.com/xfdf/\""));
    assert!(xml.contains("<fields>"));
    assert!(xml.contains("<field name=\"name\">"));
    assert!(xml.contains("<value>John Doe</value>"));
    assert!(xml.contains("<field name=\"email\">"));
    assert!(xml.contains("<value>john@example.com</value>"));
    assert!(xml.contains("</xfdf>"));
}

#[test]
fn test_xfdf_writer_with_file_spec() {
    let writer = XfdfWriter::new().with_file_spec("original.pdf");
    let xml = writer.to_xml();

    assert!(xml.contains("<f href=\"original.pdf\"/>"));
}

#[test]
fn test_xfdf_writer_xml_escaping() {
    let mut writer = XfdfWriter::new();
    writer.add_field("company", "Smith & Jones <Consulting>");

    let xml = writer.to_xml();

    assert!(xml.contains("<value>Smith &amp; Jones &lt;Consulting&gt;</value>"));
}

#[test]
fn test_xfdf_writer_boolean_values() {
    let mut writer = XfdfWriter::new();
    writer.add_fdf_field(FdfField::new("agree", FdfValue::Boolean(true)));
    writer.add_fdf_field(FdfField::new("decline", FdfValue::Boolean(false)));

    let xml = writer.to_xml();

    assert!(xml.contains("<field name=\"agree\">"));
    assert!(xml.contains("<value>Yes</value>"));
    assert!(xml.contains("<field name=\"decline\">"));
    assert!(xml.contains("<value>Off</value>"));
}

#[test]
fn test_xfdf_writer_hierarchical() {
    let mut writer = XfdfWriter::new();
    let parent = FdfField::new("address", FdfValue::None)
        .with_kid(FdfField::new("street", FdfValue::Text("123 Main St".into())))
        .with_kid(FdfField::new("city", FdfValue::Text("Anytown".into())));
    writer.add_fdf_field(parent);

    let xml = writer.to_xml();

    assert!(xml.contains("<field name=\"address\">"));
    assert!(xml.contains("<field name=\"street\">"));
    assert!(xml.contains("<value>123 Main St</value>"));
    assert!(xml.contains("<field name=\"city\">"));
    assert!(xml.contains("<value>Anytown</value>"));
}

#[test]
fn test_xfdf_write_to_file() {
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("test.xfdf");

    let mut writer = XfdfWriter::new();
    writer.add_field("test", "value");
    writer.write_to_file(&output_path).unwrap();

    // Verify file was created
    assert!(output_path.exists());

    // Verify content
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("<?xml version=\"1.0\""));
    assert!(content.contains("<field name=\"test\">"));
}

// ============================================================================
// DocumentEditor Integration Tests
// ============================================================================

#[test]
fn test_export_fdf_from_editor() {
    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("form.pdf");
    let fdf_path = temp_dir.path().join("export.fdf");

    // Create a PDF with form fields
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("username", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("test_user"),
        )
        .unwrap();
    editor
        .add_form_field(
            0,
            CheckboxWidget::new("agree", Rect::new(100.0, 660.0, 20.0, 20.0)).checked(),
        )
        .unwrap();
    editor.save(&pdf_path).unwrap();

    // Open and export FDF
    let mut editor = DocumentEditor::open(&pdf_path).unwrap();
    editor.export_form_data_fdf(&fdf_path).unwrap();

    // Verify FDF content
    let content = String::from_utf8_lossy(&std::fs::read(&fdf_path).unwrap()).to_string();
    assert!(content.contains("%FDF-1.2"));
    assert!(content.contains("/Fields"));
}

#[test]
fn test_export_xfdf_from_editor() {
    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("form.pdf");
    let xfdf_path = temp_dir.path().join("export.xfdf");

    // Create a PDF with form fields
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("email", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("test@example.com"),
        )
        .unwrap();
    editor.save(&pdf_path).unwrap();

    // Open and export XFDF
    let mut editor = DocumentEditor::open(&pdf_path).unwrap();
    editor.export_form_data_xfdf(&xfdf_path).unwrap();

    // Verify XFDF content
    let content = std::fs::read_to_string(&xfdf_path).unwrap();
    assert!(content.contains("<?xml version=\"1.0\""));
    assert!(content.contains("<xfdf"));
    assert!(content.contains("<fields>"));
}

#[test]
fn test_export_from_pdf_without_forms() {
    let temp_dir = tempdir().unwrap();
    let fdf_path = temp_dir.path().join("empty.fdf");
    let xfdf_path = temp_dir.path().join("empty.xfdf");

    // Open a PDF without forms
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Export should succeed but produce empty fields
    editor.export_form_data_fdf(&fdf_path).unwrap();
    editor.export_form_data_xfdf(&xfdf_path).unwrap();

    // Verify files exist
    assert!(fdf_path.exists());
    assert!(xfdf_path.exists());

    // FDF should have empty Fields array
    let fdf_content = String::from_utf8_lossy(&std::fs::read(&fdf_path).unwrap()).to_string();
    assert!(fdf_content.contains("%FDF-1.2"));
    assert!(fdf_content.contains("/Fields ["));

    // XFDF should have empty fields element
    let xfdf_content = std::fs::read_to_string(&xfdf_path).unwrap();
    assert!(xfdf_content.contains("<fields>"));
    assert!(xfdf_content.contains("</fields>"));
}

// ============================================================================
// High-Level Pdf API Tests
// ============================================================================

#[test]
fn test_pdf_api_export_fdf() {
    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("form.pdf");
    let fdf_path = temp_dir.path().join("export.fdf");

    // Create a PDF with form fields
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("name", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("Test Name"),
        )
        .unwrap();
    editor.save(&pdf_path).unwrap();

    // Open and export via Pdf API
    let mut pdf = Pdf::open(&pdf_path).unwrap();
    pdf.export_form_data_fdf(&fdf_path).unwrap();

    assert!(fdf_path.exists());
}

#[test]
fn test_pdf_api_export_xfdf() {
    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("form.pdf");
    let xfdf_path = temp_dir.path().join("export.xfdf");

    // Create a PDF with form fields
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();
    editor
        .add_form_field(
            0,
            ComboBoxWidget::new("country", Rect::new(100.0, 700.0, 150.0, 20.0))
                .with_choice_options(vec![
                    ChoiceOption::new("USA"),
                    ChoiceOption::new("Canada"),
                    ChoiceOption::new("UK"),
                ])
                .with_value("USA"),
        )
        .unwrap();
    editor.save(&pdf_path).unwrap();

    // Open and export via Pdf API
    let mut pdf = Pdf::open(&pdf_path).unwrap();
    pdf.export_form_data_xfdf(&xfdf_path).unwrap();

    assert!(xfdf_path.exists());
}

// ============================================================================
// Round-trip Tests
// ============================================================================

#[test]
fn test_fdf_round_trip_consistency() {
    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("form.pdf");
    let fdf_path1 = temp_dir.path().join("export1.fdf");
    let fdf_path2 = temp_dir.path().join("export2.fdf");

    // Create a PDF with multiple field types
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("text_field", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("Hello World"),
        )
        .unwrap();
    editor
        .add_form_field(
            0,
            CheckboxWidget::new("checkbox", Rect::new(100.0, 660.0, 20.0, 20.0)).checked(),
        )
        .unwrap();
    editor.save(&pdf_path).unwrap();

    // Export twice and compare
    let mut editor1 = DocumentEditor::open(&pdf_path).unwrap();
    editor1.export_form_data_fdf(&fdf_path1).unwrap();

    let mut editor2 = DocumentEditor::open(&pdf_path).unwrap();
    editor2.export_form_data_fdf(&fdf_path2).unwrap();

    // Contents should be identical
    let content1 = String::from_utf8_lossy(&std::fs::read(&fdf_path1).unwrap()).to_string();
    let content2 = String::from_utf8_lossy(&std::fs::read(&fdf_path2).unwrap()).to_string();
    assert_eq!(content1, content2);
}

#[test]
fn test_xfdf_round_trip_consistency() {
    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("form.pdf");
    let xfdf_path1 = temp_dir.path().join("export1.xfdf");
    let xfdf_path2 = temp_dir.path().join("export2.xfdf");

    // Create a PDF with form fields
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("field1", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("Value1"),
        )
        .unwrap();
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("field2", Rect::new(100.0, 660.0, 200.0, 20.0))
                .with_value("Value2"),
        )
        .unwrap();
    editor.save(&pdf_path).unwrap();

    // Export twice and compare
    let mut editor1 = DocumentEditor::open(&pdf_path).unwrap();
    editor1.export_form_data_xfdf(&xfdf_path1).unwrap();

    let mut editor2 = DocumentEditor::open(&pdf_path).unwrap();
    editor2.export_form_data_xfdf(&xfdf_path2).unwrap();

    // Contents should be identical
    let content1 = std::fs::read_to_string(&xfdf_path1).unwrap();
    let content2 = std::fs::read_to_string(&xfdf_path2).unwrap();
    assert_eq!(content1, content2);
}
