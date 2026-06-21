use pdf_oxide::converters::ConversionOptions;
use pdf_oxide::document::PdfDocument;

#[test]
fn test_recursive_xobject_image_extraction_safety() {
    // Regression test for ensuring recursive image extraction from Form XObjects
    // does not trigger RefCell borrow conflicts.

    let mut pdf = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    pdf.extend_from_slice(b"%PDF-1.4\n");

    // 1 0 obj: Catalog
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // 2 0 obj: Pages
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // 3 0 obj: Page
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /XObject << /Form1 5 0 R >> >> >>\nendobj\n");

    // 4 0 obj: Page content (invokes Form1)
    let content = b"/Form1 Do";
    offsets.push(pdf.len());
    pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    // 5 0 obj: Form XObject (invokes Im1)
    let form_content = b"q 100 0 0 100 100 100 cm /Im1 Do Q";
    let form_res = b"<< /XObject << /Im1 6 0 R >> >>";
    offsets.push(pdf.len());
    pdf.extend_from_slice(format!("5 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 612 792] /Resources {} /Length {} >>\nstream\n", String::from_utf8_lossy(form_res), form_content.len()).as_bytes());
    pdf.extend_from_slice(form_content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    // 6 0 obj: Image XObject
    let img_data = vec![0u8; 100 * 100 * 3];
    offsets.push(pdf.len());
    pdf.extend_from_slice(format!("6 0 obj\n<< /Type /XObject /Subtype /Image /Width 100 /Height 100 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n", img_data.len()).as_bytes());
    pdf.extend_from_slice(&img_data);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n");
    pdf.extend_from_slice(format!("0 {}\n", offsets.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }

    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            offsets.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );

    let doc = PdfDocument::from_bytes(pdf).expect("Failed to parse PDF");

    let options = ConversionOptions {
        include_images: true,
        ..Default::default()
    };

    // Multiple iterations to ensure caching logic is also exercised safely
    for _ in 0..10 {
        let result = doc.to_markdown(0, &options);
        assert!(result.is_ok(), "Image extraction failed in recursive XObject");
        let markdown = result.unwrap();
        assert!(markdown.contains("![Image"), "Markdown should contain image reference");
    }
}
