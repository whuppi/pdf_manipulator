#!/usr/bin/env rust
//! Test the new outline and annotations features

use pdf_oxide::PdfDocument;
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: test_new_features <pdf_file>");
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    let mut doc = match PdfDocument::open(path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Failed to open PDF: {}", e);
            std::process::exit(1);
        },
    };

    println!("Testing new features for: {}", path.display());
    println!();

    // Test outline/bookmarks
    println!("=== BOOKMARKS/OUTLINE ===");
    match doc.get_outline() {
        Ok(Some(items)) => {
            println!("Found {} top-level bookmarks:", items.len());
            for item in items {
                print_outline_item(&item, 0);
            }
        },
        Ok(None) => {
            println!("No bookmarks found in this PDF");
        },
        Err(e) => {
            println!("Error reading bookmarks: {}", e);
        },
    }

    println!();

    // Test annotations
    println!("=== ANNOTATIONS ===");
    let page_count = match doc.page_count() {
        Ok(count) => count,
        Err(e) => {
            eprintln!("Failed to get page count: {}", e);
            std::process::exit(1);
        },
    };

    let mut total_annotations = 0;
    for page_idx in 0..page_count.min(5) {
        match doc.get_annotations(page_idx) {
            Ok(annotations) => {
                if !annotations.is_empty() {
                    println!("Page {}: {} annotations", page_idx, annotations.len());
                    total_annotations += annotations.len();

                    for annot in annotations {
                        println!(
                            "  Type: {:?}, Subtype: {:?}",
                            annot.annotation_type, annot.subtype
                        );
                        if let Some(contents) = annot.contents {
                            println!(
                                "    Contents: {}",
                                if contents.len() > 80 {
                                    format!("{}...", &contents[..80])
                                } else {
                                    contents
                                }
                            );
                        }
                        if let Some(author) = annot.author {
                            println!("    Author: {}", author);
                        }
                    }
                }
            },
            Err(e) => {
                println!("Error reading annotations for page {}: {}", page_idx, e);
            },
        }
    }

    if total_annotations == 0 {
        println!("No annotations found in first {} pages", page_count.min(5));
    } else {
        println!();
        println!("Total annotations in first {} pages: {}", page_count.min(5), total_annotations);
    }
}

fn print_outline_item(item: &pdf_oxide::OutlineItem, level: usize) {
    let indent = "  ".repeat(level);
    print!("{}- {}", indent, item.title);

    if let Some(dest) = &item.dest {
        match dest {
            pdf_oxide::Destination::PageIndex(idx) => print!(" → Page {}", idx),
            pdf_oxide::Destination::Named(name) => print!(" → {}", name),
        }
    }
    println!();

    for child in &item.children {
        print_outline_item(child, level + 1);
    }
}
