//! Integration tests for PDF editing functionality.

use pdf_oxide::editor::{DocumentEditor, DocumentInfo, EditableDocument, SaveOptions};
use pdf_oxide::writer::{DocumentBuilder, DocumentMetadata, PageSize};
use std::fs;
use tempfile::tempdir;

/// Helper to create a simple test PDF
fn create_test_pdf(path: &str) -> std::io::Result<()> {
    let mut builder = DocumentBuilder::new();
    builder = builder.metadata(
        DocumentMetadata::new()
            .title("Test Document")
            .author("Test Author")
            .subject("Test Subject"),
    );

    // Add pages - need to go through the mutable reference properly
    {
        let page1 = builder.page(PageSize::Letter);
        page1.at(72.0, 720.0).text("Page 1 Content").done();
    }
    {
        let page2 = builder.page(PageSize::Letter);
        page2.at(72.0, 720.0).text("Page 2 Content").done();
    }
    {
        let page3 = builder.page(PageSize::Letter);
        page3.at(72.0, 720.0).text("Page 3 Content").done();
    }

    let pdf_bytes = builder.build().expect("Failed to build test PDF");
    fs::write(path, pdf_bytes)
}

mod document_info_tests {
    use super::*;

    #[test]
    fn test_document_info_builder() {
        let info = DocumentInfo::new()
            .title("My Title")
            .author("My Author")
            .subject("My Subject")
            .keywords("rust, pdf, test")
            .creator("test_editor.rs")
            .producer("pdf_oxide");

        assert_eq!(info.title, Some("My Title".to_string()));
        assert_eq!(info.author, Some("My Author".to_string()));
        assert_eq!(info.subject, Some("My Subject".to_string()));
        assert_eq!(info.keywords, Some("rust, pdf, test".to_string()));
        assert_eq!(info.creator, Some("test_editor.rs".to_string()));
        assert_eq!(info.producer, Some("pdf_oxide".to_string()));
    }

    #[test]
    fn test_document_info_to_object() {
        let info = DocumentInfo::new().title("Test PDF").author("John Doe");

        let obj = info.to_object();
        let dict = obj.as_dict().expect("Should be dictionary");

        assert!(dict.contains_key("Title"));
        assert!(dict.contains_key("Author"));
        assert!(!dict.contains_key("Subject"));
        assert!(!dict.contains_key("Keywords"));
    }

    #[test]
    fn test_document_info_round_trip() {
        let original = DocumentInfo::new()
            .title("Round Trip Test")
            .author("Test Author")
            .subject("Testing round trip")
            .keywords("test, round, trip");

        let obj = original.to_object();
        let restored = DocumentInfo::from_object(&obj);

        assert_eq!(original.title, restored.title);
        assert_eq!(original.author, restored.author);
        assert_eq!(original.subject, restored.subject);
        assert_eq!(original.keywords, restored.keywords);
    }
}

mod save_options_tests {
    use super::*;

    #[test]
    fn test_full_rewrite_options() {
        let opts = SaveOptions::full_rewrite();
        assert!(!opts.incremental);
        assert!(opts.compress);
        assert!(opts.garbage_collect);
    }

    #[test]
    fn test_incremental_options() {
        let opts = SaveOptions::incremental();
        assert!(opts.incremental);
        assert!(!opts.compress);
        assert!(!opts.garbage_collect);
    }
}

mod editor_open_tests {
    use super::*;

    #[test]
    fn test_open_valid_pdf() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("test.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let result = DocumentEditor::open(&pdf_path);
        assert!(result.is_ok(), "Should open valid PDF");

        let editor = result.unwrap();
        assert!(!editor.is_modified());
    }

