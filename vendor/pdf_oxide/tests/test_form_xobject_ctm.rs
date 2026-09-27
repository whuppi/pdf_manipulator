//! Tests for CTM-aware Form XObject span extraction.
//!
//! The same Form XObject may be painted multiple times on a page (or across
//! pages) with different caller CTMs, producing text at different positions.
//! The extractor must re-compute spans for each unique (XObject, CTM) pair
//! rather than returning a stale cached result from the first invocation.
//!
//! Regression for: nougat_005.pdf plain F1 = 0.333 (Issue B1)
//! Fix: `processed_xobjects` and `xobject_spans_cache` keyed by
//! `(ObjectRef, ctm_millipoints)` instead of `ObjectRef` alone.

use pdf_oxide::document::PdfDocument;

// ---------------------------------------------------------------------------
// Helper: build a PDF with a Form XObject invoked twice, each time inside a
// save/restore block that applies a different y-translation via the `cm`
// operator.
//
// Page content stream:
//   q  1 0 0 1 0   0 cm  /Fm0 Do  Q   ← paints Form at y+0
//   q  1 0 0 1 0 400 cm  /Fm0 Do  Q   ← paints Form at y+400
//
// The Form XObject contains a single text string "FORM_TEXT" at (50, 50).
// After the first `cm`:  page-space y ≈  50
// After the second `cm`: page-space y ≈ 450
// ---------------------------------------------------------------------------
fn build_form_xobject_two_ctm_pdf() -> Vec<u8> {
    let mut pdf = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    pdf.extend_from_slice(b"%PDF-1.4\n");

    // Object 1: Catalog
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

    // Object 2: Pages
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

    // Object 3: Page — references the content stream and declares fonts/XObjects
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"3 0 obj\n\
          << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]\n\
             /Contents 4 0 R\n\
             /Resources << /Font << /F1 6 0 R >> /XObject << /Fm0 5 0 R >> >>\n\
          >>\nendobj\n\n",
    );

    // Object 4: Page content stream
    // Two `q … cm … Do … Q` blocks with different y-translations.
    let page_content = b"q 1 0 0 1 0 0 cm /Fm0 Do Q q 1 0 0 1 0 400 cm /Fm0 Do Q";
    offsets.push(pdf.len());
    let hdr = format!("4 0 obj\n<< /Length {} >>\nstream\n", page_content.len());
    pdf.extend_from_slice(hdr.as_bytes());
    pdf.extend_from_slice(page_content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

    // Object 5: Form XObject — text "FORM_TEXT" at (50, 50) in XObject space
    let form_stream = b"BT /F1 12 Tf 50 50 Td (FORM_TEXT) Tj ET";
    offsets.push(pdf.len());
    let form_hdr = format!(
        "5 0 obj\n\
         << /Type /XObject /Subtype /Form /BBox [0 0 612 792]\n\
            /Resources << /Font << /F1 6 0 R >> >>\n\
            /Length {} >>\nstream\n",
        form_stream.len()
    );
    pdf.extend_from_slice(form_hdr.as_bytes());
    pdf.extend_from_slice(form_stream);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

    // Object 6: Font (Helvetica — built-in, no encoding needed for ASCII)
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"6 0 obj\n\
          << /Type /Font /Subtype /Type1 /BaseFont /Helvetica\n\
             /Encoding /WinAnsiEncoding >>\nendobj\n\n",
    );

    // Cross-reference table
    let xref_offset = pdf.len();
    let n_obj = offsets.len() + 1; // +1 for the free entry 0
    let mut xref = format!("xref\n0 {}\n", n_obj);
    xref.push_str("0000000000 65535 f \n");
    for off in &offsets {
        xref.push_str(&format!("{:010} 00000 n \n", off));
    }
    pdf.extend_from_slice(xref.as_bytes());

    // Trailer
    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        n_obj, xref_offset
    );
    pdf.extend_from_slice(trailer.as_bytes());

    pdf
}

