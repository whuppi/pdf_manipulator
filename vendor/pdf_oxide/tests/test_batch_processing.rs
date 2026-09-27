use pdf_oxide::batch::{BatchProcessor, BatchSummary};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn test_batch_multiple_valid_files() {
    let processor = BatchProcessor::new();
    let results = processor.extract_text_from_files(&[
        Path::new("tests/fixtures/simple.pdf"),
        Path::new("tests/fixtures/outline.pdf"),
    ]);

    assert_eq!(results.len(), 2);
    for r in &results {
        assert!(
            r.text.is_ok(),
            "File {} should succeed: {:?}",
            r.path.display(),
            r.text.as_ref().err()
        );
    }
}

#[test]
fn test_batch_mixed_valid_and_invalid() {
    let processor = BatchProcessor::new();
    let results = processor.extract_text_from_files(&[
        Path::new("tests/fixtures/simple.pdf"),
        Path::new("/nonexistent/fake.pdf"),
    ]);

    assert_eq!(results.len(), 2, "Should have results for both files");

    let ok_count = results.iter().filter(|r| r.text.is_ok()).count();
    let err_count = results.iter().filter(|r| r.text.is_err()).count();

    assert_eq!(ok_count, 1, "One file should succeed");
    assert_eq!(err_count, 1, "One file should fail");
}

#[test]
fn test_batch_progress_callback_invoked() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&call_count);

    let processor = BatchProcessor::new().with_progress(Box::new(move |completed, total| {
        count_clone.fetch_add(1, Ordering::Relaxed);
        assert!(completed <= total, "completed should not exceed total");
        assert_eq!(total, 2, "total should be 2");
    }));

    let _results = processor.extract_text_from_files(&[
        Path::new("tests/fixtures/simple.pdf"),
        Path::new("tests/fixtures/outline.pdf"),
    ]);

    let calls = call_count.load(Ordering::Relaxed);
    assert_eq!(calls, 2, "Progress callback should be called once per file");
}

#[test]
fn test_batch_summary_statistics() {
    let processor = BatchProcessor::new();
    let results = processor.extract_text_from_files(&[
        Path::new("tests/fixtures/simple.pdf"),
        Path::new("/nonexistent/fake.pdf"),
        Path::new("tests/fixtures/outline.pdf"),
    ]);

    let summary = BatchSummary::from_results(&results);

    assert_eq!(summary.total, 3);
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.failed, 1);
    assert!(summary.total_pages > 0, "Should have counted some pages");
}

#[test]
fn test_batch_directory_extraction() {
    let processor = BatchProcessor::new();
    let results = processor
        .extract_text_from_directory(Path::new("tests/fixtures/"))
        .unwrap();

    assert!(
        results.len() >= 2,
        "Should find at least 2 PDFs in fixtures dir, found {}",
        results.len()
    );

    let ok_count = results.iter().filter(|r| r.text.is_ok()).count();
    assert!(ok_count >= 2, "At least 2 PDFs should parse successfully");
}
