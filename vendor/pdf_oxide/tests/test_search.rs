//! Tests for the text search functionality.

use pdf_oxide::api::Pdf;
use pdf_oxide::search::{SearchOptions, TextSearcher};
use pdf_oxide::PdfDocument;

/// Helper function to create a test PDF with searchable text.
fn create_test_pdf_with_text(text: &str) -> Vec<u8> {
    let pdf = Pdf::from_text(text).expect("Failed to create PDF");
    pdf.into_bytes()
}

mod search_options {
    use super::*;

    #[test]
    fn test_search_options_default() {
        let opts = SearchOptions::default();
        assert!(!opts.case_insensitive);
        assert!(!opts.literal);
        assert!(!opts.whole_word);
        assert_eq!(opts.max_results, 0);
        assert!(opts.page_range.is_none());
    }

    #[test]
    fn test_search_options_builder() {
        let opts = SearchOptions::new()
            .with_case_insensitive(true)
            .with_literal(true)
            .with_whole_word(true)
            .with_max_results(10)
            .with_page_range(0, 5);

        assert!(opts.case_insensitive);
        assert!(opts.literal);
        assert!(opts.whole_word);
        assert_eq!(opts.max_results, 10);
        assert_eq!(opts.page_range, Some((0, 5)));
    }

    #[test]
    fn test_search_options_case_insensitive() {
        let opts = SearchOptions::case_insensitive();
        assert!(opts.case_insensitive);
        assert!(!opts.literal);
        assert!(!opts.whole_word);
    }
}

mod text_search {
    use super::*;

    #[test]
    fn test_simple_text_search() {
        // Create a test PDF
        let bytes = create_test_pdf_with_text("Hello World! Welcome to PDF search testing.");

        // Save to temp file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_simple.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        // Search for text
        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");
        let options = SearchOptions::default();
        let results = TextSearcher::search(&doc, "Hello", &options).expect("Search failed");

        assert!(!results.is_empty(), "Should find at least one match for 'Hello'");
        assert!(results[0].text.contains("Hello"));

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_case_insensitive_search() {
        let bytes = create_test_pdf_with_text("Hello World! hello again. HELLO once more.");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_case.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");

        // Case-sensitive search should find exact matches
        let options = SearchOptions::default();
        let results = TextSearcher::search(&doc, "hello", &options).expect("Search failed");

        // Case-insensitive search should find all variations
        let options_insensitive = SearchOptions::case_insensitive();
        let results_insensitive =
            TextSearcher::search(&doc, "hello", &options_insensitive).expect("Search failed");

        // The case-insensitive search should find more or equal matches
        assert!(
            results_insensitive.len() >= results.len(),
            "Case insensitive should find at least as many matches"
        );

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_regex_search() {
        let bytes = create_test_pdf_with_text("Item 1, Item 2, Item 3, and some text.");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_regex.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");

        // Search for pattern "Item \\d"
        let options = SearchOptions::default();
        let results = TextSearcher::search(&doc, r"Item \d", &options).expect("Search failed");

        // Should find multiple matches
        assert!(!results.is_empty(), "Should find at least one 'Item N' pattern");

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_literal_search() {
        let bytes =
            create_test_pdf_with_text("The regex a.b matches axb but literal a.b only matches a.b");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_literal.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");

        // Literal search for "a.b" - should not match "axb"
        let options = SearchOptions::new().with_literal(true);
        let results = TextSearcher::search(&doc, "a.b", &options).expect("Search failed");

        for result in &results {
            // Each match should contain literal "a.b"
            assert!(result.text.contains("a.b"), "Literal match should contain 'a.b'");
        }

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_whole_word_search() {
        let bytes = create_test_pdf_with_text("The cat sat on the mat. A category is not a cat.");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_whole_word.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");

        // Whole word search for "cat" - should not match "category"
        let options = SearchOptions::new().with_whole_word(true);
        let results = TextSearcher::search(&doc, "cat", &options).expect("Search failed");

        // Verify none of the matches are "category"
        for result in &results {
            assert!(
                !result.text.contains("category"),
                "Whole word search should not match 'category'"
            );
        }

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_max_results_limit() {
        let bytes = create_test_pdf_with_text("test test test test test test test test test test");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_max.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");

        // Limit to 3 results
        let options = SearchOptions::new().with_max_results(3);
        let results = TextSearcher::search(&doc, "test", &options).expect("Search failed");

        assert!(results.len() <= 3, "Should respect max_results limit of 3");

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_no_matches() {
        let bytes = create_test_pdf_with_text("Hello World!");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_no_match.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");

        let options = SearchOptions::default();
        let results =
            TextSearcher::search(&doc, "xyz123notfound", &options).expect("Search failed");

        assert!(results.is_empty(), "Should return empty results for non-existent text");

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_search_result_has_position_info() {
        let bytes = create_test_pdf_with_text("Find me in this document.");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_search_position.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let doc = PdfDocument::open(&temp_path).expect("Failed to open PDF");

        let options = SearchOptions::default();
        let results = TextSearcher::search(&doc, "Find", &options).expect("Search failed");

        if !results.is_empty() {
            let result = &results[0];
            // Verify that position information is present
            assert_eq!(result.page, 0, "Match should be on page 0");
            assert!(result.bbox.width > 0.0, "Bounding box should have positive width");
            assert!(result.bbox.height > 0.0, "Bounding box should have positive height");
        }

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }
}

mod api_integration {
    use super::*;

    #[test]
    fn test_pdf_search_api() {
        let bytes = create_test_pdf_with_text("API search test. Find this text easily.");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_api_search.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&temp_path).expect("Failed to open PDF");

        // Use the high-level API
        let results = pdf.search("search").expect("Search failed");

        assert!(!results.is_empty(), "Should find 'search' in the document");

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_pdf_search_with_options_api() {
        let bytes = create_test_pdf_with_text("SEARCH Search search");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_api_search_opts.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&temp_path).expect("Failed to open PDF");

        // Case insensitive search
        let options = SearchOptions::case_insensitive();
        let results = pdf
            .search_with_options("search", options)
            .expect("Search failed");

        // Should find all variations
        assert!(
            !results.is_empty(),
            "Should find at least one match with case insensitive search"
        );

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_pdf_search_page_api() {
        let bytes = create_test_pdf_with_text("Page specific search test.");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_api_search_page.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&temp_path).expect("Failed to open PDF");

        // Search only on page 0
        let results = pdf.search_page(0, "specific").expect("Search failed");

        // All results should be on page 0
        for result in &results {
            assert_eq!(result.page, 0, "All results should be from page 0");
        }

        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
    }
}

mod highlight_integration {
    use super::*;

    #[test]
    fn test_highlight_matches() {
        let bytes = create_test_pdf_with_text("Highlight this word. And also this word.");

        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_highlight_input.pdf");
        let output_path = temp_dir.join("test_highlight_output.pdf");
        std::fs::write(&temp_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&temp_path).expect("Failed to open PDF");

        // Search for matches
        let results = pdf.search("word").expect("Search failed");

        if !results.is_empty() {
            // Highlight the matches
            pdf.highlight_matches(&results, [1.0, 1.0, 0.0])
                .expect("Highlight failed");

            // Save the highlighted PDF
            pdf.save(&output_path).expect("Save failed");

            // Verify the output file was created
            assert!(output_path.exists(), "Highlighted PDF should be created");

            // Cleanup output
            let _ = std::fs::remove_file(&output_path);
        }

        // Cleanup input
        let _ = std::fs::remove_file(&temp_path);
    }
}
