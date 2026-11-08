use pdf_oxide::document::PdfDocument;

#[test]
fn test_from_bytes_valid_pdf() {
    let data = std::fs::read("tests/fixtures/simple.pdf").unwrap();
    let doc = PdfDocument::from_bytes(data).unwrap();
    let pages = doc.page_count().unwrap();
    assert!(pages > 0, "Should have at least 1 page");
    let _text = doc.extract_text(0).unwrap();
}

#[test]
fn test_from_bytes_matches_file() {
    let data = std::fs::read("tests/fixtures/simple.pdf").unwrap();

    let doc_file = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let doc_bytes = PdfDocument::from_bytes(data).unwrap();

    let pages = doc_file.page_count().unwrap();
    for p in 0..pages {
        let t1 = doc_file.extract_text(p).unwrap();
        let t2 = doc_bytes.extract_text(p).unwrap();
        assert_eq!(t1, t2, "Page {} text should match between open() and from_bytes()", p);
    }
}

#[test]
fn test_from_bytes_page_count() {
    let data = std::fs::read("tests/fixtures/outline.pdf").unwrap();

    let doc_file = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let doc_bytes = PdfDocument::from_bytes(data).unwrap();

    assert_eq!(
        doc_file.page_count().unwrap(),
        doc_bytes.page_count().unwrap(),
        "Page count should match"
    );
}

#[test]
fn test_from_bytes_invalid_header() {
    let result = PdfDocument::from_bytes(b"not a pdf".to_vec());
    assert!(result.is_err(), "Non-PDF data should return Err");
}

#[test]
fn test_from_bytes_truncated() {
    let truncated = b"%PDF-1.4\n1 0 obj\n".to_vec();
    let result = PdfDocument::from_bytes(truncated);
    assert!(result.is_err(), "Truncated PDF should return Err");
}
