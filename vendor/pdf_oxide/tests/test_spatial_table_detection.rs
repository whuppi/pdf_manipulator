use pdf_oxide::document::PdfDocument;
use pdf_oxide::structure::spatial_table_detector::{SpatialTableDetector, TableDetectionConfig};

#[test]
fn test_table_detection_on_fixtures() {
    for fixture in &["tests/fixtures/simple.pdf", "tests/fixtures/outline.pdf"] {
        let doc = PdfDocument::open(fixture).unwrap();
        let pages = doc.page_count().unwrap();
        let detector = SpatialTableDetector::with_config(TableDetectionConfig::default());

        for p in 0..pages {
            let spans = doc.extract_spans(p).unwrap();
            let _tables = detector.detect_tables(&spans);
        }
    }
}

#[test]
fn test_table_detection_deterministic() {
    let doc1 = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();
    let doc2 = PdfDocument::open("tests/fixtures/outline.pdf").unwrap();

    let pages = doc1.page_count().unwrap();
    let detector = SpatialTableDetector::with_config(TableDetectionConfig::default());

    for p in 0..pages {
        let spans1 = doc1.extract_spans(p).unwrap();
        let spans2 = doc2.extract_spans(p).unwrap();

        let tables1 = detector.detect_tables(&spans1);
        let tables2 = detector.detect_tables(&spans2);

        assert_eq!(tables1.len(), tables2.len(), "Page {}: table count should be deterministic", p);
    }
}
