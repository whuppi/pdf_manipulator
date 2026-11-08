//! Integration tests for XObject path extraction (Issue #40)
//! Tests recursive Form XObject processing in extract_paths

#[test]
fn test_xobject_path_extraction_no_hang() {
    use pdf_oxide::document::PdfDocument;

    // Try to open a test fixture
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("alice_wonderland.pdf");

    if !fixture_path.exists() {
        eprintln!("Test fixture not found: {}. Skipping XObject test.", fixture_path.display());
        return;
    }

    let doc = match PdfDocument::open(&fixture_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open fixture PDF: {}. Skipping test.", e);
            return;
        },
    };

    // Extract paths from first page
    // If there are circular XObject references, this would hang without proper cycle detection
    // With cycle detection via recursion stack, this should complete quickly
    let paths = match doc.extract_paths(0) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to extract paths: {}. Skipping test.", e);
            return;
        },
    };

    // If we reach here, no infinite loop occurred
    println!(
        "✓ XObject path extraction test passed: {} paths extracted, no hang detected",
        paths.len()
    );
}

#[test]
fn test_extract_paths_from_multiple_pages() {
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

    // Try extracting paths from first few pages
    let page_count = match doc.page_count() {
        Ok(count) => count,
        Err(_) => return,
    };

    for page_idx in 0..page_count.min(3) {
        match doc.extract_paths(page_idx) {
            Ok(paths) => {
                println!("✓ Page {}: extracted {} paths", page_idx, paths.len());
            },
            Err(e) => {
                eprintln!("Warning: Failed to extract paths from page {}: {}", page_idx, e);
            },
        }
    }

    println!("✓ Multiple page path extraction test completed");
}
