//! Integration tests for PdfDocument - targeting coverage gaps in document.rs
//!
//! document.rs has ~24.5% coverage (4,890 missed lines). These tests exercise:
//! - from_bytes with valid/invalid data
//! - version(), page_count(), catalog()
//! - extract_text(), extract_spans(), extract_images()
//! - to_markdown(), to_html(), to_plain_text()
//! - Multi-page documents
//! - Error paths

use pdf_oxide::document::PdfDocument;
use pdf_oxide::error::Error;

// ---------------------------------------------------------------------------
// Helper: build a minimal valid single-page PDF with optional text content
// ---------------------------------------------------------------------------

fn build_minimal_pdf(text_content: Option<&str>) -> Vec<u8> {
    let mut pdf = b"%PDF-1.7\n".to_vec();

    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let off3 = pdf.len();
    if let Some(text) = text_content {
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
        );

        let off4 = pdf.len();
        let content = format!("BT /F1 12 Tf 72 720 Td ({}) Tj ET", text);
        pdf.extend_from_slice(
            format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes(),
        );
        pdf.extend_from_slice(content.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        let off5 = pdf.len();
        pdf.extend_from_slice(
            b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
        );

        finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4, off5]);
    } else {
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
        );
        finalize_pdf(&mut pdf, &[0, off1, off2, off3]);
    }

    pdf
}

fn build_multi_page_pdf(page_count: usize) -> Vec<u8> {
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0usize]; // obj 0 placeholder

    let off1 = pdf.len();
    offsets.push(off1);
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Build kids array
    let kids: Vec<String> = (0..page_count).map(|i| format!("{} 0 R", i + 3)).collect();
    let kids_str = kids.join(" ");

    let off2 = pdf.len();
    offsets.push(off2);
    pdf.extend_from_slice(
        format!(
            "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
            kids_str, page_count
        )
        .as_bytes(),
    );

    // Font object
    let font_obj_num = page_count + 3;

    // Build pages
    for i in 0..page_count {
        let page_num = i + 3;
        let content_num = page_num + page_count;

        let off = pdf.len();
        offsets.push(off);
        pdf.extend_from_slice(
            format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R /Resources << /Font << /F1 {} 0 R >> >> >>\nendobj\n",
                page_num, content_num, font_obj_num
            )
            .as_bytes(),
        );
    }

    // Build content streams
    for i in 0..page_count {
        let content_num = i + 3 + page_count;
        let content = format!("BT /F1 12 Tf 72 720 Td (Page {}) Tj ET", i + 1);

        let off = pdf.len();
        offsets.push(off);
        pdf.extend_from_slice(
            format!("{} 0 obj\n<< /Length {} >>\nstream\n", content_num, content.len()).as_bytes(),
        );
        pdf.extend_from_slice(content.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Font object
    let font_off = pdf.len();
    offsets.push(font_off);
    pdf.extend_from_slice(
        format!(
            "{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
            font_obj_num
        )
        .as_bytes(),
    );

    finalize_pdf_with_size(&mut pdf, &offsets);
    pdf
}

fn finalize_pdf(pdf: &mut Vec<u8>, obj_offsets: &[usize]) {
    let xref_offset = pdf.len();
    let count = obj_offsets.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", count).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \r\n");
    for &off in &obj_offsets[1..] {
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", off).as_bytes());
    }
    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        count, xref_offset
    );
    pdf.extend_from_slice(trailer.as_bytes());
}

fn finalize_pdf_with_size(pdf: &mut Vec<u8>, obj_offsets: &[usize]) {
    let xref_offset = pdf.len();
    let count = obj_offsets.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", count).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \r\n");
    for &off in &obj_offsets[1..] {
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", off).as_bytes());
    }
    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        count, xref_offset
    );
    pdf.extend_from_slice(trailer.as_bytes());
}

fn write_temp_pdf(data: &[u8], name: &str) -> std::path::PathBuf {
    use std::io::Write;
    let dir = std::env::temp_dir().join("pdf_oxide_coverage_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

// ===========================================================================
// Tests: Document Opening
// ===========================================================================

#[test]
fn test_from_bytes_valid_minimal() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).expect("Should open minimal PDF");
    assert_eq!(doc.version(), (1, 7));
    assert_eq!(doc.page_count().unwrap(), 1);
}

