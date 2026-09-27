#!/usr/bin/env rust
//! Comprehensive PDF testing tool
//!
//! Processes large collections of PDFs and generates detailed statistics:
//! - Success/failure rates by category
//! - Feature coverage analysis
//! - Performance metrics
//! - Problem identification

use pdf_oxide::{Error, PdfDocument};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Default, Serialize, Deserialize)]
struct TestResults {
    total_files: usize,
    successful: usize,
    failed: usize,

    // By category
    results_by_category: HashMap<String, CategoryStats>,

    // Feature coverage
    with_bookmarks: usize,
    with_annotations: usize,
    with_forms: usize,
    with_images: usize,

    // Performance
    total_time_ms: u64,
    total_pages: usize,
    avg_time_per_pdf: f64,
    avg_time_per_page: f64,

    // Problems
    failed_files: Vec<FailedFile>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CategoryStats {
    total: usize,
    successful: usize,
    failed: usize,
    total_pages: usize,
    total_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct FailedFile {
    path: String,
    category: String,
    error: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let root_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("test_datasets/pdfs")
    };

    if !root_dir.exists() {
        eprintln!("Directory not found: {}", root_dir.display());
        eprintln!("Usage: comprehensive_test [directory]");
        eprintln!("Default: test_datasets/pdfs");
        std::process::exit(1);
    }

    println!("=== PDF Library Comprehensive Test Suite ===");
    println!("Testing directory: {}", root_dir.display());
    println!();

    let start = Instant::now();
    let mut results = TestResults::default();

    // Scan for all PDFs
    let pdf_files = find_all_pdfs(&root_dir);
    results.total_files = pdf_files.len();

    println!("Found {} PDF files", pdf_files.len());
    println!();

    // Process each PDF
    let mut progress_interval = (pdf_files.len() / 20).max(1);
    for (idx, (path, category)) in pdf_files.iter().enumerate() {
        if (idx + 1) % progress_interval == 0 {
            println!(
                "Progress: {}/{} ({:.1}%)",
                idx + 1,
                pdf_files.len(),
                (idx + 1) as f64 / pdf_files.len() as f64 * 100.0
            );
        }

        test_pdf(path, category, &mut results);
    }

    let elapsed = start.elapsed();
    results.total_time_ms = elapsed.as_millis() as u64;
    results.avg_time_per_pdf = results.total_time_ms as f64 / results.total_files as f64;
    if results.total_pages > 0 {
        results.avg_time_per_page = results.total_time_ms as f64 / results.total_pages as f64;
    }

    println!();
    println!("=== Test Results ===");
    println!();

    // Overall statistics
    println!("## Overall Statistics");
    println!("Total PDFs: {}", results.total_files);
    println!(
        "Successful: {} ({:.1}%)",
        results.successful,
        results.successful as f64 / results.total_files as f64 * 100.0
    );
    println!(
        "Failed: {} ({:.1}%)",
        results.failed,
        results.failed as f64 / results.total_files as f64 * 100.0
    );
    println!();

    // Performance
    println!("## Performance");
    println!("Total time: {:.2}s", elapsed.as_secs_f64());
    println!("Avg time per PDF: {:.1}ms", results.avg_time_per_pdf);
    println!("Total pages processed: {}", results.total_pages);
    if results.total_pages > 0 {
        println!("Avg time per page: {:.1}ms", results.avg_time_per_page);
    }
    println!();

    // Feature coverage
    println!("## Feature Coverage");
    println!(
        "PDFs with bookmarks: {} ({:.1}%)",
        results.with_bookmarks,
        results.with_bookmarks as f64 / results.successful as f64 * 100.0
    );
    println!(
        "PDFs with annotations: {} ({:.1}%)",
        results.with_annotations,
        results.with_annotations as f64 / results.successful as f64 * 100.0
    );
    println!(
        "PDFs with forms: {} ({:.1}%)",
        results.with_forms,
        results.with_forms as f64 / results.successful as f64 * 100.0
    );
    println!(
        "PDFs with images: {} ({:.1}%)",
        results.with_images,
        results.with_images as f64 / results.successful as f64 * 100.0
    );
    println!();

    // By category
    println!("## Results by Category");
    let mut categories: Vec<_> = results.results_by_category.iter().collect();
    categories.sort_by_key(|(name, _)| *name);

    for (category, stats) in categories {
        println!(
            "  {}: {}/{} ({:.1}%) - {} pages in {:.2}s",
            category,
            stats.successful,
            stats.total,
            stats.successful as f64 / stats.total as f64 * 100.0,
            stats.total_pages,
            stats.total_time_ms as f64 / 1000.0
        );
    }
    println!();

    // Failed files
    if !results.failed_files.is_empty() {
        println!("## Failed Files ({} files)", results.failed_files.len());
        for (idx, failed) in results.failed_files.iter().take(20).enumerate() {
            println!(
                "  {}. [{}] {} - {}",
                idx + 1,
                failed.category,
                Path::new(&failed.path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy(),
                truncate(&failed.error, 80)
            );
        }
        if results.failed_files.len() > 20 {
            println!("  ... and {} more", results.failed_files.len() - 20);
        }
        println!();
    }

    // Save results to JSON
    let output_path = root_dir.join("test_results.json");
    match serde_json::to_string_pretty(&results) {
        Ok(json) => {
            if let Err(e) = fs::write(&output_path, json) {
                eprintln!("Failed to write results: {}", e);
            } else {
                println!("Results saved to: {}", output_path.display());
            }
        },
        Err(e) => eprintln!("Failed to serialize results: {}", e),
    }

    // Exit with error code if too many failures
    let success_rate = results.successful as f64 / results.total_files as f64;
    if success_rate < 0.95 {
        eprintln!();
        eprintln!("WARNING: Success rate below 95% ({:.1}%)", success_rate * 100.0);
        std::process::exit(1);
    }
}

fn find_all_pdfs(root: &Path) -> Vec<(PathBuf, String)> {
    let mut pdfs = Vec::new();

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Category directory
                let category = path.file_name().unwrap().to_string_lossy().to_string();

                // Recursively find PDFs in subdirectories
                find_pdfs_in_category(&path, &category, &mut pdfs);
            }
        }
    }

    pdfs
}

