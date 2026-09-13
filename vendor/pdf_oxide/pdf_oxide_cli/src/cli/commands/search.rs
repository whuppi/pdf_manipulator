use pdf_oxide::search::{SearchOptions, TextSearcher};
use std::path::Path;

pub fn run(
    file: &Path,
    pattern: &str,
    ignore_case: bool,
    pages: Option<&str>,
    password: Option<&str>,
    json: bool,
) -> pdf_oxide::Result<()> {
    let doc = super::open_doc(file, password)?;
    let page_count = doc.page_count()?;

    let selected_pages = if let Some(ranges) = pages {
        Some(super::resolve_pages(Some(ranges), page_count)?)
    } else {
        None
    };

    let page_range = selected_pages.as_ref().map(|indices| {
        let min = *indices.iter().min().unwrap_or(&0);
        let max = *indices.iter().max().unwrap_or(&0);
        (min, max)
    });

    let options = SearchOptions {
        case_insensitive: ignore_case,
        page_range,
        ..Default::default()
    };

    let mut results = TextSearcher::search(&doc, pattern, &options)?;

    // Filter to only the requested pages (page_range is a contiguous min..max
    // but the user may have specified non-contiguous pages like "1,3,7")
    if let Some(ref indices) = selected_pages {
        results.retain(|r| indices.contains(&r.page));
    }

    if json {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "page": r.page + 1,
                    "text": r.text,
                    "start_index": r.start_index,
                    "end_index": r.end_index,
                })
            })
            .collect();
        let json_out = serde_json::json!({
            "file": file.display().to_string(),
            "pattern": pattern,
            "matches": results.len(),
            "results": json_results,
        });
        super::write_output(&serde_json::to_string_pretty(&json_out).unwrap(), None)?;
    } else if results.is_empty() {
        eprintln!("No matches found for '{pattern}'");
    } else {
        eprintln!("Found {} match(es) for '{pattern}':", results.len());
        for r in &results {
            println!("  Page {}: \"{}\"", r.page + 1, r.text);
        }
    }

    Ok(())
}
