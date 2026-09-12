//! Regression test for recursive XObject processing (previously caused aliasing UB).
//!
//! Before the fix, `TextExtractor` stored a `*mut PdfDocument` raw pointer and dereferenced
//! it as `&mut` inside `process_xobject`. Nested Form XObjects created overlapping `&mut`
//! references — undefined behavior under Rust's aliasing rules (confirmed by Miri).
//!
//! The fix uses interior mutability (`RefCell`/`Cell`) on cache fields so that
//! `TextExtractor` holds a `*const PdfDocument` and dereferences as `&`, eliminating
//! the aliased `&mut` references entirely.
//!
//! This test constructs PDFs with deeply nested Form XObjects to exercise the recursive path
//! and ensure no regressions.

use pdf_oxide::document::PdfDocument;

/// Build a minimal PDF with a chain of nested Form XObjects.
///
/// Structure: Page content → Form1 → Form2 → Form3 → Form4
/// Each Form XObject has its own /Resources (with /Font and /XObject),
/// which forces `process_xobject` to save/restore fonts and recurse —
/// creating stacked `&mut PdfDocument` references from the same raw pointer.
fn build_nested_xobject_pdf(depth: usize) -> Vec<u8> {
    // Object layout:
    //   1: Catalog
    //   2: Pages
    //   3: Page
    //   4: Page content stream
    //   5..5+depth-1: Form XObjects (chained)
    //   5+depth: Font
    let font_obj_id = 5 + depth;

    let mut pdf = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    // Header
    pdf.extend_from_slice(b"%PDF-1.4\n");

    // Object 1: Catalog
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Object 2: Pages
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // Object 3: Page
    offsets.push(pdf.len());
    let page = format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]\n\
         /Contents 4 0 R\n\
         /Resources << /Font << /F1 {} 0 R >> /XObject << /Form1 5 0 R >> >>\n\
         >>\nendobj\n",
        font_obj_id
    );
    pdf.extend_from_slice(page.as_bytes());

    // Object 4: Page content stream
    let content = b"BT /F1 12 Tf 100 700 Td (Page level text) Tj ET\n/Form1 Do";
    offsets.push(pdf.len());
    let header = format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len());
    pdf.extend_from_slice(header.as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    // Objects 5..5+depth-1: Chained Form XObjects
    for i in 0..depth {
        let obj_id = 5 + i;
        let form_name = format!("Form{}", i + 1);
        let next_form_name = format!("Form{}", i + 2);
        let next_obj_id = 5 + i + 1;
        let y_pos = 600 - (i * 50);

        let stream_content = if i + 1 < depth {
            // Intermediate: render text + invoke next Form XObject
            format!(
                "BT /F1 12 Tf 100 {} Td ({} text here) Tj ET\n/{} Do",
                y_pos, form_name, next_form_name
            )
        } else {
            // Leaf: just render text
            format!("BT /F1 12 Tf 100 {} Td ({} leaf text) Tj ET", y_pos, form_name)
        };

        let resources = if i + 1 < depth {
            // Has own /Font and /XObject → triggers font save/restore in process_xobject
            format!(
                "/Resources << /Font << /F1 {} 0 R >> /XObject << /{} {} 0 R >> >>",
                font_obj_id, next_form_name, next_obj_id
            )
        } else {
            // Leaf: only /Font
            format!("/Resources << /Font << /F1 {} 0 R >> >>", font_obj_id)
        };

        offsets.push(pdf.len());
        let obj = format!(
            "{} 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 612 792]\n\
             {}\n\
             /Length {} >>\nstream\n",
            obj_id,
            resources,
            stream_content.len()
        );
        pdf.extend_from_slice(obj.as_bytes());
        pdf.extend_from_slice(stream_content.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Font object
    offsets.push(pdf.len());
    let font = format!(
        "{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        font_obj_id
    );
    pdf.extend_from_slice(font.as_bytes());

    // Cross-reference table
    let xref_offset = pdf.len();
    let total_objects = offsets.len() + 1; // +1 for object 0
    pdf.extend_from_slice(b"xref\n");
    pdf.extend_from_slice(format!("0 {}\n", total_objects).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }

    // Trailer
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            total_objects, xref_offset
        )
        .as_bytes(),
    );

    pdf
}

/// Exercises the recursive process_xobject path with 4 levels of nesting.
///
/// Each nesting level creates `unsafe { &mut *self.document }` while the
/// previous level's `&mut PdfDocument` is still live on the call stack.
/// This is undefined behavior that can cause segfaults under optimization.
///
/// Run with `cargo test --release test_nested_xobject` to increase the
/// chance of the UB manifesting as a crash.
#[test]
fn test_nested_xobject_extraction_does_not_segfault() {
    let _ = env_logger::builder().is_test(true).try_init();
    let pdf_bytes = build_nested_xobject_pdf(4);
    let doc = PdfDocument::from_bytes(pdf_bytes).expect("Failed to parse nested XObject PDF");

    let options = pdf_oxide::converters::ConversionOptions {
        extract_tables: false,
        ..Default::default()
    };

    let text = doc
        .extract_text_with_options(0, &options)
        .expect("Extraction failed (possible segfault from aliased &mut)");

    assert!(text.contains("Page level text"), "Missing page-level text, got: '{}'", text);
}

/// Stress test: run extraction repeatedly to increase the probability of
/// UB-induced miscompilation manifesting as a crash.
#[test]
fn test_nested_xobject_stress() {
    let pdf_bytes = build_nested_xobject_pdf(5);
    let options = pdf_oxide::converters::ConversionOptions {
        extract_tables: false,
        ..Default::default()
    };

    for iteration in 0..50 {
        let doc =
            PdfDocument::from_bytes(pdf_bytes.clone()).expect("Failed to parse nested XObject PDF");

        let result = doc.extract_text_with_options(0, &options);
        assert!(result.is_ok(), "Iteration {}: {:?}", iteration, result.err());
    }
}

/// Deep nesting (8 levels) to maximize stacked aliased &mut references.
#[test]
fn test_deeply_nested_xobject_extraction() {
    let pdf_bytes = build_nested_xobject_pdf(8);
    let doc = PdfDocument::from_bytes(pdf_bytes).expect("Failed to parse deep XObject PDF");
    let options = pdf_oxide::converters::ConversionOptions {
        extract_tables: false,
        ..Default::default()
    };

    let text = doc
        .extract_text_with_options(0, &options)
        .expect("Extraction failed on deeply nested XObject PDF");

    assert!(text.contains("Page level text"), "Missing page-level text, got: '{}'", text);
}
