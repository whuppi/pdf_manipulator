//! Integration tests for annotation editing functionality.
//!
//! Tests the complete annotation workflow:
//! - Reading annotations from PDFs
//! - Adding new annotations via Editor API
//! - Modifying existing annotations
//! - Round-trip preservation of annotation properties

use pdf_oxide::annotation_types::AnnotationSubtype;
use pdf_oxide::editor::{AnnotationWrapper, DocumentEditor};
use pdf_oxide::geometry::Rect;
use pdf_oxide::writer::{DocumentBuilder, LinkAnnotation, PageSize, TextAnnotation};

/// Helper to create a simple test PDF with text.
fn create_simple_pdf() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0).text("Hello World").done();
    }
    builder.build().expect("Failed to build PDF")
}

/// Helper to create a test PDF with a link annotation.
fn create_pdf_with_link() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0)
            .text("Click here")
            .link_url("https://example.com")
            .done();
    }
    builder.build().expect("Failed to build PDF")
}

/// Helper to create a test PDF with highlighted text.
fn create_pdf_with_highlight() -> Vec<u8> {
    let mut builder = DocumentBuilder::new();
    {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0)
            .text("Highlighted text")
            .highlight((1.0, 1.0, 0.0))
            .done();
    }
    builder.build().expect("Failed to build PDF")
}