// ---------------------------------------------------------------------------
// Same as above but the Form XObject is used across two pages.  Each page
// applies a different CTM before `Do`.
// ---------------------------------------------------------------------------
fn build_form_xobject_two_pages_pdf() -> Vec<u8> {
    let mut pdf = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    pdf.extend_from_slice(b"%PDF-1.4\n");

    // Object 1: Catalog
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

    // Object 2: Pages (two pages)
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n\n");

    // Object 3: Page 1 — paints Form at y-offset 0 (identity cm)
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"3 0 obj\n\
          << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]\n\
             /Contents 5 0 R\n\
             /Resources << /Font << /F1 8 0 R >> /XObject << /Fm0 7 0 R >> >>\n\
          >>\nendobj\n\n",
    );

    // Object 4: Page 2 — paints Form at y-offset 300
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"4 0 obj\n\
          << /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]\n\
             /Contents 6 0 R\n\
             /Resources << /Font << /F1 8 0 R >> /XObject << /Fm0 7 0 R >> >>\n\
          >>\nendobj\n\n",
    );

    // Object 5: Content for page 1 (identity CTM)
    let content1 = b"q 1 0 0 1 0 0 cm /Fm0 Do Q";
    offsets.push(pdf.len());
    let hdr5 = format!("5 0 obj\n<< /Length {} >>\nstream\n", content1.len());
    pdf.extend_from_slice(hdr5.as_bytes());
    pdf.extend_from_slice(content1);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

    // Object 6: Content for page 2 (y-translation 300)
    let content2 = b"q 1 0 0 1 0 300 cm /Fm0 Do Q";
    offsets.push(pdf.len());
    let hdr6 = format!("6 0 obj\n<< /Length {} >>\nstream\n", content2.len());
    pdf.extend_from_slice(hdr6.as_bytes());
    pdf.extend_from_slice(content2);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

    // Object 7: Form XObject — "PAGE_TEXT" at (30, 100) in XObject space
    let form_stream = b"BT /F1 12 Tf 30 100 Td (PAGE_TEXT) Tj ET";
    offsets.push(pdf.len());
    let form_hdr = format!(
        "7 0 obj\n\
         << /Type /XObject /Subtype /Form /BBox [0 0 612 792]\n\
            /Resources << /Font << /F1 8 0 R >> >>\n\
            /Length {} >>\nstream\n",
        form_stream.len()
    );
    pdf.extend_from_slice(form_hdr.as_bytes());
    pdf.extend_from_slice(form_stream);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

    // Object 8: Font
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"8 0 obj\n\
          << /Type /Font /Subtype /Type1 /BaseFont /Helvetica\n\
             /Encoding /WinAnsiEncoding >>\nendobj\n\n",
    );

    // Cross-reference table
    let xref_offset = pdf.len();
    let n_obj = offsets.len() + 1;
    let mut xref = format!("xref\n0 {}\n", n_obj);
    xref.push_str("0000000000 65535 f \n");
    for off in &offsets {
        xref.push_str(&format!("{:010} 00000 n \n", off));
    }
    pdf.extend_from_slice(xref.as_bytes());

    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        n_obj, xref_offset
    );
    pdf.extend_from_slice(trailer.as_bytes());

    pdf
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify that when the same Form XObject is painted twice on a single page
/// with different y-translation CTMs, both invocations produce spans at
/// distinct y-coordinates.
///
/// Before the fix, `processed_xobjects` (keyed only by ObjectRef) would
/// block the second `Do` call, yielding only one span instead of two.
#[test]
fn test_same_form_xobject_twice_different_ctm_same_page() {
    let _ = env_logger::builder().is_test(true).try_init();
    let pdf = build_form_xobject_two_ctm_pdf();
    let doc = PdfDocument::from_bytes(pdf).expect("parse PDF");

    let spans = doc.extract_spans(0).expect("extract spans");

    // Collect spans that contain "FORM_TEXT"
    let matching: Vec<_> = spans
        .iter()
        .filter(|s| s.text.contains("FORM_TEXT"))
        .collect();

    assert!(
        matching.len() >= 2,
        "Expected at least 2 'FORM_TEXT' spans (one per CTM invocation), \
         got {}: {:?}",
        matching.len(),
        matching
            .iter()
            .map(|s| (s.text.as_str(), s.bbox.y))
            .collect::<Vec<_>>()
    );

    // The two spans must be at different y positions — the second invocation
    // adds 400 units of y-translation, so they should differ by ~400.
    let y_vals: Vec<f32> = matching.iter().map(|s| s.bbox.y).collect();
    let y_min = y_vals.iter().cloned().fold(f32::INFINITY, f32::min);
    let y_max = y_vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let y_diff = (y_max - y_min).abs();

    assert!(
        y_diff > 300.0,
        "Y-coordinates of the two spans should differ by ~400 (got diff = {}). \
         Positions: {:?}",
        y_diff,
        y_vals
    );
}

/// Verify that the same Form XObject used on two different pages with
/// different CTMs yields correct (distinct) y-coordinates on each page.
///
/// Before the fix, the `xobject_spans_cache` keyed only by ObjectRef would
/// return page-1's coordinates for page 2 if the cache was warmed on page 1.
#[test]
fn test_same_form_xobject_two_pages_different_ctm() {
    let _ = env_logger::builder().is_test(true).try_init();
    let pdf = build_form_xobject_two_pages_pdf();
    let doc = PdfDocument::from_bytes(pdf).expect("parse PDF");

    // Extract from page 0 first (warms the cache with identity CTM)
    let spans0 = doc.extract_spans(0).expect("extract page 0 spans");
    // Then page 1 (should use a fresh extraction with y+300 CTM, not the cached result)
    let spans1 = doc.extract_spans(1).expect("extract page 1 spans");

    let find_text_span = |spans: &[pdf_oxide::layout::TextSpan]| {
        spans.iter().find(|s| s.text.contains("PAGE_TEXT")).cloned()
    };

    let span0 = find_text_span(&spans0);
    let span1 = find_text_span(&spans1);

    assert!(
        span0.is_some(),
        "Page 0 should contain 'PAGE_TEXT', got: {:?}",
        spans0.iter().map(|s| s.text.as_str()).collect::<Vec<_>>()
    );
    assert!(
        span1.is_some(),
        "Page 1 should contain 'PAGE_TEXT', got: {:?}",
        spans1.iter().map(|s| s.text.as_str()).collect::<Vec<_>>()
    );

    let y0 = span0.unwrap().bbox.y;
    let y1 = span1.unwrap().bbox.y;

    // Page 1 applies a y-translation of 300, so its span y should be ~300 higher
    // than page 0's span y (which uses identity CTM).
    // The Form places text at y=100; after +300 translation y ≈ 400.
    let diff = (y1 - y0).abs();
    assert!(
        diff > 200.0,
        "Page 1's 'PAGE_TEXT' y ({}) should be ~300 above page 0's y ({}); diff = {}",
        y1,
        y0,
        diff
    );
}
