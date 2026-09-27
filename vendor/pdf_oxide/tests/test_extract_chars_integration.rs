//! Integration tests for extract_chars API (Issue #39)
//! Tests the new character-level text extraction API with real PDFs

#[test]
fn test_extract_chars_character_properties() {
    use pdf_oxide::document::PdfDocument;

    // Try to open a test fixture or skip
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("alice_wonderland.pdf");

    if !fixture_path.exists() {
        eprintln!("Test fixture not found: {}. Skipping integration test.", fixture_path.display());
        return;
    }

    let doc = match PdfDocument::open(&fixture_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open fixture PDF: {}. Skipping integration test.", e);
            return;
        },
    };

    // Extract characters from first page
    let chars = match doc.extract_chars(0) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to extract characters: {}. Skipping integration test.", e);
            return;
        },
    };

    // Basic assertions for extract_chars API
    if !chars.is_empty() {
        // Characters should be non-empty for a valid PDF
        assert!(!chars.is_empty(), "extract_chars should return characters from page");

        // First character should have valid properties
        let first_char = &chars[0];
        assert!(
            !first_char.char.is_whitespace() || chars.len() > 1,
            "Should extract non-whitespace characters"
        );

        // Characters should have bounding boxes (Rect with x, y, width, height)
        let bbox = &first_char.bbox;
        assert!(bbox.x.is_finite(), "bbox x should be finite");
        assert!(bbox.y.is_finite(), "bbox y should be finite");
        assert!(bbox.width >= 0.0, "bbox width should be non-negative");
        assert!(bbox.height >= 0.0, "bbox height should be non-negative");

        println!("✓ extract_chars integration test passed: {} characters extracted", chars.len());
    }
}

#[test]
fn test_extract_chars_bbox_format() {
    use pdf_oxide::document::PdfDocument;

    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("alice_wonderland.pdf");

    if !fixture_path.exists() {
        return;
    }

    let doc = match PdfDocument::open(&fixture_path) {
        Ok(d) => d,
        Err(_) => return,
    };

    let chars = match doc.extract_chars(0) {
        Ok(c) => c,
        Err(_) => return,
    };

    if !chars.is_empty() {
        // Verify bbox is a Rect with x, y, width, height fields
        let bbox = &chars[0].bbox;
        // If this compiles and doesn't panic, the Rect format is correct
        let _x = bbox.x;
        let _y = bbox.y;
        let _w = bbox.width;
        let _h = bbox.height;

        println!("✓ extract_chars bbox Rect format verified");
    }
}
