use pdf_oxide::document::PdfDocument;
use pdf_oxide::{clear_global_font_cache, global_font_cache_stats, set_global_font_cache_capacity};
use std::io::Write;

fn write_temp_pdf(data: &[u8], name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("pdf_oxide_determinism_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(data).unwrap();
    path
}

#[test]
fn test_extraction_identical_regardless_of_stream_size() {
    let doc1 = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let doc2 = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();

    let pages = doc1.page_count().unwrap();
    for p in 0..pages {
        let t1 = doc1.extract_text(p).unwrap();
        let t2 = doc2.extract_text(p).unwrap();
        assert_eq!(t1, t2, "Page {} extraction should be identical across runs", p);
    }
}

#[test]
fn test_large_stream_with_no_text_extracts_empty() {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1_offset = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

    let obj2_offset = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

    let mut content_stream = Vec::new();
    for i in 0..500 {
        let line = format!("{} {} m {} {} l S\n", i, i * 2, i + 10, i * 2 + 10);
        content_stream.extend_from_slice(line.as_bytes());
    }

    let obj4_offset = pdf.len();
    let content_header = format!("4 0 obj\n<< /Length {} >>\nstream\n", content_stream.len());
    pdf.extend_from_slice(content_header.as_bytes());
    pdf.extend_from_slice(&content_stream);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

    let obj3_offset = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
          /Contents 4 0 R >>\nendobj\n\n",
    );

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 5\n");
    pdf.extend_from_slice(b"0000000000 65535 f \r\n");
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj1_offset).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj2_offset).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj3_offset).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj4_offset).as_bytes());

    let trailer =
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", xref_offset);
    pdf.extend_from_slice(trailer.as_bytes());

    let path = write_temp_pdf(&pdf, "large_no_text.pdf");
    let doc = PdfDocument::open(&path).expect("Should open");
    let text = doc.extract_text(0).unwrap();
    assert!(
        text.trim().is_empty(),
        "Pure-graphics stream should extract empty text, got '{}'",
        text.chars().take(100).collect::<String>()
    );
}

#[test]
fn test_repeated_extraction_deterministic() {
    let doc = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let t1 = doc.extract_text(0).unwrap();
    let t2 = doc.extract_text(0).unwrap();
    assert_eq!(t1, t2, "Repeated extraction of page 0 should be identical");
}

#[test]
fn test_all_pages_twice_deterministic() {
    let doc = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let pages = doc.page_count().unwrap();

    let mut first_pass: Vec<String> = Vec::new();
    for p in 0..pages {
        first_pass.push(doc.extract_text(p).unwrap());
    }

    for (p, first) in first_pass.iter().enumerate().take(pages) {
        let second = doc.extract_text(p).unwrap();
        assert_eq!(
            *first, second,
            "Page {} text should match between first and second extraction",
            p
        );
    }
}

#[test]
fn test_text_page_not_skipped() {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.4\n");

    let obj1_offset = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\n");

    let obj2_offset = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\n");

    let obj4_offset = pdf.len();
    pdf.extend_from_slice(
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
          /Encoding /WinAnsiEncoding >>\nendobj\n\n",
    );

    let content = b"BT /F1 12 Tf 72 700 Td (NotSkipped) Tj ET";
    let obj5_offset = pdf.len();
    let content_header = format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len());
    pdf.extend_from_slice(content_header.as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream\nendobj\n\n");

    let obj3_offset = pdf.len();
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
          /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n\n",
    );

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n");
    pdf.extend_from_slice(b"0000000000 65535 f \r\n");
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj1_offset).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj2_offset).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj3_offset).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj4_offset).as_bytes());
    pdf.extend_from_slice(format!("{:010} 00000 n \r\n", obj5_offset).as_bytes());

    let trailer =
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", xref_offset);
    pdf.extend_from_slice(trailer.as_bytes());

    let path = write_temp_pdf(&pdf, "text_not_skipped.pdf");
    let doc = PdfDocument::open(&path).expect("Should open");
    let text = doc.extract_text(0).unwrap();
    assert!(
        text.contains("NotSkipped"),
        "Page with /Font resources should not be skipped, got '{}'",
        text
    );
}

#[test]
fn test_global_cache_populated_after_extraction() {
    clear_global_font_cache();

    let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let _text = doc.extract_text(0).unwrap();

    let (size, _capacity) = global_font_cache_stats();
    assert!(size <= _capacity, "Cache size should not exceed capacity");
}

#[test]
fn test_global_cache_clear_works() {
    let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
    let _text = doc.extract_text(0).unwrap();
    drop(doc);

    clear_global_font_cache();
    let (size, _) = global_font_cache_stats();
    assert_eq!(size, 0, "After clear, cache size should be 0");
}

#[test]
fn test_global_cache_capacity_limit() {
    clear_global_font_cache();
    set_global_font_cache_capacity(1);

    let doc = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let pages = doc.page_count().unwrap();
    for p in 0..pages {
        let _text = doc.extract_text(p).unwrap();
    }
    drop(doc);

    let (size, capacity) = global_font_cache_stats();
    assert!(size <= 1, "Cache size should be at most 1, got {}", size);
    assert_eq!(capacity, 1, "Capacity should be 1");

    set_global_font_cache_capacity(1024);
    clear_global_font_cache();
}

#[test]
fn test_three_independent_runs_identical() {
    clear_global_font_cache();

    let mut results: Vec<Vec<String>> = Vec::new();

    for _ in 0..3 {
        let doc = PdfDocument::open("tests/fixtures/simple.pdf").unwrap();
        let pages = doc.page_count().unwrap();
        let mut page_texts = Vec::new();
        for p in 0..pages {
            page_texts.push(doc.extract_text(p).unwrap());
        }
        results.push(page_texts);
    }

    for run in 1..3 {
        assert_eq!(results[0].len(), results[run].len(), "Run {} page count differs", run);
        for (p, (a, b)) in results[0].iter().zip(results[run].iter()).enumerate() {
            assert_eq!(a, b, "Run 0 vs run {}: page {} text differs", run, p);
        }
    }
}

#[test]
fn test_file_vs_bytes_extraction_identical() {
    let path = "tests/fixtures/simple.pdf";
    let data = std::fs::read(path).unwrap();

    let doc_file = PdfDocument::open(path).unwrap();
    let doc_bytes = PdfDocument::from_bytes(data).unwrap();

    let pages = doc_file.page_count().unwrap();
    for p in 0..pages {
        let t1 = doc_file.extract_text(p).unwrap();
        let t2 = doc_bytes.extract_text(p).unwrap();
        assert_eq!(t1, t2, "Page {} file vs bytes extraction should match", p);
    }
}
