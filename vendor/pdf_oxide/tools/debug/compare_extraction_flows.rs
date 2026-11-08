#!/usr/bin/env rust-script
//! Compare extraction flows to find the difference
//!
//! ```cargo
//! [dependencies]
//! pdf_oxide = { path = "." }
//! ```

use pdf_oxide::converters::ConversionOptions;
use pdf_oxide::PdfDocument;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf_path = "test_datasets/pdfs/academic/arxiv_2510.21165v1.pdf";

    println!("Opening PDF: {}", pdf_path);
    let mut doc = PdfDocument::open(pdf_path)?;

    // METHOD 1: Using to_markdown() (what the test uses)
    println!("\n=== METHOD 1: doc.to_markdown() (TEST METHOD) ===");
    let options1 = ConversionOptions::default();
    let markdown1 = doc.to_markdown(0, &options1)?;

    println!("Generated {} chars", markdown1.len());
    println!("First 500 chars:");
    println!("{}", &markdown1[..500.min(markdown1.len())]);

    // Check for word splitting
    let word_splits1 = ["var ious", "cor relation", "retur ns", "distr ibutions"];
    let mut found_issues1 = Vec::new();
    for pattern in &word_splits1 {
        if markdown1.contains(pattern) {
            found_issues1.push(pattern);
        }
    }

    println!("\nWord-splitting issues found: {:?}", found_issues1);


    // METHOD 2: Using extract_spans + convert (what export binary uses)
    println!("\n=== METHOD 2: extract_spans + convert (EXPORT BINARY METHOD) ===");
    let spans = doc.extract_spans(0)?;

    use pdf_oxide::converters::MarkdownConverter;
    let converter = MarkdownConverter::new();
    let options2 = ConversionOptions {
        detect_headings: false,
        ..Default::default()
    };
    let markdown2 = converter.convert_page_from_spans(&spans, &options2)?;

    println!("Generated {} chars", markdown2.len());
    println!("First 500 chars:");
    println!("{}", &markdown2[..500.min(markdown2.len())]);

    // Check for word splitting
    let mut found_issues2 = Vec::new();
    for pattern in &word_splits1 {
        if markdown2.contains(pattern) {
            found_issues2.push(pattern);
        }
    }

    println!("\nWord-splitting issues found: {:?}", found_issues2);


    // COMPARISON
    println!("\n=== COMPARISON ===");
    println!("Method 1 (test) issues: {:?}", found_issues1);
    println!("Method 2 (export) issues: {:?}", found_issues2);

    if found_issues1.is_empty() && !found_issues2.is_empty() {
        println!("\n🔴 FOUND IT! Method 1 (test) is CLEAN, Method 2 (export) has issues!");
        println!("The difference is in the conversion options or converter behavior.");
    } else if !found_issues1.is_empty() && found_issues2.is_empty() {
        println!("\n🔴 FOUND IT! Method 2 (export) is CLEAN, Method 1 (test) has issues!");
    } else if found_issues1 == found_issues2 {
        println!("\n✅ Both methods have the SAME issues - problem is upstream in extract_spans");
    } else {
        println!("\n⚠️  Methods have DIFFERENT issues - interesting!");
    }

    // Save both outputs for manual inspection
    std::fs::write("/tmp/method1_test.md", &markdown1)?;
    std::fs::write("/tmp/method2_export.md", &markdown2)?;
    println!("\n📝 Saved outputs to:");
    println!("  /tmp/method1_test.md");
    println!("  /tmp/method2_export.md");

    Ok(())
}
