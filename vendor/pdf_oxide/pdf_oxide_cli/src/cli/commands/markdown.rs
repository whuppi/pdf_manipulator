use pdf_oxide::converters::ConversionOptions;
use std::path::Path;

pub fn run(
    file: &Path,
    pages: Option<&str>,
    output: Option<&Path>,
    password: Option<&str>,
    json: bool,
) -> pdf_oxide::Result<()> {
    let doc = super::open_doc(file, password)?;
    let page_count = doc.page_count()?;
    let page_indices = super::resolve_pages(pages, page_count)?;
    let options = ConversionOptions::default();

    let mut results: Vec<String> = Vec::new();
    for &page_idx in &page_indices {
        let md = doc.to_markdown(page_idx, &options)?;
        results.push(md);
    }

    if json {
        let json_out = serde_json::json!({
            "file": file.display().to_string(),
            "pages": page_indices.iter().map(|p| p + 1).collect::<Vec<_>>(),
            "markdown": results,
        });
        super::write_output(&serde_json::to_string_pretty(&json_out).unwrap(), output)?;
    } else {
        let combined = results.join("\n\n---\n\n");
        super::write_output(&combined, output)?;
    }

    Ok(())
}