/// Test that we can read annotations from a page.
#[test]
fn test_read_page_annotations() {
    let bytes = create_pdf_with_link();

    // Write to temp file
    let temp_path = std::env::temp_dir().join("test_read_annotations.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    // Open with editor and read annotations
    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let page = editor.get_page(0).expect("Failed to get page");

    // Verify annotations are loaded
    assert!(
        page.annotation_count() >= 1,
        "Expected at least 1 annotation, found {}",
        page.annotation_count()
    );

    // Clean up
    let _ = std::fs::remove_file(&temp_path);
}

/// Test adding a new link annotation via Editor API.
#[test]
fn test_add_link_annotation() {
    let bytes = create_simple_pdf();

    let temp_path = std::env::temp_dir().join("test_add_link.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    // Open and add annotation
    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let mut page = editor.get_page(0).expect("Failed to get page");

    let initial_count = page.annotation_count();

    // Add a link annotation
    let link = LinkAnnotation::uri(Rect::new(72.0, 700.0, 100.0, 20.0), "https://rust-lang.org");
    let annot_id = page.add_annotation(link);

    // Verify annotation was added
    assert_eq!(page.annotation_count(), initial_count + 1);

    // Verify we can find the annotation by ID
    let found = page.find_annotation(annot_id);
    assert!(found.is_some(), "Should find annotation by ID");
    assert_eq!(found.unwrap().subtype(), AnnotationSubtype::Link);

    // Save the page
    editor.save_page(page).expect("Failed to save page");

    // Clean up
    let _ = std::fs::remove_file(&temp_path);
}

/// Test adding a text (sticky note) annotation.
#[test]
fn test_add_sticky_note_annotation() {
    let bytes = create_simple_pdf();

    let temp_path = std::env::temp_dir().join("test_sticky_note.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let mut page = editor.get_page(0).expect("Failed to get page");

    // Add a sticky note
    let note =
        TextAnnotation::new(Rect::new(100.0, 700.0, 24.0, 24.0), "Please review this section");
    let annot_id = page.add_annotation(note);

    // Verify
    let annot = page
        .find_annotation(annot_id)
        .expect("Should find annotation");
    assert_eq!(annot.subtype(), AnnotationSubtype::Text);

    editor.save_page(page).expect("Failed to save page");
    let _ = std::fs::remove_file(&temp_path);
}

/// Test removing an annotation.
#[test]
fn test_remove_annotation() {
    let bytes = create_pdf_with_link();

    let temp_path = std::env::temp_dir().join("test_remove_annotation.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let mut page = editor.get_page(0).expect("Failed to get page");

    let initial_count = page.annotation_count();
    assert!(initial_count >= 1, "Need at least 1 annotation to test removal");

    // Remove the first annotation
    let removed = page.remove_annotation(0);
    assert!(removed.is_some(), "Should return removed annotation");

    // Verify count decreased
    assert_eq!(page.annotation_count(), initial_count - 1);

    editor.save_page(page).expect("Failed to save page");
    let _ = std::fs::remove_file(&temp_path);
}

/// Test finding annotations by type.
#[test]
fn test_find_annotations_by_type() {
    let bytes = create_pdf_with_link();

    let temp_path = std::env::temp_dir().join("test_find_by_type.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let page = editor.get_page(0).expect("Failed to get page");

    // Find all link annotations
    let links = page.find_annotations_by_type(AnnotationSubtype::Link);
    assert!(!links.is_empty(), "Should find at least 1 link annotation");

    let _ = std::fs::remove_file(&temp_path);
}

/// Test finding annotations in a region.
#[test]
fn test_find_annotations_in_region() {
    let bytes = create_pdf_with_link();

    let temp_path = std::env::temp_dir().join("test_find_in_region.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let page = editor.get_page(0).expect("Failed to get page");

    // Find annotations in top region (where we placed the link)
    let top_region = Rect::new(0.0, 600.0, 612.0, 200.0);
    let top_annotations = page.find_annotations_in_region(top_region);

    assert!(!top_annotations.is_empty(), "Should find annotations in top region");

    let _ = std::fs::remove_file(&temp_path);
}

/// Test that annotations track modification state.
#[test]
fn test_annotation_modification_tracking() {
    let bytes = create_simple_pdf();

    let temp_path = std::env::temp_dir().join("test_modification_tracking.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let mut page = editor.get_page(0).expect("Failed to get page");

    // Initially not modified
    assert!(!page.has_annotations_modified());

    // Add an annotation
    let note = TextAnnotation::new(Rect::new(100.0, 700.0, 24.0, 24.0), "New note");
    page.add_annotation(note);

    // Now should be modified
    assert!(page.has_annotations_modified());

    let _ = std::fs::remove_file(&temp_path);
}

/// Test round-trip preservation of raw dictionary.
#[test]
fn test_raw_dict_preservation() {
    let bytes = create_pdf_with_link();

    let temp_path = std::env::temp_dir().join("test_raw_dict.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let page = editor.get_page(0).expect("Failed to get page");

    // Check that annotations from file have raw_dict preserved
    for annot in page.annotations() {
        if !annot.is_new() {
            // Existing annotations should have raw_dict available
            // (for round-trip preservation)
            let raw = annot.raw_dict();
            // raw_dict should be Some for annotations loaded from PDF
            assert!(raw.is_some(), "Annotations from PDF should have raw_dict");
        }
    }

    let _ = std::fs::remove_file(&temp_path);
}

/// Test AnnotationWrapper subtype detection for writer annotations.
#[test]
fn test_annotation_wrapper_subtype_detection() {
    // Create wrappers for different annotation types
    let link = LinkAnnotation::uri(Rect::new(0.0, 0.0, 100.0, 20.0), "https://example.com");
    let wrapper = AnnotationWrapper::from_write(link);
    assert_eq!(wrapper.subtype(), AnnotationSubtype::Link);

    let note = TextAnnotation::new(Rect::new(0.0, 0.0, 24.0, 24.0), "Comment");
    let wrapper = AnnotationWrapper::from_write(note);
    assert_eq!(wrapper.subtype(), AnnotationSubtype::Text);

    // Verify is_new works
    assert!(wrapper.is_new());
    assert!(wrapper.is_modified());
}

/// Test that PdfPage exposes annotations correctly.
#[test]
fn test_pdf_page_annotation_accessors() {
    let bytes = create_simple_pdf();

    let temp_path = std::env::temp_dir().join("test_accessors.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let mut page = editor.get_page(0).expect("Failed to get page");

    // Add some annotations
    page.add_annotation(TextAnnotation::new(Rect::new(100.0, 700.0, 24.0, 24.0), "Note 1"));
    page.add_annotation(TextAnnotation::new(Rect::new(200.0, 700.0, 24.0, 24.0), "Note 2"));

    // Test annotations()
    let annotations = page.annotations();
    assert!(annotations.len() >= 2);

    // Test annotation() by index
    let first = page.annotation(0);
    assert!(first.is_some());

    // Test annotation_mut() by index
    let first_mut = page.annotation_mut(0);
    assert!(first_mut.is_some());

    // Test annotation_count()
    let count = page.annotation_count();
    assert!(count >= 2);

    let _ = std::fs::remove_file(&temp_path);
}

/// Test remove annotation by ID.
#[test]
fn test_remove_annotation_by_id() {
    let bytes = create_simple_pdf();

    let temp_path = std::env::temp_dir().join("test_remove_by_id.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let mut page = editor.get_page(0).expect("Failed to get page");

    // Add an annotation and get its ID
    let note = TextAnnotation::new(Rect::new(100.0, 700.0, 24.0, 24.0), "To be removed");
    let annot_id = page.add_annotation(note);

    let count_after_add = page.annotation_count();

    // Remove by ID
    let removed = page.remove_annotation_by_id(annot_id);
    assert!(removed.is_some(), "Should find and remove annotation by ID");

    // Verify count decreased
    assert_eq!(page.annotation_count(), count_after_add - 1);

    // Verify annotation is no longer findable
    assert!(page.find_annotation(annot_id).is_none());

    let _ = std::fs::remove_file(&temp_path);
}

/// Test highlight annotation integration.
#[test]
fn test_highlight_annotation() {
    let bytes = create_pdf_with_highlight();

    let temp_path = std::env::temp_dir().join("test_highlight.pdf");
    std::fs::write(&temp_path, bytes).expect("Failed to write PDF");

    let mut editor = DocumentEditor::open(&temp_path).expect("Failed to open PDF");
    let page = editor.get_page(0).expect("Failed to get page");

    // Find highlight annotations
    let highlights = page.find_annotations_by_type(AnnotationSubtype::Highlight);
    assert!(!highlights.is_empty(), "Should find at least 1 highlight annotation");

    let _ = std::fs::remove_file(&temp_path);
}