fn find_pdfs_in_category(dir: &Path, category: &str, pdfs: &mut Vec<(PathBuf, String)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_file() && path.extension().map(|e| e == "pdf").unwrap_or(false) {
                pdfs.push((path, category.to_string()));
            } else if path.is_dir() {
                find_pdfs_in_category(&path, category, pdfs);
            }
        }
    }
}

fn test_pdf(path: &Path, category: &str, results: &mut TestResults) {
    let start = Instant::now();

    // Get or create category stats
    let stats = results
        .results_by_category
        .entry(category.to_string())
        .or_insert_with(CategoryStats::default);
    stats.total += 1;

    // Try to open and process PDF
    let mut doc = match PdfDocument::open(path) {
        Ok(doc) => {
            results.successful += 1;
            stats.successful += 1;
            doc
        },
        Err(e) => {
            results.failed += 1;
            stats.failed += 1;
            results.failed_files.push(FailedFile {
                path: path.display().to_string(),
                category: category.to_string(),
                error: format!("{}", e),
            });
            return;
        },
    };

    // Count pages
    if let Ok(page_count) = doc.page_count() {
        results.total_pages += page_count;
        stats.total_pages += page_count;

        // Test text extraction on first page
        let _ = doc.extract_text(0);
    }

    // Check for features
    if let Ok(Some(_)) = doc.get_outline() {
        results.with_bookmarks += 1;
    }

    if let Ok(annotations) = doc.get_annotations(0) {
        if !annotations.is_empty() {
            results.with_annotations += 1;
        }
    }

    // Check for images (first page only to save time)
    if let Ok(images) = doc.extract_images(0) {
        if !images.is_empty() {
            results.with_images += 1;
        }
    }

    let elapsed = start.elapsed();
    stats.total_time_ms += elapsed.as_millis() as u64;
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