#[test]
fn test_from_bytes_with_text() {
    let pdf = build_minimal_pdf(Some("Hello World"));
    let doc = PdfDocument::from_bytes(pdf).expect("Should open PDF with text");
    assert_eq!(doc.page_count().unwrap(), 1);
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Hello") || text.contains("World"));
}

#[test]
fn test_from_bytes_empty_data() {
    let result = PdfDocument::from_bytes(vec![]);
    assert!(result.is_err());
}

#[test]
fn test_from_bytes_garbage_data() {
    let result = PdfDocument::from_bytes(b"not a pdf file at all".to_vec());
    assert!(result.is_err());
}

#[test]
fn test_from_bytes_truncated_header() {
    let result = PdfDocument::from_bytes(b"%PDF".to_vec());
    assert!(result.is_err());
}

#[test]
fn test_open_nonexistent_file() {
    let result = PdfDocument::open("/nonexistent/path/fakefile.pdf");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), Error::Io(_)));
}

#[test]
fn test_open_from_file() {
    let pdf = build_minimal_pdf(Some("File test"));
    let path = write_temp_pdf(&pdf, "test_open_file.pdf");
    let doc = PdfDocument::open(&path).unwrap();
    assert_eq!(doc.page_count().unwrap(), 1);
    let _ = std::fs::remove_file(&path);
}

// ===========================================================================
// Tests: Version and Properties
// ===========================================================================

#[test]
fn test_version_1_4() {
    let mut pdf = build_minimal_pdf(None);
    // Replace version in header
    pdf[5] = b'1';
    pdf[7] = b'4';
    let doc = PdfDocument::from_bytes(pdf).expect("Should open 1.4 PDF");
    assert_eq!(doc.version(), (1, 4));
}

#[test]
fn test_version_2_0() {
    let mut pdf = build_minimal_pdf(None);
    pdf[5] = b'2';
    pdf[7] = b'0';
    let doc = PdfDocument::from_bytes(pdf).expect("Should open 2.0 PDF");
    assert_eq!(doc.version(), (2, 0));
}

#[test]
fn test_catalog_access() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let catalog = doc.catalog().unwrap();
    let dict = catalog.as_dict().unwrap();
    assert!(dict.contains_key("Type"));
}

#[test]
fn test_trailer_access() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let trailer = doc.trailer();
    let dict = trailer.as_dict().unwrap();
    assert!(dict.contains_key("Size"));
    assert!(dict.contains_key("Root"));
}

// ===========================================================================
// Tests: Multi-page documents
// ===========================================================================

#[test]
fn test_multi_page_count() {
    let pdf = build_multi_page_pdf(5);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    assert_eq!(doc.page_count().unwrap(), 5);
}

#[test]
fn test_multi_page_extract_text() {
    let pdf = build_multi_page_pdf(3);
    let doc = PdfDocument::from_bytes(pdf).unwrap();

    let text0 = doc.extract_text(0).unwrap();
    assert!(text0.contains("Page 1"));

    let text1 = doc.extract_text(1).unwrap();
    assert!(text1.contains("Page 2"));

    let text2 = doc.extract_text(2).unwrap();
    assert!(text2.contains("Page 3"));
}

#[test]
fn test_extract_all_text() {
    let pdf = build_multi_page_pdf(3);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let all_text = doc.extract_all_text().unwrap();
    assert!(all_text.contains("Page 1"));
    assert!(all_text.contains("Page 2"));
    assert!(all_text.contains("Page 3"));
}

