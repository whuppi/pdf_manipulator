use pdf_oxide::editor::{DocumentEditor, EditableDocument, SaveOptions};
use std::path::Path;

pub fn run(
    file: &Path,
    degrees: i32,
    pages: Option<&str>,
    output: Option<&Path>,
    password: Option<&str>,
) -> pdf_oxide::Result<()> {
    // Validate degrees
    let normalized = match degrees {
        90 | 180 | 270 | -90 => degrees,
        _ => {
            return Err(pdf_oxide::Error::InvalidOperation(format!(
                "Invalid rotation: {degrees}. Must be 90, 180, 270, or -90"
            )));
        },
    };

    let doc = super::open_doc(file, password)?;
    let page_count = doc.page_count()?;
    drop(doc);

    let page_indices = super::resolve_pages(pages, page_count)?;

    let mut editor = DocumentEditor::open(file)?;

    for &idx in &page_indices {
        editor.rotate_page_by(idx, normalized)?;
    }

    let out_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| super::output_beside(file, "_rotated.pdf"));

    editor.save_with_options(
        &out_path,
        SaveOptions {
            compress: true,
            garbage_collect: true,
            ..Default::default()
        },
    )?;

    eprintln!("Rotated {} page(s) by {degrees}° → {}", page_indices.len(), out_path.display());

    Ok(())
}
