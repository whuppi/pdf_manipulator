//! Integration tests for PDF writing/creation
//!
//! Targets coverage gaps in:
//! - writer/content_stream.rs (54.9% → higher)
//! - api/pdf_builder.rs (55.9% → higher)
//! - annotations.rs (37.6% → higher)

use pdf_oxide::api::{Pdf, PdfBuilder};
use pdf_oxide::document::PdfDocument;
use pdf_oxide::writer::{
    ContentStreamBuilder, DocumentBuilder, DocumentMetadata, LineCap, LineJoin, PageSize,
};

// ===========================================================================
// Tests: ContentStreamBuilder
// ===========================================================================

#[test]
fn test_content_stream_builder_basic() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .begin_text()
        .set_font("F1", 12.0)
        .text("Hello", 72.0, 720.0)
        .end_text();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("BT"));
    assert!(content.contains("ET"));
    assert!(content.contains("Tf"));
}

#[test]
fn test_content_stream_builder_colors() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .set_fill_color(1.0, 0.0, 0.0)
        .set_stroke_color(0.0, 1.0, 0.0)
        .set_line_width(2.0);

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("rg"));
    assert!(content.contains("RG"));
    assert!(content.contains("w"));
}

#[test]
fn test_content_stream_builder_path_ops() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .move_to(100.0, 200.0)
        .line_to(300.0, 400.0)
        .rect(50.0, 50.0, 200.0, 150.0)
        .stroke();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("m"));
    assert!(content.contains("l"));
    assert!(content.contains("re"));
    assert!(content.contains("S"));
}

#[test]
fn test_content_stream_builder_fill_ops() {
    let mut builder = ContentStreamBuilder::new();
    builder.rect(0.0, 0.0, 100.0, 100.0).fill();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("f"));
}

#[test]
fn test_content_stream_builder_fill_even_odd() {
    let mut builder = ContentStreamBuilder::new();
    builder.rect(0.0, 0.0, 100.0, 100.0).fill_even_odd();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("f*"));
}

#[test]
fn test_content_stream_builder_fill_stroke() {
    let mut builder = ContentStreamBuilder::new();
    builder.rect(0.0, 0.0, 100.0, 100.0).fill_stroke();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("B"));
}

#[test]
fn test_content_stream_builder_close_fill_stroke() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .move_to(0.0, 0.0)
        .line_to(100.0, 0.0)
        .line_to(100.0, 100.0)
        .close_fill_stroke();

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_close_path() {
    let mut builder = ContentStreamBuilder::new();
    builder.move_to(0.0, 0.0).line_to(100.0, 100.0).close_path();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("h"));
}

#[test]
fn test_content_stream_builder_clip() {
    let mut builder = ContentStreamBuilder::new();
    builder.rect(0.0, 0.0, 100.0, 100.0).clip().end_path();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("W"));
    assert!(content.contains("n"));
}

#[test]
fn test_content_stream_builder_clip_rect() {
    let mut builder = ContentStreamBuilder::new();
    builder.clip_rect(10.0, 10.0, 200.0, 300.0);
    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_save_restore() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .save_state()
        .set_fill_color(1.0, 0.0, 0.0)
        .rect(0.0, 0.0, 100.0, 100.0)
        .fill()
        .restore_state();

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("q"));
    assert!(content.contains("Q"));
}

#[test]
fn test_content_stream_builder_transform() {
    let mut builder = ContentStreamBuilder::new();
    builder.transform(1.0, 0.0, 0.0, 1.0, 72.0, 720.0);

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("cm"));
}

#[test]
fn test_content_stream_builder_translate() {
    let mut builder = ContentStreamBuilder::new();
    builder.translate(100.0, 200.0);

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("cm"));
}

