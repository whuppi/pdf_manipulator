//! Tests for PDF header parsing with binary prefixes and resilience handling.
//!
//! Verifies that PDFs with binary data before the PDF header can be parsed.
//! Tests lenient mode which searches first 8192 bytes for %PDF- marker.

#[test]
fn test_pdf_header_parsing_basic() {
    use pdf_oxide::document::PdfDocument;
    use std::path::Path;

    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("simple.pdf");

    assert!(fixture_path.exists(), "Test fixture missing: {}", fixture_path.display());

    let pdf_path = fixture_path.to_str().unwrap();

    // Open PDF successfully
    let doc = match PdfDocument::open(pdf_path) {
        Ok(d) => d,
        Err(e) => panic!("Failed to open PDF: {}", e),
    };

    // Verify version info
    let (major, _minor) = doc.version();
    assert!(major >= 1, "Invalid PDF major version");

    // Verify page count
    let page_count = doc.page_count().expect("Failed to get page count");
    assert!(page_count > 0, "PDF should have at least one page");

    // Attempt text extraction (may be empty for minimal fixtures)
    let _ = doc.extract_spans(0);
}

#[test]
fn test_pdf_header_parsing_multiple_pages() {
    use pdf_oxide::document::PdfDocument;
    use std::path::Path;

    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("simple.pdf");

    assert!(fixture_path.exists(), "Test fixture missing: {}", fixture_path.display());

    let pdf_path = fixture_path.to_str().unwrap();

    // Open PDF document
    let doc = PdfDocument::open(pdf_path).expect("Failed to open PDF");

    // Verify version
    let (major, _minor) = doc.version();
    assert_eq!(major, 1);

    // Verify page count
    let page_count = doc.page_count().expect("Failed to get page count");
    assert!(page_count > 0);

    // Test extraction on each page (gracefully handle extraction errors)
    for i in 0..page_count {
        let _ = doc.extract_spans(i);
    }
}

#[test]
fn test_header_beyond_1024_bytes() {
    use pdf_oxide::document::parse_header;
    use std::io::Cursor;

    // 2000 bytes of junk followed by a valid PDF header
    let mut data = vec![b'X'; 2000];
    data.extend_from_slice(b"%PDF-1.4\n");

    let mut cursor = Cursor::new(data);
    let (major, minor, offset) = parse_header(&mut cursor, true).unwrap();
    assert_eq!(major, 1);
    assert_eq!(minor, 4);
    assert_eq!(offset, 2000);
}

#[test]
fn test_header_beyond_8192_bytes_falls_back() {
    use pdf_oxide::document::parse_header;
    use std::io::Cursor;

    // 9000 bytes of junk — beyond the 8192-byte search window.
    // In lenient mode, headerless PDFs default to version 1.4.
    let mut data = vec![b'X'; 9000];
    data.extend_from_slice(b"%PDF-1.4\n");

    let mut cursor = Cursor::new(data.clone());
    let (major, minor, offset) = parse_header(&mut cursor, true).unwrap();
    assert_eq!((major, minor), (1, 4));
    assert_eq!(offset, 0); // falls back to start

    // Strict mode should fail
    let mut cursor = Cursor::new(data);
    assert!(parse_header(&mut cursor, false).is_err());
}

#[test]
fn test_header_with_newline_in_version() {
    use pdf_oxide::document::parse_header;
    use std::io::Cursor;

    // %PDF-1.\n — newline where minor version digit should be
    let data = b"%PDF-1.\n";
    let mut cursor = Cursor::new(&data[..]);
    // Strict mode should fail
    assert!(parse_header(&mut cursor, false).is_err());

    // Lenient mode (via search path): put junk before to force lenient parsing path
    let mut lenient_data = vec![b'X'; 1];
    lenient_data.extend_from_slice(b"%PDF-1.\n");
    let mut cursor = Cursor::new(lenient_data);
    let (major, minor, _offset) = parse_header(&mut cursor, true).unwrap();
    assert_eq!((major, minor), (1, 4)); // defaults to 1.4
}

#[test]
fn test_header_with_letter_version() {
    use pdf_oxide::document::parse_header;
    use std::io::Cursor;

    // %PDF-a.4 — letter where major version digit should be
    let mut data = vec![b'X'; 1];
    data.extend_from_slice(b"%PDF-a.4");
    data.push(b'\n');
    let mut cursor = Cursor::new(data);
    let (major, minor, _offset) = parse_header(&mut cursor, true).unwrap();
    assert_eq!((major, minor), (1, 4)); // defaults to 1.4
}

#[test]
fn test_authenticate_empty_password() {
    use pdf_oxide::document::PdfDocument;
    use std::path::Path;

    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("simple.pdf");

    assert!(fixture_path.exists(), "Test fixture missing: {}", fixture_path.display());

    let doc = PdfDocument::open(&fixture_path).unwrap();
    // Non-encrypted PDF: authenticate always returns true
    let result = doc.authenticate(b"").unwrap();
    assert!(result, "Non-encrypted PDF should always authenticate");
}
