use pdf_oxide::document::PdfDocument;

#[test]
fn out_of_range_page_index_returns_err() {
    let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let result = doc.extract_text(99999);
    assert!(result.is_err(), "Out-of-range page index should return Err");
}

#[test]
fn empty_bytes_returns_err() {
    let result = PdfDocument::from_bytes(vec![]);
    assert!(result.is_err(), "Empty bytes should return Err");
}

#[test]
fn garbage_bytes_returns_err() {
    let result = PdfDocument::from_bytes(vec![0xFF; 100]);
    assert!(result.is_err(), "Garbage bytes should return Err");
}
