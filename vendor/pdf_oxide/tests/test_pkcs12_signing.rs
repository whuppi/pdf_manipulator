//! PKCS#12 sign and verify round-trip test (#208 / #411).
//!
//! Loads `tests/fixtures/test_signing.p12`, builds a minimal PDF, signs it
//! with `pdf_sign_bytes`, then verifies that the output contains the expected
//! PDF digital-signature markers.
#![cfg(feature = "signatures")]
#![allow(clippy::missing_safety_doc)]
#![allow(unused_unsafe)]

use pdf_oxide::ffi::*;
use std::ffi::CString;

fn cstring(s: &str) -> CString {
    CString::new(s).unwrap()
}

#[test]
fn pkcs12_sign_pdf_bytes_round_trip() {
    let p12_data =
        std::fs::read("tests/fixtures/test_signing.p12").expect("test_signing.p12 must exist");
    let password = cstring("testpass");
    let mut ec: i32 = -1;

    let cert_handle = unsafe {
        pdf_certificate_load_from_bytes(
            p12_data.as_ptr() as *const _,
            p12_data.len() as i32,
            password.as_ptr(),
            &mut ec,
        )
    };
    assert_eq!(ec, 0, "pdf_certificate_load_from_bytes returned error {ec}");
    assert!(!cert_handle.is_null(), "certificate handle must not be null");

    let builder = unsafe { pdf_document_builder_create(&mut ec) };
    assert_eq!(ec, 0);
    let page = unsafe { pdf_document_builder_letter_page(builder, &mut ec) };
    assert_eq!(ec, 0);
    let text = cstring("Signed document");
    assert_eq!(unsafe { pdf_page_builder_at(page, 72.0, 720.0, &mut ec) }, 0);
    assert_eq!(
        unsafe { pdf_page_builder_font(page, cstring("Helvetica").as_ptr(), 12.0, &mut ec) },
        0
    );
    assert_eq!(unsafe { pdf_page_builder_text(page, text.as_ptr(), &mut ec) }, 0);
    assert_eq!(unsafe { pdf_page_builder_done(page, &mut ec) }, 0);
    let mut pdf_len: usize = 0;
    let pdf_ptr = unsafe { pdf_document_builder_build(builder, &mut pdf_len, &mut ec) };
    assert_eq!(ec, 0);
    assert!(!pdf_ptr.is_null());
    unsafe { pdf_document_builder_free(builder) };

    let reason = cstring("Approved");
    let location = cstring("Test Suite");
    let mut signed_len: usize = 0;
    let signed_ptr = unsafe {
        pdf_sign_bytes(
            pdf_ptr,
            pdf_len,
            cert_handle,
            reason.as_ptr(),
            location.as_ptr(),
            &mut signed_len,
            &mut ec,
        )
    };
    unsafe { free_bytes(pdf_ptr as *mut _) };

    assert_eq!(ec, 0, "pdf_sign_bytes returned error {ec}");
    assert!(!signed_ptr.is_null(), "signed PDF must not be null");
    assert!(signed_len > pdf_len, "signed PDF must be larger than unsigned");

    let signed_bytes = unsafe { std::slice::from_raw_parts(signed_ptr, signed_len) };
    assert!(signed_bytes.starts_with(b"%PDF-"), "output must be a PDF");

    let content = String::from_utf8_lossy(signed_bytes);
    assert!(
        content.contains("/Sig") || content.contains("/ByteRange"),
        "signed PDF must contain /Sig or /ByteRange"
    );

    unsafe { free_bytes(signed_ptr as *mut _) };
    unsafe { pdf_certificate_free(cert_handle) };
}
