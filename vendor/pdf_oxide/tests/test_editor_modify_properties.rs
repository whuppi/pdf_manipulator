//! Integration tests for form field property modification in DocumentEditor.
//!
//! Tests the ability to modify field properties (flags, tooltip, rect, etc.)
//! on existing PDF form fields.

use pdf_oxide::editor::{DocumentEditor, EditableDocument, FormFieldValue};
use pdf_oxide::geometry::Rect;
use pdf_oxide::writer::form_fields::TextFieldWidget;
use tempfile::tempdir;

/// Test setting a field to read-only.
#[test]
fn test_set_field_readonly() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("name", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Set it to read-only
    editor.set_form_field_readonly("name", true).unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("readonly.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field as required.
#[test]
fn test_set_field_required() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("email", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Set it as required
    editor.set_form_field_required("email", true).unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("required.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field's tooltip.
#[test]
fn test_set_field_tooltip() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("phone", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Set tooltip
    editor
        .set_form_field_tooltip("phone", "Enter your phone number with country code")
        .unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("tooltip.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field's bounding rectangle.
#[test]
fn test_set_field_rect() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("address", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Change its position/size
    editor
        .set_form_field_rect("address", Rect::new(150.0, 650.0, 250.0, 30.0))
        .unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("rect_changed.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field's max length.
#[test]
fn test_set_field_max_length() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("ssn", Rect::new(100.0, 700.0, 100.0, 20.0)))
        .unwrap();

    // Set max length
    editor.set_form_field_max_length("ssn", 9).unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("maxlen.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field's alignment.
#[test]
fn test_set_field_alignment() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("amount", Rect::new(100.0, 700.0, 100.0, 20.0)))
        .unwrap();

    // Set alignment to center (1)
    editor.set_form_field_alignment("amount", 1).unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("alignment.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field's background color.
#[test]
fn test_set_field_background_color() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("highlight", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Set light yellow background
    editor
        .set_form_field_background_color("highlight", [1.0, 1.0, 0.8])
        .unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("bgcolor.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field's border color.
#[test]
fn test_set_field_border_color() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("important", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Set red border
    editor
        .set_form_field_border_color("important", [1.0, 0.0, 0.0])
        .unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("bordercolor.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting a field's border width.
#[test]
fn test_set_field_border_width() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("thick", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Set thick border
    editor.set_form_field_border_width("thick", 3.0).unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("borderwidth.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test setting field flags directly.
#[test]
fn test_set_field_flags() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("special", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Set flags: ReadOnly (1) | Required (2) = 3
    editor.set_form_field_flags("special", 0x03).unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("flags.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test modifying multiple properties on the same field.
#[test]
fn test_modify_multiple_properties() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(
            0,
            TextFieldWidget::new("complete", Rect::new(100.0, 700.0, 200.0, 20.0))
                .with_value("initial value"),
        )
        .unwrap();

    // Modify multiple properties
    editor
        .set_form_field_tooltip("complete", "A fully customized field")
        .unwrap();
    editor.set_form_field_readonly("complete", true).unwrap();
    editor.set_form_field_alignment("complete", 2).unwrap(); // right align
    editor
        .set_form_field_background_color("complete", [0.9, 0.9, 1.0])
        .unwrap(); // light blue

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("multi_props.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test error when modifying non-existent field.
#[test]
fn test_modify_nonexistent_field() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    let result = editor.set_form_field_readonly("nonexistent", true);

    assert!(result.is_err());
}

/// Test error when modifying deleted field.
#[test]
fn test_modify_deleted_field() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add and then delete a field
    editor
        .add_form_field(0, TextFieldWidget::new("to_delete", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();
    editor.remove_form_field("to_delete").unwrap();

    // Try to modify the deleted field
    let result = editor.set_form_field_readonly("to_delete", true);

    assert!(result.is_err());
}

/// Test modifying value and properties together.
#[test]
fn test_modify_value_and_properties() {
    let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

    // Add a field
    editor
        .add_form_field(0, TextFieldWidget::new("combined", Rect::new(100.0, 700.0, 200.0, 20.0)))
        .unwrap();

    // Modify value
    editor
        .set_form_field_value("combined", FormFieldValue::Text("Hello World".to_string()))
        .unwrap();

    // Modify properties
    editor.set_form_field_required("combined", true).unwrap();
    editor
        .set_form_field_tooltip("combined", "Enter your greeting")
        .unwrap();

    // Save
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("value_and_props.pdf");
    editor.save(&output_path).unwrap();

    assert!(output_path.exists());
}

/// Test that modification persistence works (regression test for the save bug).
#[test]
fn test_modification_persists_on_save() {
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("persist_test.pdf");

    // Create and modify a field, then save
    {
        let mut editor = DocumentEditor::open("tests/fixtures/simple.pdf").unwrap();

        editor
            .add_form_field(
                0,
                TextFieldWidget::new("persist", Rect::new(100.0, 700.0, 200.0, 20.0)),
            )
            .unwrap();

        // Set value and properties
        editor
            .set_form_field_value("persist", FormFieldValue::Text("Test Value".to_string()))
            .unwrap();
        editor.set_form_field_readonly("persist", true).unwrap();

        editor.save(&output_path).unwrap();
    }

    // Reload and verify the field exists
    {
        let mut editor = DocumentEditor::open(&output_path).unwrap();
        let fields = editor.get_form_fields().unwrap();
        let field_names: Vec<_> = fields.iter().map(|f| f.name()).collect();

        assert!(
            field_names.contains(&"persist"),
            "Field 'persist' not found after reload. Found: {:?}",
            field_names
        );
    }
}