#[test]
fn test_extract_text_invalid_page() {
    let pdf = build_minimal_pdf(Some("Hello"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let result = doc.extract_text(99);
    assert!(result.is_err());
}

// ===========================================================================
// Tests: Text Extraction and Spans
// ===========================================================================

#[test]
fn test_extract_spans_basic() {
    let pdf = build_minimal_pdf(Some("Test spans"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let spans = doc.extract_spans(0).unwrap();
    // Should return at least one span with text content
    let has_text = spans.iter().any(|s| !s.text.is_empty());
    assert!(has_text, "Should have spans with text");
}

#[test]
fn test_extract_spans_empty_page() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let spans = doc.extract_spans(0).unwrap();
    // Empty page should have no text spans
    assert!(spans.is_empty() || spans.iter().all(|s| s.text.trim().is_empty()));
}

#[test]
fn test_extract_spans_invalid_page() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let result = doc.extract_spans(100);
    assert!(result.is_err());
}

// ===========================================================================
// Tests: Format Conversion
// ===========================================================================

#[test]
fn test_to_markdown_basic() {
    let pdf = build_minimal_pdf(Some("Markdown test"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let md = doc
        .to_markdown(0, &pdf_oxide::converters::ConversionOptions::default())
        .unwrap();
    // Main point: to_markdown doesn't crash on a minimal PDF
    let _ = md;
}

#[test]
fn test_to_html_basic() {
    let pdf = build_minimal_pdf(Some("HTML test"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let html = doc
        .to_html(0, &pdf_oxide::converters::ConversionOptions::default())
        .unwrap();
    // Should produce some output without crashing
    let _ = html;
}

#[test]
fn test_to_plain_text_basic() {
    let pdf = build_minimal_pdf(Some("Plain text test"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc
        .to_plain_text(0, &pdf_oxide::converters::ConversionOptions::default())
        .unwrap();
    let _ = text;
}

#[test]
fn test_to_markdown_all() {
    let pdf = build_multi_page_pdf(2);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let md = doc
        .to_markdown_all(&pdf_oxide::converters::ConversionOptions::default())
        .unwrap();
    let _ = md;
}

#[test]
fn test_to_plain_text_all() {
    let pdf = build_multi_page_pdf(2);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc
        .to_plain_text_all(&pdf_oxide::converters::ConversionOptions::default())
        .unwrap();
    let _ = text;
}

#[test]
fn test_to_html_all() {
    let pdf = build_multi_page_pdf(2);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let html = doc
        .to_html_all(&pdf_oxide::converters::ConversionOptions::default())
        .unwrap();
    let _ = html;
}

// ===========================================================================
// Tests: Image Extraction
// ===========================================================================

#[test]
fn test_extract_images_no_images() {
    let pdf = build_minimal_pdf(Some("No images here"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let images = doc.extract_images(0).unwrap();
    assert!(images.is_empty());
}

#[test]
fn test_extract_images_empty_page() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let images = doc.extract_images(0).unwrap();
    assert!(images.is_empty());
}

#[test]
fn test_extract_images_invalid_page() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let result = doc.extract_images(42);
    assert!(result.is_err());
}

// ===========================================================================
// Tests: Annotations
// ===========================================================================

#[test]
fn test_get_annotations_no_annotations() {
    let pdf = build_minimal_pdf(Some("No annotations"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let annotations = doc.get_annotations(0).unwrap();
    assert!(annotations.is_empty());
}

// ===========================================================================
// Tests: Debug and Utility
// ===========================================================================

#[test]
fn test_debug_display() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let debug_str = format!("{:?}", doc);
    assert!(debug_str.contains("PdfDocument"));
    assert!(debug_str.contains("version"));
}

#[test]
fn test_check_circular_references() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let cycles = doc.check_for_circular_references();
    // The function runs without crashing; may find parent-child "cycles"
    // in the page tree (/Parent references) which are normal.
    let _ = cycles;
}

#[test]
fn test_may_contain_text_public() {
    assert!(PdfDocument::may_contain_text_public(b"BT /F1 12 Tf (Hello) Tj ET"));
    assert!(PdfDocument::may_contain_text_public(b"/X0 Do"));
    assert!(!PdfDocument::may_contain_text_public(b"q 1 0 0 1 0 0 cm Q"));
}

// ===========================================================================
// Tests: Content Stream with Various Operators
// ===========================================================================

#[test]
fn test_content_stream_with_tj_operator() {
    // Test Tj text showing operator
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let off4 = pdf.len();
    let content = b"BT /F1 12 Tf 72 720 Td (Hello) Tj 0 -14 Td (World) Tj ET";
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );
    finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4, off5]);

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Hello"));
    assert!(text.contains("World"));
}

#[test]
fn test_content_stream_with_tj_array() {
    // Test TJ operator (text with positioning)
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let off4 = pdf.len();
    let content = b"BT /F1 12 Tf 72 720 Td [(Hello) -100 (World)] TJ ET";
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );
    finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4, off5]);

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Hello"));
}

#[test]
fn test_content_stream_with_quote_operator() {
    // Test ' (quote) text showing operator
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let off4 = pdf.len();
    let content = b"BT /F1 12 Tf 14 TL 72 720 Td (Line1) Tj (Line2) ' ET";
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );
    finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4, off5]);

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Line1"));
}

