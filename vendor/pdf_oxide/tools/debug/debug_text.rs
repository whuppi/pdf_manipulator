//! Debug tool to examine raw text extraction
//!
//! This tool extracts raw character data from a PDF and displays it
//! to help diagnose text extraction quality issues.

use pdf_oxide::document::PdfDocument;
use pdf_oxide::error::Result;
use pdf_oxide::layout::FontWeight;
use std::env;

fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <pdf_file>", args[0]);
        std::process::exit(1);
    }

    let pdf_path = &args[1];
    println!("Opening PDF: {}", pdf_path);

    let mut pdf = PdfDocument::open(pdf_path)?;
    let page_count = pdf.page_count()?;
    println!("Page count: {}", page_count);
    println!();

    // Extract from first page only
    let page_num = 0;
    println!("=== Page {} ===", page_num + 1);

    let chars = pdf.extract_positioned_chars(page_num)?;
    println!("Extracted {} characters", chars.len());

    // Count bold vs normal
    let bold_count = chars
        .iter()
        .filter(|ch| ch.font_weight == FontWeight::Bold)
        .count();
    let normal_count = chars.len() - bold_count;
    println!(
        "  Bold: {} ({:.1}%)",
        bold_count,
        100.0 * bold_count as f64 / chars.len() as f64
    );
    println!(
        "  Normal: {} ({:.1}%)",
        normal_count,
        100.0 * normal_count as f64 / chars.len() as f64
    );
    println!();

    // Show first 200 characters with details
    println!("First 200 characters with positions:");
    println!(
        "{:<5} {:<10} {:<10} {:<10} {:<15} {:<6} {}",
        "Index", "Char", "X", "Y", "Font", "Size", "Weight"
    );
    println!("{}", "-".repeat(90));

    for (i, ch) in chars.iter().take(200).enumerate() {
        let char_repr = if ch.char.is_control() || ch.char == ' ' {
            format!("U+{:04X}", ch.char as u32)
        } else {
            format!("'{}'", ch.char)
        };

        let weight = match ch.font_weight {
            FontWeight::Thin => "Thin",
            FontWeight::ExtraLight => "XLight",
            FontWeight::Light => "Light",
            FontWeight::Normal => "Normal",
            FontWeight::Medium => "Medium",
            FontWeight::SemiBold => "SemiBd",
            FontWeight::Bold => "Bold",
            FontWeight::ExtraBold => "XBold",
            FontWeight::Black => "Black",
        };

        println!(
            "{:<5} {:<10} {:<10.2} {:<10.2} {:<15} {:<6.1} {}",
            i,
            char_repr,
            ch.bbox.x,
            ch.bbox.y,
            ch.font_name.chars().take(12).collect::<String>(),
            ch.font_size,
            weight
        );
    }
    println!();

    // Show actual text (first 1000 characters)
    println!("Actual extracted text (first 1000 characters):");
    println!("{}", "-".repeat(80));
    let text: String = chars.iter().take(1000).map(|ch| ch.char).collect();
    println!("{}", text);
    println!();

    // Show character position statistics
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for ch in &chars {
        min_x = min_x.min(ch.bbox.x);
        max_x = max_x.max(ch.bbox.x);
        min_y = min_y.min(ch.bbox.y);
        max_y = max_y.max(ch.bbox.y);
    }

    println!("Position statistics:");
    println!("  X range: {:.2} to {:.2} (width: {:.2})", min_x, max_x, max_x - min_x);
    println!("  Y range: {:.2} to {:.2} (height: {:.2})", min_y, max_y, max_y - min_y);
    println!();

    // Group by Y coordinate (tolerance 1.0) to see line structure
    let mut y_positions: Vec<f32> = chars.iter().map(|ch| ch.bbox.y).collect();
    y_positions.sort_by(|a, b| a.partial_cmp(b).unwrap());
    y_positions.dedup_by(|a, b| (*a - *b).abs() < 1.0);

    println!("Detected {} unique Y positions (lines)", y_positions.len());
    println!("First 20 Y positions:");
    for (i, y) in y_positions.iter().take(20).enumerate() {
        let chars_on_line: Vec<_> = chars
            .iter()
            .filter(|ch| (ch.bbox.y - y).abs() < 1.0)
            .collect();
        let text: String = chars_on_line.iter().map(|ch| ch.char).collect();
        println!(
            "  Line {}: Y={:.2}, {} chars: {}",
            i,
            y,
            chars_on_line.len(),
            text.chars().take(80).collect::<String>()
        );
    }

    Ok(())
}
