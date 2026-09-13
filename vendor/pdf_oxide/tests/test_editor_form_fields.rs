//! Integration tests for form field editing in DocumentEditor.
//!
//! Tests the ability to add, modify, query, and remove form fields
//! from existing PDF documents.

use pdf_oxide::editor::{DocumentEditor, EditableDocument, FormFieldValue};
use pdf_oxide::geometry::Rect;
use pdf_oxide::writer::form_fields::{CheckboxWidget, ComboBoxWidget, TextFieldWidget};
use std::fs;
use tempfile::tempdir;

/// Test adding a text field to an existing PDF.
#[test]
fn test_add_text_field_to_existing_pdf() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a text field
    let name = editor
        .add_form_field(
            0,
            TextFieldWidget::new("email", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("test@example.com"),
        )
        .unwrap();

    assert_eq!(name, "email");
    assert!(editor.has_form_field("email").unwrap());

    // Save and verify
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("output.pdf");
    editor.save(&output_path).unwrap();

    // Verify the file was created and has content
    assert!(output_path.exists());
    let contents = fs::read(&output_path).unwrap();
    assert!(contents.len() > 100);
}

/// Test adding a checkbox field.
#[test]
fn test_add_checkbox_field() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a checkbox field
    let name = editor
        .add_form_field(
            0,
            CheckboxWidget::new("agree", Rect::new(100.0, 650.0, 15.0, 15.0)).checked(),
        )
        .unwrap();

    assert_eq!(name, "agree");
    assert!(editor.has_form_field("agree").unwrap());
}

/// Test adding a combo box field.
#[test]
fn test_add_combobox_field() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a combo box field
    let name = editor
        .add_form_field(
            0,
            ComboBoxWidget::new("country", Rect::new(100.0, 600.0, 150.0, 20.0))
                .with_options(vec!["USA", "Canada", "UK"])
                .with_value("USA"),
        )
        .unwrap();

    assert_eq!(name, "country");
    assert!(editor.has_form_field("country").unwrap());
}

/// Test unique name generation when adding duplicate field names.
#[test]
fn test_unique_field_names() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add first field
    let name1 = editor
        .add_form_field(0, TextFieldWidget::new("field", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();
    assert_eq!(name1, "field");

    // Add second field with same name - should get unique suffix
    let name2 = editor
        .add_form_field(0, TextFieldWidget::new("field", Rect::new(100.0, 650.0, 200.0, 20.0)))
        .unwrap();
    assert_eq!(name2, "field_1");

    // Both should exist
    assert!(editor.has_form_field("field").unwrap());
    assert!(editor.has_form_field("field_1").unwrap());
}

/// Test removing a form field.
#[test]
fn test_remove_form_field() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("to_remove", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    assert!(editor.has_form_field("to_remove").unwrap());

    // Remove it
    editor.remove_form_field("to_remove").unwrap();

    // Should no longer exist
    assert!(!editor.has_form_field("to_remove").unwrap());
}

/// Test setting form field value.
#[test]
fn test_set_form_field_value() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("name", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("initial"),
        )
        .unwrap();

    // Modify the value
    editor
        .set_form_field_value("name", FormFieldValue::Text("updated".to_string()))
        .unwrap();

    // Get the value
    let value = editor.get_form_field_value("name").unwrap();
    assert_eq!(value, Some(FormFieldValue::Text("updated".to_string())));
}

/// Test getting form fields returns added fields.
#[test]
fn test_get_form_fields() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add some fields
    editor
        .add_form_field(0, TextFieldWidget::new("field1", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();
    editor
        .add_form_field(0, TextFieldWidget::new("field2", Rect::new(100.0, 650.0, 200.0, 20.0)))
        .unwrap();

    // Get all fields
    let fields = editor.get_form_fields().unwrap();

    // Should have at least our 2 fields
    let field_names: Vec<_> = fields.iter().map(|f| f.name()).collect();
    assert!(field_names.contains(&"field1"));
    assert!(field_names.contains(&"field2"));
}

/// Test error when trying to set value on non-existent field.
#[test]
fn test_set_value_nonexistent_field() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    let result = editor.set_form_field_value("nonexistent", FormFieldValue::Text("value".into()));

    assert!(result.is_err());
}

/// Test error when trying to remove non-existent field.
#[test]
fn test_remove_nonexistent_field() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    let result = editor.remove_form_field("nonexistent");

    assert!(result.is_err());
}

/// Test page index validation.
#[test]
fn test_add_field_invalid_page() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Try to add to page 100 (doesn't exist)
    let result = editor
        .add_form_field(100, TextFieldWidget::new("field", Rect::new(100.0, 700.0, 200.0, 20.0)));

    assert!(result.is_err());
}

/// Test adding multiple field types to same page.
#[test]
fn test_multiple_field_types_same_page() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add various field types
    editor
        .add_form_field(0, TextFieldWidget::new("text", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();
    editor
        .add_form_field(0, CheckboxWidget::new("check", Rect::new(100.0, 650.0, 15.0, 15.0)))
        .unwrap();
    editor
        .add_form_field(
            0,
            ComboBoxWidget::new("combo", Rect::new(100.0, 600.0, 150.0, 20.0))
                .with_options(vec!["A", "B", "C"]),
        )
        .unwrap();

    // All should exist
    assert!(editor.has_form_field("text").unwrap());
    assert!(editor.has_form_field("check").unwrap());
    assert!(editor.has_form_field("combo").unwrap());

    // Save and verify
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("multi_field.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}