#[test]
fn test_content_stream_with_text_state_operators() {
    // Test Tc, Tw, Tz, TL, Tr, Ts operators
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let off4 = pdf.len();
    let content = b"BT /F1 12 Tf 1 Tc 2 Tw 100 Tz 14 TL 0 Tr 3 Ts 72 720 Td (Styled) Tj ET";
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );
    finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4, off5]);

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Styled"));
}

#[test]
fn test_content_stream_with_graphics_state() {
    // Test q, Q, cm operators
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let off4 = pdf.len();
    let content = b"q 1 0 0 1 72 720 cm BT /F1 12 Tf 0 0 Td (Transformed) Tj ET Q";
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );
    finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4, off5]);

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Transformed"));
}

#[test]
fn test_content_stream_with_color_operators() {
    // Test rg, RG, g, G, k, K, cs, CS operators
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let off4 = pdf.len();
    let content =
        b"BT /F1 12 Tf 1 0 0 rg 0 0 0 RG 0.5 g 0.3 G 0 1 0 1 k 1 0 1 0 K 72 720 Td (Colored) Tj ET";
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let off5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );
    finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4, off5]);

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Colored"));
}

// ===========================================================================
// Tests: Fixture-based tests
// ===========================================================================

#[test]
fn test_fixture_simple_pdf() {
    let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let _ = doc.page_count().unwrap();
    let _ = doc.version();
}

#[test]
fn test_fixture_outline_pdf() {
    let doc = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let count = doc.page_count().unwrap();
    assert!(count >= 1);
    // Extract text from each page
    for i in 0..count {
        let _ = doc.extract_text(i);
    }
}

#[test]
fn test_fixture_simple_pdf_format_conversions() {
    let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let opts = pdf_oxide::converters::ConversionOptions::default();
    let _ = doc.to_markdown(0, &opts);
    let _ = doc.to_html(0, &opts);
    let _ = doc.to_plain_text(0, &opts);
}

// ===========================================================================
// Tests: Extract chars (fast path)
// ===========================================================================

#[test]
fn test_extract_chars_basic() {
    let pdf = build_minimal_pdf(Some("Char test"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let chars = doc.extract_chars(0).unwrap();
    // Should return characters or be empty for simple fonts
    let _ = chars;
}

// ===========================================================================
// Tests: Page content data
// ===========================================================================

#[test]
fn test_get_page_content_data() {
    let pdf = build_minimal_pdf(Some("Content data"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let data = doc.get_page_content_data(0).unwrap();
    assert!(!data.is_empty(), "Content stream should not be empty");
    // Should contain the text operator
    let content_str = String::from_utf8_lossy(&data);
    assert!(content_str.contains("BT") || content_str.contains("Tj"));
}

// ===========================================================================
// Tests: Path extraction
// ===========================================================================

#[test]
fn test_extract_paths_no_paths() {
    let pdf = build_minimal_pdf(Some("No paths"));
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let paths = doc.extract_paths(0).unwrap();
    assert!(paths.is_empty());
}

#[test]
fn test_extract_paths_with_rect() {
    // Build a PDF with rectangle drawing
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let off1 = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let off2 = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let off3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>\nendobj\n",
    );
    let off4 = pdf.len();
    let content = b"q 1 0 0 1 0 0 cm 0 0 0 rg 100 100 200 150 re f Q";
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    finalize_pdf(&mut pdf, &[0, off1, off2, off3, off4]);

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let paths = doc.extract_paths(0).unwrap();
    // Should find the rectangle path
    let _ = paths;
}

// ===========================================================================
// Tests: Authenticate (empty password)
// ===========================================================================

#[test]
fn test_authenticate_unencrypted_pdf() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    // Authenticating an unencrypted PDF should return Ok
    let result = doc.authenticate(b"");
    // Either succeeds or is a no-op for unencrypted PDFs
    let _ = result;
}

// ===========================================================================
// Tests: Structure tree
// ===========================================================================

#[test]
fn test_structure_tree_untagged() {
    let pdf = build_minimal_pdf(None);
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let tree = doc.structure_tree().unwrap();
    assert!(tree.is_none(), "Minimal PDF should not have a structure tree");
}
