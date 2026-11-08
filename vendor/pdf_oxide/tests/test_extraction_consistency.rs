use pdf_oxide::document::PdfDocument;
use std::io::Write;

fn finalize_pdf(pdf: &mut Vec<u8>, obj_offsets: &[usize]) {
    let xref_offset = pdf.len();
    let count = obj_offsets.len();
    pdf.extend_from_slice(
        format!(
            "xref
0 {}
",
            count
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(
        b"0000000000 65535 f 
",
    );
    for &off in &obj_offsets[1..] {
        pdf.extend_from_slice(
            format!(
                "{:010} 00000 n 
",
                off
            )
            .as_bytes(),
        );
    }
    let trailer = format!(
        "trailer
<< /Size {} /Root 1 0 R >>
startxref
{}
%%EOF
",
        count, xref_offset
    );
    pdf.extend_from_slice(trailer.as_bytes());
}

fn build_test_pdf_with_xobject() -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(
        b"%PDF-1.4
",
    );

    let obj1 = pdf.len();
    pdf.extend_from_slice(
        b"1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj

",
    );

    let obj2 = pdf.len();
    pdf.extend_from_slice(
        b"2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj

",
    );

    let obj3 = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] 
          /Resources << /Font << /F1 5 0 R >> /XObject << /X0 4 0 R >> >> 
          /Contents 6 0 R >>
endobj

",
    );

    let obj4 = pdf.len();
    let stream = b"BT /F1 12 Tf 10 10 Td (XObjectContent) Tj ET";
    let header = format!(
        "4 0 obj
<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] 
         /Resources << /Font << /F1 5 0 R >> >> /Length {} >>
stream
",
        stream.len()
    );
    pdf.extend_from_slice(header.as_bytes());
    pdf.extend_from_slice(stream);
    pdf.extend_from_slice(
        b"
endstream
endobj

",
    );

    let obj5 = pdf.len();
    pdf.extend_from_slice(
        b"5 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica 
          /Encoding /WinAnsiEncoding >>
endobj

",
    );

    let obj6 = pdf.len();
    let content = b"BT /F1 12 Tf 10 50 Td (PageContent) Tj ET /X0 Do";
    let header = format!(
        "6 0 obj
<< /Length {} >>
stream
",
        content.len()
    );
    pdf.extend_from_slice(header.as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(
        b"
endstream
endobj

",
    );

    finalize_pdf(&mut pdf, &[0, obj1, obj2, obj3, obj4, obj5, obj6]);
    pdf
}

fn write_temp_pdf(data: &[u8], name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("pdf_oxide_consistency_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

#[test]
fn test_synthetic_repeated_extraction_consistency() {
    let data = build_test_pdf_with_xobject();
    let path = write_temp_pdf(&data, "consistency_test.pdf");

    let doc = PdfDocument::open(&path).unwrap();

    // 1. extract_spans
    let spans1 = doc.extract_spans(0).unwrap();
    assert!(!spans1.is_empty(), "First spans call should not be empty");
    let has_xobj_text = spans1.iter().any(|s| s.text.contains("XObjectContent"));
    assert!(has_xobj_text, "Should contain text from XObject");

    // 2. extract_spans again (should use cache)
    let spans2 = doc.extract_spans(0).unwrap();
    assert_eq!(spans1.len(), spans2.len(), "Second spans call should match first");

    // 3. extract_chars (should IGNORE span cache and process stream)
    let chars1 = doc.extract_chars(0).unwrap();
    assert!(!chars1.is_empty(), "First chars call should not be empty");

    // 4. extract_chars again
    let chars2 = doc.extract_chars(0).unwrap();
    assert_eq!(chars1.len(), chars2.len(), "Second chars call should match first");

    // 5. extract_spans again (should still use cache and NOT be empty)
    let spans3 = doc.extract_spans(0).unwrap();
    assert_eq!(spans1.len(), spans3.len(), "Third spans call should match first");
}

#[test]
fn test_synthetic_chars_then_spans_consistency() {
    let data = build_test_pdf_with_xobject();
    let path = write_temp_pdf(&data, "consistency_test_2.pdf");

    let doc = PdfDocument::open(&path).unwrap();

    // 1. extract_chars
    let chars1 = doc.extract_chars(0).unwrap();
    assert!(!chars1.is_empty(), "First chars call should not be empty");

    // 2. extract_chars again
    let chars2 = doc.extract_chars(0).unwrap();
    assert_eq!(chars1.len(), chars2.len(), "Second chars call should match first");

    // 3. extract_spans (should NOT be affected by previous char extraction poisoning cache)
    let spans1 = doc.extract_spans(0).unwrap();
    assert!(!spans1.is_empty(), "First spans call after chars should not be empty");
    let has_xobj_text = spans1.iter().any(|s| s.text.contains("XObjectContent"));
    assert!(has_xobj_text, "Should contain text from XObject after char extraction");

    // 4. extract_spans again
    let spans2 = doc.extract_spans(0).unwrap();
    assert_eq!(spans1.len(), spans2.len(), "Second spans call should match first");
}