    #[test]
    fn test_open_nonexistent_pdf() {
        let result = DocumentEditor::open("/nonexistent/path/to/file.pdf");
        assert!(result.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_source_path() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("source.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let editor = DocumentEditor::open(&pdf_path).unwrap();
        assert!(editor.source_path().contains("source.pdf"));
    }

    #[test]
    fn test_version() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("version.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let editor = DocumentEditor::open(&pdf_path).unwrap();
        let (major, minor) = editor.version();

        // PDF version should be 1.x
        assert_eq!(major, 1);
        assert!((4..=7).contains(&minor));
    }
}

mod metadata_editing_tests {
    use super::*;

    #[test]
    fn test_set_title() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("title.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        assert!(!editor.is_modified());

        editor.set_title("New Title");

        assert!(editor.is_modified());
        assert_eq!(editor.title().unwrap(), Some("New Title".to_string()));
    }

    #[test]
    fn test_set_author() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("author.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        editor.set_author("Jane Doe");

        assert!(editor.is_modified());
        assert_eq!(editor.author().unwrap(), Some("Jane Doe".to_string()));
    }

    #[test]
    fn test_set_subject() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("subject.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        editor.set_subject("Updated Subject");

        assert!(editor.is_modified());
        assert_eq!(editor.subject().unwrap(), Some("Updated Subject".to_string()));
    }

    #[test]
    fn test_set_keywords() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("keywords.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        editor.set_keywords("pdf, rust, editing");

        assert!(editor.is_modified());
        assert_eq!(editor.keywords().unwrap(), Some("pdf, rust, editing".to_string()));
    }

    #[test]
    fn test_set_info() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("info.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();

        let new_info = DocumentInfo::new()
            .title("New Document")
            .author("New Author")
            .subject("New Subject");

        editor.set_info(new_info).unwrap();

        let retrieved = editor.get_info().unwrap();
        assert_eq!(retrieved.title, Some("New Document".to_string()));
        assert_eq!(retrieved.author, Some("New Author".to_string()));
        assert_eq!(retrieved.subject, Some("New Subject".to_string()));
    }
}

mod page_operations_tests {
    use super::*;

    #[test]
    fn test_page_count() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("pagecount.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        let count = editor.page_count().unwrap();

        assert_eq!(count, 3);
    }

    #[test]
    fn test_current_page_count() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("current_count.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let editor = DocumentEditor::open(&pdf_path).unwrap();
        assert_eq!(editor.current_page_count(), 3);
    }

    #[test]
    fn test_get_page_info() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("pageinfo.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        let info = editor.get_page_info(0).unwrap();

        assert_eq!(info.index, 0);
        // Letter size is 612 x 792 points
        assert!((info.width - 612.0).abs() < 1.0);
        assert!((info.height - 792.0).abs() < 1.0);
    }

    #[test]
    fn test_get_page_info_out_of_range() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("pageinfo_oor.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        let result = editor.get_page_info(100);

        assert!(result.is_err());
    }

    #[test]
    fn test_remove_page() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("remove.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        assert_eq!(editor.current_page_count(), 3);

        editor.remove_page(1).unwrap();

        assert!(editor.is_modified());
        assert_eq!(editor.current_page_count(), 2);
    }

    #[test]
    fn test_remove_page_out_of_range() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("remove_oor.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        let result = editor.remove_page(100);

        assert!(result.is_err());
    }

    #[test]
    fn test_move_page() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("move.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();

        // Move page 2 to position 0
        editor.move_page(2, 0).unwrap();

        assert!(editor.is_modified());
        assert_eq!(editor.current_page_count(), 3);
    }

    #[test]
    fn test_move_page_out_of_range() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("move_oor.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        let result = editor.move_page(100, 0);

        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_page() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("duplicate.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        assert_eq!(editor.current_page_count(), 3);

        let new_index = editor.duplicate_page(0).unwrap();

        assert!(editor.is_modified());
        assert_eq!(editor.current_page_count(), 4);
        assert_eq!(new_index, 3); // Added at the end
    }

    #[test]
    fn test_duplicate_page_out_of_range() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("dup_oor.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        let result = editor.duplicate_page(100);

        assert!(result.is_err());
    }
}

mod save_tests {
    use super::*;

    #[test]
    fn test_save_unmodified() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("unmodified.pdf");
        let output_path = dir.path().join("output.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();

        // Save without modifications
        let result = editor.save(&output_path);
        assert!(result.is_ok());

        // Output file should exist
        assert!(output_path.exists());

        // Should be able to open the saved file
        let reopened = DocumentEditor::open(&output_path);
        assert!(reopened.is_ok());
    }

    #[test]
    fn test_save_with_metadata_changes() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("meta_change.pdf");
        let output_path = dir.path().join("meta_output.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        editor.set_title("Modified Title");
        editor.set_author("Modified Author");

        let result = editor.save(&output_path);
        assert!(result.is_ok());

        // Verify the output file exists
        assert!(output_path.exists());
    }

    #[test]
    fn test_save_with_options() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("with_options.pdf");
        let output_path = dir.path().join("options_output.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        editor.set_title("Options Test");

        let options = SaveOptions::full_rewrite();
        let result = editor.save_with_options(&output_path, options);

        assert!(result.is_ok());
        assert!(output_path.exists());
    }
}

mod merge_tests {
    use super::*;

    #[test]
    fn test_merge_from() {
        let dir = tempdir().unwrap();
        let main_path = dir.path().join("main.pdf");
        let append_path = dir.path().join("append.pdf");

        create_test_pdf(main_path.to_str().unwrap()).unwrap();
        create_test_pdf(append_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&main_path).unwrap();

        let merged_count = editor.merge_from(&append_path).unwrap();
        assert_eq!(merged_count, 3); // Source has 3 pages
        assert!(editor.is_modified());
    }

    #[test]
    fn test_merge_from_nonexistent() {
        let dir = tempdir().unwrap();
        let main_path = dir.path().join("main.pdf");

        create_test_pdf(main_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&main_path).unwrap();

        let result = editor.merge_from("/nonexistent/file.pdf");
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_pages_from() {
        let dir = tempdir().unwrap();
        let main_path = dir.path().join("main.pdf");
        let source_path = dir.path().join("source.pdf");

        create_test_pdf(main_path.to_str().unwrap()).unwrap();
        create_test_pdf(source_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&main_path).unwrap();

        // Merge only pages 0 and 2
        let merged_count = editor.merge_pages_from(&source_path, &[0, 2]).unwrap();
        assert_eq!(merged_count, 2);
        assert!(editor.is_modified());
    }

    #[test]
    fn test_merge_pages_empty() {
        let dir = tempdir().unwrap();
        let main_path = dir.path().join("main.pdf");
        let source_path = dir.path().join("source.pdf");

        create_test_pdf(main_path.to_str().unwrap()).unwrap();
        create_test_pdf(source_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&main_path).unwrap();

        // Merge no pages
        let merged_count = editor.merge_pages_from(&source_path, &[]).unwrap();
        assert_eq!(merged_count, 0);
    }

    #[test]
    fn test_merge_pages_out_of_range() {
        let dir = tempdir().unwrap();
        let main_path = dir.path().join("main.pdf");
        let source_path = dir.path().join("source.pdf");

        create_test_pdf(main_path.to_str().unwrap()).unwrap();
        create_test_pdf(source_path.to_str().unwrap()).unwrap();

        let mut editor = DocumentEditor::open(&main_path).unwrap();

        // Try to merge page that doesn't exist
        let result = editor.merge_pages_from(&source_path, &[100]);
        assert!(result.is_err());
    }
}

mod integration_tests {
    use super::*;

    #[test]
    fn test_complete_editing_workflow() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("workflow.pdf");
        let output_path = dir.path().join("workflow_output.pdf");

        // Create source PDF
        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        // Open for editing
        let mut editor = DocumentEditor::open(&pdf_path).unwrap();
        assert_eq!(editor.current_page_count(), 3);

        // Modify metadata
        editor.set_title("Edited Document");
        editor.set_author("Workflow Tester");
        editor.set_subject("Integration Test");
        editor.set_keywords("workflow, test, integration");

        // Modify pages
        editor.remove_page(1).unwrap(); // Remove middle page
        assert_eq!(editor.current_page_count(), 2);

        // Duplicate first page
        editor.duplicate_page(0).unwrap();
        assert_eq!(editor.current_page_count(), 3);

        // Verify modified flag
        assert!(editor.is_modified());

        // Save changes
        let result = editor.save(&output_path);
        assert!(result.is_ok());

        // Verify output exists
        assert!(output_path.exists());
    }

    #[test]
    fn test_metadata_preservation() {
        let dir = tempdir().unwrap();
        let pdf_path = dir.path().join("preserve.pdf");
        let output_path = dir.path().join("preserve_output.pdf");

        create_test_pdf(pdf_path.to_str().unwrap()).unwrap();

        // Open and modify
        {
            let mut editor = DocumentEditor::open(&pdf_path).unwrap();
            editor.set_title("Preserved Title");
            editor.set_author("Preserved Author");
            editor.save(&output_path).unwrap();
        }

        // Reopen and verify (basic check - file should open)
        {
            let reopened = DocumentEditor::open(&output_path);
            assert!(reopened.is_ok());
        }
    }
}
