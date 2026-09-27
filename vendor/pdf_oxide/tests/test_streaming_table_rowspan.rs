//! StreamingTable rowspan test (#400).
//!
//! Verifies that `begin_v2` with `max_rowspan=2` and `push_row_v2` with
//! non-unit rowspan values produce a valid PDF and that the table content
//! survives a re-open + text extraction round-trip.
#![allow(clippy::missing_safety_doc)]
#![allow(unused_unsafe)]

use pdf_oxide::ffi::*;
use std::ffi::CString;

fn cstring(s: &str) -> CString {
    CString::new(s).unwrap()
}

#[test]
fn streaming_table_rowspan_produces_valid_pdf() {
    let mut ec: i32 = -1;
    let builder = unsafe { pdf_document_builder_create(&mut ec) };
    assert_eq!(ec, 0);
    let page = unsafe { pdf_document_builder_letter_page(builder, &mut ec) };
    assert_eq!(ec, 0);

    assert_eq!(
        unsafe { pdf_page_builder_font(page, cstring("Helvetica").as_ptr(), 10.0, &mut ec) },
        0
    );
    assert_eq!(unsafe { pdf_page_builder_at(page, 72.0, 720.0, &mut ec) }, 0);

    let headers: [CString; 3] = [cstring("Category"), cstring("Item"), cstring("Notes")];
    let header_ptrs: [*const std::os::raw::c_char; 3] = [
        headers[0].as_ptr(),
        headers[1].as_ptr(),
        headers[2].as_ptr(),
    ];
    let widths: [f32; 3] = [100.0, 150.0, 150.0];
    let aligns: [i32; 3] = [0, 0, 0];

    assert_eq!(
        unsafe {
            pdf_page_builder_streaming_table_begin_v2(
                page,
                3,
                header_ptrs.as_ptr(),
                widths.as_ptr(),
                aligns.as_ptr(),
                1,   // repeat_header
                0,   // Fixed mode
                0,   // sample_rows
                0.0, // min_col_width_pt
                0.0, // max_col_width_pt
                2,   // max_rowspan = 2
                &mut ec,
            )
        },
        0,
        "begin_v2 failed ec={ec}"
    );

    // Row 1: first cell spans 2 rows.
    let row1: [CString; 3] = [cstring("Fruits"), cstring("Apple"), cstring("Red")];
    let row1_ptrs: [*const std::os::raw::c_char; 3] =
        [row1[0].as_ptr(), row1[1].as_ptr(), row1[2].as_ptr()];
    let rowspans_1: [usize; 3] = [2, 1, 1];
    assert_eq!(
        unsafe {
            pdf_page_builder_streaming_table_push_row_v2(
                page,
                3,
                row1_ptrs.as_ptr(),
                rowspans_1.as_ptr(),
                &mut ec,
            )
        },
        0,
        "push_row_v2 row1 failed ec={ec}"
    );

    // Row 2: first cell is the rowspan continuation.
    let row2: [CString; 3] = [cstring(""), cstring("Banana"), cstring("Yellow")];
    let row2_ptrs: [*const std::os::raw::c_char; 3] =
        [row2[0].as_ptr(), row2[1].as_ptr(), row2[2].as_ptr()];
    let rowspans_2: [usize; 3] = [1, 1, 1];
    assert_eq!(
        unsafe {
            pdf_page_builder_streaming_table_push_row_v2(
                page,
                3,
                row2_ptrs.as_ptr(),
                rowspans_2.as_ptr(),
                &mut ec,
            )
        },
        0,
        "push_row_v2 row2 failed ec={ec}"
    );

    // Row 3: normal row.
    let row3: [CString; 3] = [cstring("Vegetables"), cstring("Carrot"), cstring("Orange")];
    let row3_ptrs: [*const std::os::raw::c_char; 3] =
        [row3[0].as_ptr(), row3[1].as_ptr(), row3[2].as_ptr()];
    let rowspans_3: [usize; 3] = [1, 1, 1];
    assert_eq!(
        unsafe {
            pdf_page_builder_streaming_table_push_row_v2(
                page,
                3,
                row3_ptrs.as_ptr(),
                rowspans_3.as_ptr(),
                &mut ec,
            )
        },
        0,
        "push_row_v2 row3 failed ec={ec}"
    );

    assert_eq!(unsafe { pdf_page_builder_streaming_table_finish(page, &mut ec) }, 0);
    assert_eq!(unsafe { pdf_page_builder_done(page, &mut ec) }, 0);

    let mut out_len: usize = 0;
    let bytes_ptr = unsafe { pdf_document_builder_build(builder, &mut out_len, &mut ec) };
    assert_eq!(ec, 0, "build failed");
    assert!(!bytes_ptr.is_null());

    let bytes = unsafe { std::slice::from_raw_parts(bytes_ptr as *const u8, out_len) };
    assert!(bytes.starts_with(b"%PDF-"), "output must be a valid PDF");

    let doc_handle = unsafe { pdf_document_open_from_bytes(bytes.as_ptr(), bytes.len(), &mut ec) };
    assert_eq!(ec, 0, "re-open failed");
    let text_ptr = unsafe { pdf_document_extract_text(doc_handle, 0, &mut ec) };
    assert_eq!(ec, 0, "extract_text failed");

    let extracted = unsafe { std::ffi::CStr::from_ptr(text_ptr) }
        .to_string_lossy()
        .to_string();

    assert!(
        extracted.contains("Fruits") || extracted.contains("Apple"),
        "table content 'Fruits/Apple' not found in extracted text: {extracted:.200}"
    );
    assert!(
        extracted.contains("Carrot"),
        "table content 'Carrot' not found in extracted text"
    );

    unsafe { free_string(text_ptr) };
    unsafe { pdf_document_free(doc_handle) };
    unsafe { free_bytes(bytes_ptr) };
    unsafe { pdf_document_builder_free(builder) };
}
