//! save_to_bytes / open_from_bytes in-memory round-trip test (#409).
//!
//! Builds a PDF entirely in memory (no filesystem), then re-opens it from
//! bytes and verifies that the original text is still extractable.
#![allow(clippy::missing_safety_doc)]
#![allow(unused_unsafe)]

use pdf_oxide::ffi::*;
use std::ffi::CString;

fn cstring(s: &str) -> CString {
    CString::new(s).unwrap()
}

#[test]
fn save_to_bytes_round_trip() {
    let mut ec: i32 = -1;
    let builder = unsafe { pdf_document_builder_create(&mut ec) };
    assert_eq!(ec, 0);
    let page = unsafe { pdf_document_builder_letter_page(builder, &mut ec) };
    assert_eq!(ec, 0);
    assert_eq!(
        unsafe { pdf_page_builder_font(page, cstring("Helvetica").as_ptr(), 12.0, &mut ec) },
        0
    );
    assert_eq!(unsafe { pdf_page_builder_at(page, 72.0, 720.0, &mut ec) }, 0);
    let content_text = cstring("In-memory round-trip content");
    assert_eq!(unsafe { pdf_page_builder_text(page, content_text.as_ptr(), &mut ec) }, 0);
    assert_eq!(unsafe { pdf_page_builder_done(page, &mut ec) }, 0);

    let mut out_len: usize = 0;
    let bytes_ptr = unsafe { pdf_document_builder_build(builder, &mut out_len, &mut ec) };
    assert_eq!(ec, 0);
    assert!(!bytes_ptr.is_null());
    assert!(out_len > 0, "build produced 0 bytes");

    let bytes = unsafe { std::slice::from_raw_parts(bytes_ptr as *const u8, out_len) };
    assert!(bytes.starts_with(b"%PDF-"), "must be a valid PDF");

    let doc = unsafe { pdf_document_open_from_bytes(bytes.as_ptr(), bytes.len(), &mut ec) };
    assert_eq!(ec, 0, "open_from_bytes failed");
    assert!(!doc.is_null());

    let text_ptr = unsafe { pdf_document_extract_text(doc, 0, &mut ec) };
    assert_eq!(ec, 0);
    let extracted = unsafe { std::ffi::CStr::from_ptr(text_ptr) }
        .to_string_lossy()
        .to_string();
    assert!(
        extracted.contains("In-memory"),
        "extracted text missing 'In-memory': {extracted:.200}"
    );

    unsafe { free_string(text_ptr) };
    unsafe { pdf_document_free(doc) };
    unsafe { free_bytes(bytes_ptr) };
    unsafe { pdf_document_builder_free(builder) };
}
