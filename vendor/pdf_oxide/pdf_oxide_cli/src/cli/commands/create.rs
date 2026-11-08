use pdf_oxide::api::Pdf;
use std::path::Path;

pub fn run(file: &Path, from: &str, output: Option<&Path>) -> pdf_oxide::Result<()> {
    let content = std::fs::read_to_string(file)?;

    let mut pdf = match from {
        "markdown" => Pdf::from_markdown(&content)?,
        "html" => Pdf::from_html(&content)?,
        "text" => Pdf::from_text(&content)?,
        _ => {
            return Err(pdf_oxide::Error::InvalidOperation(format!(
                "Unknown format: '{from}'. Use --from markdown, --from html, or --from text"
            )))
        },
    };

    let out_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        std::path::PathBuf::from(format!("{stem}.pdf"))
    });

    pdf.save(&out_path)?;
    eprintln!("Created {}", out_path.display());

    Ok(())
}
