//! Tests for embedded files (file attachments) functionality.

use pdf_oxide::api::Pdf;
use pdf_oxide::writer::{AFRelationship, EmbeddedFile};

/// Helper function to create a simple test PDF.
fn create_test_pdf() -> Vec<u8> {
    let pdf = Pdf::from_text("Test document for embedded files.").expect("Failed to create PDF");
    pdf.into_bytes()
}

mod embedded_file_struct {
    use super::*;

    #[test]
    fn test_embedded_file_new() {
        let file = EmbeddedFile::new("test.txt", b"Hello, World!".to_vec());
        assert_eq!(file.name, "test.txt");
        assert_eq!(file.size(), 13);
        assert!(file.description.is_none());
        assert!(file.mime_type.is_none());
    }

    #[test]
    fn test_embedded_file_builder() {
        let file = EmbeddedFile::new("data.csv", b"a,b,c".to_vec())
            .with_description("Test CSV file")
            .with_mime_type("text/csv")
            .with_creation_date("2024-01-15T10:30:00Z")
            .with_af_relationship(AFRelationship::Data);

        assert_eq!(file.description, Some("Test CSV file".to_string()));
        assert_eq!(file.mime_type, Some("text/csv".to_string()));
        assert_eq!(file.af_relationship, Some(AFRelationship::Data));
    }

    #[test]
    fn test_af_relationship_pdf_name() {
        assert_eq!(AFRelationship::Source.pdf_name(), "Source");
        assert_eq!(AFRelationship::Data.pdf_name(), "Data");
        assert_eq!(AFRelationship::Alternative.pdf_name(), "Alternative");
        assert_eq!(AFRelationship::Supplement.pdf_name(), "Supplement");
        assert_eq!(AFRelationship::EncryptedPayload.pdf_name(), "EncryptedPayload");
        assert_eq!(AFRelationship::FormData.pdf_name(), "FormData");
        assert_eq!(AFRelationship::Schema.pdf_name(), "Schema");
        assert_eq!(AFRelationship::Unspecified.pdf_name(), "Unspecified");
    }

    #[test]
    fn test_build_stream_dict() {
        let file = EmbeddedFile::new("test.txt", b"Hello".to_vec()).with_mime_type("text/plain");

        let dict = file.build_stream_dict();

        assert!(dict.contains_key("Type"));
        assert!(dict.contains_key("Subtype"));
        assert!(dict.contains_key("Params"));
    }
}

mod embed_file_api {
    use super::*;

    #[test]
    fn test_embed_file_simple() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_embed_input.pdf");
        let output_path = temp_dir.join("test_embed_output.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        // Embed a file
        pdf.embed_file("attachment.txt", b"This is an attached file.".to_vec())
            .expect("Failed to embed file");

        // Check pending files
        let pending = pdf.pending_embedded_files();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].name, "attachment.txt");

        // Save
        pdf.save(&output_path).expect("Failed to save PDF");

        // Verify output exists
        assert!(output_path.exists());

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_embed_file_with_options() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_embed_opts_input.pdf");
        let output_path = temp_dir.join("test_embed_opts_output.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        // Embed a file with options
        let file = EmbeddedFile::new("data.csv", b"col1,col2\na,b".to_vec())
            .with_description("Sales data")
            .with_mime_type("text/csv")
            .with_af_relationship(AFRelationship::Data);

        pdf.embed_file_with_options(file)
            .expect("Failed to embed file");

        // Check pending files
        let pending = pdf.pending_embedded_files();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].description, Some("Sales data".to_string()));
        assert_eq!(pending[0].mime_type, Some("text/csv".to_string()));

        // Save
        pdf.save(&output_path).expect("Failed to save PDF");

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_embed_multiple_files() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_embed_multi_input.pdf");
        let output_path = temp_dir.join("test_embed_multi_output.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        // Embed multiple files
        pdf.embed_file("file1.txt", b"Content 1".to_vec())
            .expect("Failed to embed file 1");
        pdf.embed_file("file2.txt", b"Content 2".to_vec())
            .expect("Failed to embed file 2");
        pdf.embed_file("file3.txt", b"Content 3".to_vec())
            .expect("Failed to embed file 3");

        // Check pending files
        assert_eq!(pdf.pending_embedded_files().len(), 3);

        // Save
        pdf.save(&output_path).expect("Failed to save PDF");

        // Verify output exists and is valid
        assert!(output_path.exists());
        let output_bytes = std::fs::read(&output_path).expect("Failed to read output");
        assert!(output_bytes.len() > bytes.len()); // Should be larger with attachments

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_clear_embedded_files() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_embed_clear_input.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        // Embed files
        pdf.embed_file("file1.txt", b"Content 1".to_vec()).unwrap();
        pdf.embed_file("file2.txt", b"Content 2".to_vec()).unwrap();
        assert_eq!(pdf.pending_embedded_files().len(), 2);

        // Clear
        pdf.clear_embedded_files();
        assert_eq!(pdf.pending_embedded_files().len(), 0);

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
    }

    #[test]
    fn test_embed_binary_file() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_embed_binary_input.pdf");
        let output_path = temp_dir.join("test_embed_binary_output.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        // Embed binary data (simulating an image)
        let binary_data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let file = EmbeddedFile::new("image.bin", binary_data.clone())
            .with_mime_type("application/octet-stream");

        pdf.embed_file_with_options(file)
            .expect("Failed to embed binary file");

        // Save
        pdf.save(&output_path).expect("Failed to save PDF");

        // Verify output exists
        assert!(output_path.exists());

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    }
}

mod embedded_files_builder {
    use super::*;
    use pdf_oxide::writer::EmbeddedFilesBuilder;

    #[test]
    fn test_builder_empty() {
        let builder = EmbeddedFilesBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
    }

    #[test]
    fn test_builder_add_files() {
        let mut builder = EmbeddedFilesBuilder::new();
        builder.add_file(EmbeddedFile::new("file1.txt", b"content1".to_vec()));
        builder.add_file(EmbeddedFile::new("file2.txt", b"content2".to_vec()));

        assert_eq!(builder.len(), 2);
        assert!(!builder.is_empty());
    }

    #[test]
    fn test_builder_into_files() {
        let mut builder = EmbeddedFilesBuilder::new();
        builder.add_file(EmbeddedFile::new("file.txt", b"content".to_vec()));

        let files = builder.into_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "file.txt");
    }
}
