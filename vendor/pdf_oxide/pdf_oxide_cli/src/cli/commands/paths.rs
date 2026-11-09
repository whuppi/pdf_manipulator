use pdf_oxide::geometry::Rect;
use std::path::Path;

pub fn run(
    file: &Path,
    format: &str,
    area: Option<&str>,
    pages: Option<&str>,
    output: Option<&Path>,
    password: Option<&str>,
    json: bool,
) -> pdf_oxide::Result<()> {
    let doc = super::open_doc(file, password)?;
    let page_count = doc.page_count()?;
    let page_indices = super::resolve_pages(pages, page_count)?;

    let region = if let Some(area_str) = area {
        Some(parse_area(area_str)?)
    } else {
        None
    };

    let mut all_pages = Vec::new();

    for &page_idx in &page_indices {
        let paths = match format {
            "rects" => {
                let list = doc.extract_rects(page_idx)?;
                if let Some(r) = region {
                    list.into_iter().filter(|p| p.bbox.intersects(&r)).collect()
                } else {
                    list
                }
            },
            "lines" => {
                let list = doc.extract_lines(page_idx)?;
                if let Some(r) = region {
                    list.into_iter().filter(|p| p.bbox.intersects(&r)).collect()
                } else {
                    list
                }
            },
            _ => {
                if let Some(r) = region {
                    doc.extract_paths_in_rect(page_idx, r)?
                } else {
                    doc.extract_paths(page_idx)?
                }
            },
        };

        if json {
            all_pages.push(serde_json::json!({
                "page": page_idx + 1,
                "paths": paths,
            }));
        } else {
            // For non-JSON, just print a summary or list
            println!("Page {}: {} paths found", page_idx + 1, paths.len());
            if format != "json" {
                for (i, path) in paths.iter().enumerate() {
                    println!("  {}. {:?} at {:?}", i + 1, format, path.bbox);
                }
            }
        }
    }

    if json {
        let json_out = serde_json::json!({
            "file": file.display().to_string(),
            "format": format,
            "area": area,
            "pages": all_pages,
        });
        super::write_output(&serde_json::to_string_pretty(&json_out).unwrap(), output)?;
    }

    Ok(())
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
