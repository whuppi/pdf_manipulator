use pdf_oxide::editor::{DocumentEditor, EditableDocument, SaveOptions};
use pdf_oxide::geometry::Rect;
use pdf_oxide::writer::WatermarkAnnotation;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    text: &str,
    opacity: f32,
    rotation: f32,
    font_size: f32,
    color: Option<&str>,
    pages: Option<&str>,
    output: Option<&Path>,
    password: Option<&str>,
) -> pdf_oxide::Result<()> {
    let _ = password;

    let doc = super::open_doc(file, None)?;
    let page_count = doc.page_count()?;
    drop(doc);

    let page_indices = super::resolve_pages(pages, page_count)?;

    // Parse color if provided
    let color_rgb = if let Some(c) = color {
        let parts: Vec<f32> = c
            .split(',')
            .map(|s| s.trim().parse::<f32>().unwrap_or(0.0))
            .collect();
        if parts.len() != 3 {
            return Err(pdf_oxide::Error::InvalidOperation(
                "Color must be R,G,B (e.g. '0.8,0,0')".to_string(),
            ));
        }
        Some((parts[0], parts[1], parts[2]))
    } else {
        None
    };

    let mut editor = DocumentEditor::open(file)?;

    for &idx in &page_indices {
        let page_info = editor.get_page_info(idx)?;

        // Build watermark covering the full page
        let wm = match text.to_uppercase().as_str() {
            "CONFIDENTIAL" => WatermarkAnnotation::confidential(),
            "DRAFT" => WatermarkAnnotation::draft(),
            "SAMPLE" => WatermarkAnnotation::sample(),
            "DO NOT COPY" => WatermarkAnnotation::do_not_copy(),
            _ => WatermarkAnnotation::new(text),
        };

        let mut wm = wm
            .with_rect(Rect::new(0.0, 0.0, page_info.width, page_info.height))
            .with_opacity(opacity)
            .with_rotation(rotation)
            .with_font("Helvetica", font_size);

        if let Some((r, g, b)) = color_rgb {
            wm = wm.with_color(r, g, b);
        }

        editor.edit_page(idx, |page| {
            page.add_annotation(wm);
            Ok(())
        })?;
    }

    let out_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| super::output_beside(file, "_watermarked.pdf"));

    editor.save_with_options(
        &out_path,
        SaveOptions {
            compress: true,
            garbage_collect: true,
            ..Default::default()
        },
    )?;

    eprintln!(
        "Added watermark '{}' to {} page(s) → {}",
        text,
        page_indices.len(),
        out_path.display()
    );

    Ok(())
}