#[test]
fn test_content_stream_builder_scale() {
    let mut builder = ContentStreamBuilder::new();
    builder.scale(2.0, 3.0);

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_rotate() {
    let mut builder = ContentStreamBuilder::new();
    builder.rotate(std::f32::consts::FRAC_PI_2); // 90 degrees
    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_rotate_degrees() {
    let mut builder = ContentStreamBuilder::new();
    builder.rotate_degrees(45.0);
    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_line_cap_join() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .set_line_cap(LineCap::Round)
        .set_line_join(LineJoin::Bevel)
        .set_miter_limit(10.0);

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_dash_pattern() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .set_dash_pattern(vec![5.0, 3.0], 0.0)
        .move_to(0.0, 0.0)
        .line_to(100.0, 0.0)
        .stroke()
        .set_solid_line();

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_cmyk() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .set_fill_color_cmyk(0.0, 1.0, 0.0, 0.0)
        .set_stroke_color_cmyk(1.0, 0.0, 0.0, 0.0);

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("k") || content.contains("K"));
}

#[test]
fn test_content_stream_builder_curves() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .move_to(0.0, 0.0)
        .curve_to(25.0, 100.0, 75.0, 100.0, 100.0, 0.0)
        .curve_to_v(50.0, 50.0, 100.0, 0.0)
        .curve_to_y(0.0, 50.0, 50.0, 50.0)
        .stroke();

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_circle() {
    let mut builder = ContentStreamBuilder::new();
    builder.circle(200.0, 400.0, 50.0).stroke();

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_ellipse() {
    let mut builder = ContentStreamBuilder::new();
    builder.ellipse(200.0, 400.0, 100.0, 50.0).fill();

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_rounded_rect() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .rounded_rect(50.0, 50.0, 200.0, 100.0, 10.0)
        .fill_stroke();

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_draw_image() {
    let mut builder = ContentStreamBuilder::new();
    builder.draw_image("Im1", 72.0, 720.0, 200.0, 150.0);

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("Do"));
}

#[test]
fn test_content_stream_builder_ext_gstate() {
    let mut builder = ContentStreamBuilder::new();
    builder.set_ext_gstate("GS1");

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("gs"));
}

#[test]
fn test_content_stream_builder_color_space() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .set_fill_color_space("DeviceRGB")
        .set_stroke_color_space("DeviceCMYK");

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_color_n() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .set_fill_color_n(vec![0.5, 0.3, 0.7])
        .set_stroke_color_n(vec![0.1, 0.2]);

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_pattern() {
    let mut builder = ContentStreamBuilder::new();
    builder
        .set_fill_pattern("P1", vec![0.5])
        .set_stroke_pattern("P2", vec![0.3]);

    let data = builder.build().unwrap();
    assert!(!data.is_empty());
}

#[test]
fn test_content_stream_builder_shading() {
    let mut builder = ContentStreamBuilder::new();
    builder.paint_shading("Sh1");

    let data = builder.build().unwrap();
    let content = String::from_utf8_lossy(&data);
    assert!(content.contains("sh"));
}

// ===========================================================================
// Tests: DocumentBuilder
// ===========================================================================

#[test]
fn test_document_builder_simple() {
    let mut builder = DocumentBuilder::new();
    builder = builder.metadata(DocumentMetadata::new().title("Test").author("Author"));
    {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0).text("Hello World").done();
    }
    let pdf_bytes = builder.build().expect("Should build PDF");
    assert!(!pdf_bytes.is_empty());

    // Verify the produced PDF is valid
    let doc = PdfDocument::from_bytes(pdf_bytes).unwrap();
    assert_eq!(doc.page_count().unwrap(), 1);
}

#[test]
fn test_document_builder_multi_page() {
    let mut builder = DocumentBuilder::new();
    for i in 0..3 {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0).text(&format!("Page {}", i + 1)).done();
    }
    let pdf_bytes = builder.build().unwrap();

    let doc = PdfDocument::from_bytes(pdf_bytes).unwrap();
    assert_eq!(doc.page_count().unwrap(), 3);
}

#[test]
fn test_document_builder_a4_page() {
    let mut builder = DocumentBuilder::new();
    {
        let page = builder.page(PageSize::A4);
        page.at(72.0, 720.0).text("A4 page").done();
    }
    let pdf_bytes = builder.build().unwrap();
    assert!(!pdf_bytes.is_empty());
}

#[test]
fn test_document_builder_custom_page_size() {
    let mut builder = DocumentBuilder::new();
    {
        let page = builder.page(PageSize::Custom(400.0, 300.0));
        page.at(10.0, 280.0).text("Custom").done();
    }
    let pdf_bytes = builder.build().unwrap();
    assert!(!pdf_bytes.is_empty());
}

// ===========================================================================
// Tests: Pdf high-level API
// ===========================================================================

#[test]
fn test_pdf_from_text() {
    let pdf = Pdf::from_text("Hello from text API").unwrap();
    assert!(!pdf.as_bytes().is_empty());
}

#[test]
fn test_pdf_from_text_multiline() {
    let text = "Line 1\nLine 2\nLine 3\n\nParagraph 2";
    let pdf = Pdf::from_text(text).unwrap();
    assert!(!pdf.as_bytes().is_empty());
}

#[test]
fn test_pdf_from_markdown() {
    let md = "# Hello\n\nThis is **bold** and *italic*.\n\n- Item 1\n- Item 2";
    let pdf = Pdf::from_markdown(md).unwrap();
    assert!(!pdf.as_bytes().is_empty());
}

#[test]
fn test_pdf_from_html() {
    let html = "<h1>Title</h1><p>Paragraph with <b>bold</b> text.</p>";
    let pdf = Pdf::from_html(html).unwrap();
    assert!(!pdf.as_bytes().is_empty());
}

#[test]
fn test_pdf_new_empty() {
    let pdf = Pdf::new();
    let _ = pdf.as_bytes(); // Just verify no crash
}

