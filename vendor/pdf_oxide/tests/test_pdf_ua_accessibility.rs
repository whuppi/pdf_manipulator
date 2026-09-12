//! PDF/UA accessibility feature tests (#234).
//!
//! Verifies that `image_with_alt` writes an /Alt entry into the structure tree
//! and that `image_artifact` marks decorative images with /Artifact in the
//! content stream.
#![allow(clippy::missing_safety_doc)]
#![allow(unused_unsafe)]

use pdf_oxide::ffi::*;
use std::ffi::CString;

fn cstring(s: &str) -> CString {
    CString::new(s).unwrap()
}

#[test]
fn image_with_alt_writes_alt_into_structure_tree() {
    let mut ec: i32 = -1;
    let builder = unsafe { pdf_document_builder_create(&mut ec) };
    assert_eq!(ec, 0);

    assert_eq!(
        unsafe { pdf_document_builder_tagged_pdf_ua1(builder, &mut ec) },
        0,
        "tagged_pdf_ua1 failed"
    );
    let lang = cstring("en-US");
    assert_eq!(unsafe { pdf_document_builder_language(builder, lang.as_ptr(), &mut ec) }, 0);

    let page = unsafe { pdf_document_builder_a4_page(builder, &mut ec) };
    assert_eq!(ec, 0);

    assert_eq!(
        unsafe { pdf_page_builder_font(page, cstring("Helvetica").as_ptr(), 12.0, &mut ec) },
        0
    );
    assert_eq!(unsafe { pdf_page_builder_at(page, 72.0, 720.0, &mut ec) }, 0);
    let heading = cstring("Document with accessible image");
    assert_eq!(unsafe { pdf_page_builder_heading(page, 1, heading.as_ptr(), &mut ec) }, 0);

    let jpeg_data =
        std::fs::read("tests/fixtures/adobe_cmyk_10x11_white.jpg").expect("JPEG fixture");
    let alt = cstring("A white JPEG test image");
    let rc = unsafe {
        pdf_page_builder_image_with_alt(
            page,
            jpeg_data.as_ptr(),
            jpeg_data.len(),
            72.0,
            600.0,
            100.0,
            100.0,
            alt.as_ptr(),
            &mut ec,
        )
    };
    assert_eq!(rc, 0, "image_with_alt failed ec={ec}");

    assert_eq!(unsafe { pdf_page_builder_done(page, &mut ec) }, 0);

    let mut out_len: usize = 0;
    let bytes_ptr = unsafe { pdf_document_builder_build(builder, &mut out_len, &mut ec) };
    assert_eq!(ec, 0, "build failed");
    assert!(!bytes_ptr.is_null());

    let bytes = unsafe { std::slice::from_raw_parts(bytes_ptr as *const u8, out_len) };
    let content = String::from_utf8_lossy(bytes);

    assert!(
        content.contains("/Alt"),
        "/Alt not found in PDF output — image alt text was not written to structure tree"
    );
    assert!(content.contains("/MarkInfo"), "missing /MarkInfo");
    assert!(content.contains("/StructTreeRoot"), "missing /StructTreeRoot");

    unsafe { free_bytes(bytes_ptr) };
    unsafe { pdf_document_builder_free(builder) };
}

#[test]
fn image_artifact_marks_decorative_image_as_artifact() {
    let mut ec: i32 = -1;
    let builder = unsafe { pdf_document_builder_create(&mut ec) };
    assert_eq!(ec, 0);
    assert_eq!(unsafe { pdf_document_builder_tagged_pdf_ua1(builder, &mut ec) }, 0);
    let lang = cstring("en-US");
    assert_eq!(unsafe { pdf_document_builder_language(builder, lang.as_ptr(), &mut ec) }, 0);
    let page = unsafe { pdf_document_builder_a4_page(builder, &mut ec) };
    assert_eq!(ec, 0);
    assert_eq!(
        unsafe { pdf_page_builder_font(page, cstring("Helvetica").as_ptr(), 12.0, &mut ec) },
        0
    );

    let jpeg_data =
        std::fs::read("tests/fixtures/adobe_cmyk_10x11_white.jpg").expect("JPEG fixture");
    let rc = unsafe {
        pdf_page_builder_image_artifact(
            page,
            jpeg_data.as_ptr(),
            jpeg_data.len(),
            72.0,
            600.0,
            50.0,
            50.0,
            &mut ec,
        )
    };
    assert_eq!(rc, 0, "image_artifact failed ec={ec}");

    assert_eq!(unsafe { pdf_page_builder_done(page, &mut ec) }, 0);
    let mut out_len: usize = 0;
    let bytes_ptr = unsafe { pdf_document_builder_build(builder, &mut out_len, &mut ec) };
    assert_eq!(ec, 0);
    assert!(!bytes_ptr.is_null());

    let bytes = unsafe { std::slice::from_raw_parts(bytes_ptr as *const u8, out_len) };
    let content = String::from_utf8_lossy(bytes);
    assert!(
        content.contains("/Artifact"),
        "/Artifact not found — decorative image not marked as artifact"
    );

    unsafe { free_bytes(bytes_ptr) };
    unsafe { pdf_document_builder_free(builder) };
}
