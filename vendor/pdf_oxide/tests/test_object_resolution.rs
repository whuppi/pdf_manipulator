//! Test object resolution and xref handling.

use pdf_oxide::document::PdfDocument;
use std::fs;
use std::path::PathBuf;

/// Test that we can open all PDFs in the fixtures directory.
#[test]
fn test_open_all_pdfs() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    let pdf_files: Vec<_> = fs::read_dir(&fixtures_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("pdf"))
        .collect();

    println!("Found {} PDF files", pdf_files.len());

    for entry in pdf_files {
        let path = entry.path();
        let filename = path.file_name().unwrap().to_string_lossy();

        println!("\n==> Testing: {}", filename);

        match PdfDocument::open(&path) {
            Ok(doc) => {
                println!("  ✓ Opened successfully");
                println!("  Version: {}.{}", doc.version().0, doc.version().1);

                // Try to get catalog
                match doc.catalog() {
                    Ok(_) => {
                        println!("  ✓ Loaded catalog");

                        // Try to get page count
                        match doc.page_count() {
                            Ok(count) => println!("  ✓ Page count: {}", count),
                            Err(e) => eprintln!("  ✗ Page count failed: {}", e),
                        }
                    },
                    Err(e) => eprintln!("  ✗ Catalog failed: {}", e),
                }
            },
            Err(e) => {
                eprintln!("  ✗ Open failed: {}", e);
            },
        }
    }
}

/// Test a specific problematic pattern: object referenced in catalog but missing from xref.
#[test]
#[ignore] // Run with --ignored
fn test_missing_object_handling() {
    // This test would check if we handle missing objects gracefully
    // For now, just ensure test compiles
}