#[test]
fn test_pdf_save_and_reopen() {
    let pdf = Pdf::from_text("Save test").unwrap();
    let dir = std::env::temp_dir().join("pdf_oxide_writer_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("save_test.pdf");
    let bytes = pdf.as_bytes().to_vec();
    std::fs::write(&path, &bytes).unwrap();

    let doc = PdfDocument::open(&path).unwrap();
    assert_eq!(doc.page_count().unwrap(), 1);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_pdf_into_bytes() {
    let pdf = Pdf::from_text("Bytes test").unwrap();
    let bytes = pdf.into_bytes();
    assert!(!bytes.is_empty());
}

// ===========================================================================
// Tests: PdfBuilder
// ===========================================================================

#[test]
fn test_pdf_builder_chain() {
    let pdf = PdfBuilder::new()
        .title("Builder Title")
        .author("Builder Author")
        .subject("Builder Subject")
        .keywords("test, builder")
        .page_size(PageSize::A4)
        .margin(36.0)
        .font_size(14.0)
        .line_height(1.5)
        .from_text("Built with builder")
        .unwrap();

    assert!(!pdf.as_bytes().is_empty());
}

#[test]
fn test_pdf_builder_margins() {
    let pdf = PdfBuilder::new()
        .margins(36.0, 36.0, 72.0, 72.0)
        .from_text("Custom margins")
        .unwrap();

    assert!(!pdf.as_bytes().is_empty());
}

#[test]
fn test_pdf_builder_from_markdown() {
    let pdf = PdfBuilder::new()
        .title("MD Builder")
        .from_markdown("# Title\n\nParagraph")
        .unwrap();

    assert!(!pdf.as_bytes().is_empty());
}

#[test]
fn test_pdf_builder_from_html() {
    let pdf = PdfBuilder::new()
        .title("HTML Builder")
        .from_html("<h1>Title</h1><p>Content</p>")
        .unwrap();

    assert!(!pdf.as_bytes().is_empty());
}

// ===========================================================================
// Tests: Pdf open and extract
// ===========================================================================

#[test]
fn test_pdf_open_fixture() {
    let mut pdf = Pdf::open("tests/fixtures/simple.pdf").unwrap();
    let count = pdf.page_count().unwrap();
    assert!(count >= 1);
}

#[test]
fn test_pdf_to_text() {
    let mut pdf = Pdf::open("tests/fixtures/simple.pdf").unwrap();
    let text = pdf.to_text(0);
    let _ = text; // Just verify no crash
}

#[test]
fn test_pdf_to_markdown_method() {
    let mut pdf = Pdf::open("tests/fixtures/simple.pdf").unwrap();
    let md = pdf.to_markdown(0);
    let _ = md;
}

#[test]
fn test_pdf_to_html_method() {
    let mut pdf = Pdf::open("tests/fixtures/simple.pdf").unwrap();
    let html = pdf.to_html(0);
    let _ = html;
}

#[test]
fn test_pdf_config() {
    let pdf = Pdf::from_text("Config test").unwrap();
    let config = pdf.config();
    // PdfConfig should have sensible defaults
    let _ = config;
}

#[test]
fn test_pdf_source_path_none_for_new() {
    let pdf = Pdf::from_text("No source").unwrap();
    assert!(pdf.source_path().is_none());
}

#[test]
fn test_pdf_source_path_some_for_opened() {
    let pdf = Pdf::open("tests/fixtures/simple.pdf").unwrap();
    assert!(pdf.source_path().is_some());
}

// ===========================================================================
// Tests: Annotations extraction
// ===========================================================================

#[test]
fn test_annotations_empty_page() {
    let pdf = Pdf::from_text("No annotations").unwrap();
    let bytes = pdf.as_bytes().to_vec();
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    let annotations = doc.get_annotations(0).unwrap();
    assert!(annotations.is_empty());
}

// ===========================================================================
// Tests: PDF round-trip (create + read)
// ===========================================================================

#[test]
fn test_round_trip_text_pdf() {
    // Create
    let mut builder = DocumentBuilder::new();
    builder = builder.metadata(DocumentMetadata::new().title("Round Trip"));
    {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0).text("Round trip content").done();
    }
    let pdf_bytes = builder.build().unwrap();

    // Read back
    let doc = PdfDocument::from_bytes(pdf_bytes).unwrap();
    assert_eq!(doc.page_count().unwrap(), 1);
    let text = doc.extract_text(0).unwrap();
    assert!(text.contains("Round") || text.contains("trip"));
}

#[test]
fn test_round_trip_multi_page() {
    let mut builder = DocumentBuilder::new();
    for i in 0..5 {
        let page = builder.page(PageSize::Letter);
        page.at(72.0, 720.0).text(&format!("Content {}", i)).done();
    }
    let pdf_bytes = builder.build().unwrap();

    let doc = PdfDocument::from_bytes(pdf_bytes).unwrap();
    assert_eq!(doc.page_count().unwrap(), 5);
}
