use pdf_oxide::editor::form_fields::FormFieldValue;
use pdf_oxide::editor::{DocumentEditor, EditableDocument, SaveOptions};
use pdf_oxide::geometry::Rect;
use std::path::Path;

#[allow(clippy::too_many_arguments)] // CLI command signature mirrors clap arg parser
pub fn run(
    file: &Path,
    fill: Option<&str>,
    export: Option<&str>,
    area: Option<&str>,
    pages: Option<&str>,
    output: Option<&Path>,
    password: Option<&str>,
    json: bool,
) -> pdf_oxide::Result<()> {
    let _ = password;
    let mut editor = DocumentEditor::open(file)?;

    let region = if let Some(area_str) = area {
        Some(parse_area(area_str)?)
    } else {
        None
    };

    let page_count = editor.source_mut().page_count()?;
    let page_indices = super::resolve_pages(pages, page_count)?;

    // Export mode
    if let Some(format) = export {
        let export_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            Path::new(&format!("{stem}.{format}")).to_path_buf()
        });

        match format {
            "fdf" => editor.export_form_data_fdf(&export_path)?,
            "xfdf" => editor.export_form_data_xfdf(&export_path)?,
            _ => unreachable!(), // clap validates this
        }

        eprintln!("Exported form data to {}", export_path.display());
        return Ok(());
    }

    // Fill mode
    if let Some(fill_str) = fill {
        let pairs = parse_fill_pairs(fill_str)?;

        for (name, value) in &pairs {
            editor.set_form_field_value(name, FormFieldValue::Text(value.clone()))?;
        }

        let out_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            Path::new(&format!("{stem}_filled.pdf")).to_path_buf()
        });

        editor.save_with_options(
            &out_path,
            SaveOptions {
                compress: true,
                garbage_collect: true,
                ..Default::default()
            },
        )?;

        eprintln!("Filled {} field(s) → {}", pairs.len(), out_path.display());
        return Ok(());
    }

    // List mode (default)
    let mut fields = editor.get_form_fields()?;

    // Filter by pages
    fields.retain(|f| page_indices.contains(&f.page_index()));

    // Filter by area
    if let Some(r) = region {
        fields.retain(|f| {
            if let Some(bounds) = f.bounds() {
                bounds.intersects(&r)
            } else {
                false
            }
        });
    }

    if fields.is_empty() {
        if json {
            super::write_output("[]", None)?;
        } else {
            eprintln!("No matching form fields found in {}", file.display());
        }
        return Ok(());
    }

    if json {
        let json_fields: Vec<serde_json::Value> = fields
            .iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.name(),
                    "type": format!("{:?}", f.field_type()),
                    "value": format_value(&f.value()),
                    "page": f.page_index() + 1,
                })
            })
            .collect();
        let out = serde_json::to_string_pretty(&json_fields).unwrap();
        super::write_output(&out, None)?;
    } else {
        eprintln!("Found {} matching form field(s):", fields.len());
        for f in &fields {
            let ftype = f
                .field_type()
                .map(|t| format!("{t:?}"))
                .unwrap_or_else(|| "Unknown".to_string());
            let val = format_value(&f.value());
            println!("  {} [{}] = {}", f.name(), ftype, val);
        }
    }

    Ok(())
}

fn format_value(val: &FormFieldValue) -> String {
    match val {
        FormFieldValue::Text(s) => format!("\"{s}\""),
        FormFieldValue::Boolean(b) => b.to_string(),
        FormFieldValue::Choice(s) => format!("\"{s}\""),
        FormFieldValue::MultiChoice(v) => format!("{v:?}"),
        FormFieldValue::None => "(empty)".to_string(),
    }
}

fn parse_fill_pairs(s: &str) -> pdf_oxide::Result<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            pairs.push((key.trim().to_string(), value.trim().to_string()));
        } else {
            return Err(pdf_oxide::Error::InvalidOperation(format!(
                "Invalid fill pair: '{part}'. Expected key=value"
            )));
        }
    }
    Ok(pairs)
}

fn parse_area(s: &str) -> pdf_oxide::Result<Rect> {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.len() != 4 {
        return Err(pdf_oxide::Error::InvalidOperation(
            "Area must be provided as x,y,width,height".to_string(),
        ));
    }

    let x = parts[0].parse::<f32>().map_err(|_| {
        pdf_oxide::Error::InvalidOperation(format!("Invalid x coordinate: {}", parts[0]))
    })?;
    let y = parts[1].parse::<f32>().map_err(|_| {
        pdf_oxide::Error::InvalidOperation(format!("Invalid y coordinate: {}", parts[1]))
    })?;
    let w = parts[2]
        .parse::<f32>()
        .map_err(|_| pdf_oxide::Error::InvalidOperation(format!("Invalid width: {}", parts[2])))?;
    let h = parts[3]
        .parse::<f32>()
        .map_err(|_| pdf_oxide::Error::InvalidOperation(format!("Invalid height: {}", parts[3])))?;

    Ok(Rect::new(x, y, w, h))
}
