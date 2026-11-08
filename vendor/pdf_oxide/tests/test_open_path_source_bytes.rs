//! `PdfDocument::open(path)` must populate `source_bytes`.
//!
//! Any API that re-reads the raw bytes (e.g. `compliance::convert_to_pdf_a`)
//! depends on `source_bytes` being non-empty after `open(path)`.
//! Guards against regressions where the in-memory copy was left empty.

use pdf_oxide::document::PdfDocument;

#[test]
fn open_path_populates_source_bytes() {
    // Use a small fixture that ships in-tree.
    let doc = PdfDocument::open("tests/fixtures/simple.pdf").expect("simple fixture opens");
    // ── pdf_manipulator patch: the source's bytes are read, not held as a field ──
    let source_bytes = doc.source_bytes_whole().expect("source bytes read");
    // ── end pdf_manipulator patch ──
    assert!(
        // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
        !source_bytes.is_empty(),
        "source_bytes must be populated after `open(path)` so byte-consuming \
         APIs (convert_to_pdf_a, FFI pdf_document_get_source_bytes, etc.) \
         see the document content"
    );
    // Sanity: bytes start with the PDF header.
    assert_eq!(&source_bytes[..5], b"%PDF-", "source_bytes should start with %PDF-");
        // ── end pdf_manipulator patch ──
}

#[test]
fn open_path_and_from_bytes_produce_equal_source_bytes() {
    let path = "tests/fixtures/simple.pdf";
    let bytes = std::fs::read(path).expect("read fixture");
    let from_path = PdfDocument::open(path).expect("open path");
    let from_bytes = PdfDocument::from_bytes(bytes.clone()).expect("from_bytes");
    // ── pdf_manipulator patch: the source's bytes are read, not held as a field ──
    let from_path_bytes = from_path.source_bytes_whole().expect("source bytes read");
    let from_bytes_bytes = from_bytes.source_bytes_whole().expect("source bytes read");
    // ── end pdf_manipulator patch ──
    assert_eq!(
        // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
        from_path_bytes, from_bytes_bytes,
        "open(path) should now hold the same bytes as from_bytes(read(path))"
    );
    // Both should also match the actual file contents.
    assert_eq!(from_path_bytes, bytes);
        // ── end pdf_manipulator patch ──
}
