use pdf_oxide::geometry::Rect;
use std::path::Path;

pub fn run(
    file: &Path,
    area: Option<&str>,
    pages: Option<&str>,
    output: Option<&Path>,
    password: Option<&str>,
    json: bool,
) -> pdf_oxide::Result<()> {
    let doc = super::open_doc(file, password)?;
    let page_count = doc.page_count()?;
    let page_indices = super::resolve_pages(pages, page_count)?;

    let out_dir = output.unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(out_dir)?;

    let region = if let Some(area_str) = area {
        Some(parse_area(area_str)?)
    } else {
        None
    };

    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("img");

    let mut total_images = 0;
    let mut all_images = Vec::new();

    for &page_idx in &page_indices {
        // If area is specified, we filter before saving
        let images = if let Some(r) = region {
            let filtered_images = doc.extract_images_in_rect(page_idx, r)?;

            // Manually save filtered images
            let mut saved = Vec::new();
            let prefix = format!("{stem}_p{}", page_idx + 1);
            let mut idx = total_images + 1;

            for img in filtered_images {
                let (format, extension) = match img.data() {
                    pdf_oxide::extractors::ImageData::Jpeg(_) => {
                        (pdf_oxide::ImageFormat::Jpeg, "jpg")
                    },
                    _ => (pdf_oxide::ImageFormat::Png, "png"),
                };
                let filename = format!("{}_{:03}.{}", prefix, idx, extension);
                let filepath = out_dir.join(&filename);

                match format {
                    pdf_oxide::ImageFormat::Jpeg => img.save_as_jpeg(&filepath)?,
                    pdf_oxide::ImageFormat::Png => img.save_as_png(&filepath)?,
                }

                saved.push(pdf_oxide::ExtractedImageRef {
                    filename,
                    format,
                    width: img.width(),
                    height: img.height(),
                    bbox: img.bbox().cloned(),
                    rotation: img.rotation_degrees(),
                    matrix: img.matrix(),
                });
                idx += 1;
            }
            saved
        } else {
            let prefix = format!("{stem}_p{}", page_idx + 1);
            doc.extract_images_to_files(page_idx, out_dir, Some(&prefix), Some(total_images + 1))?
        };

        total_images += images.len();
        all_images.extend(images);
    }

    if json {
        let json_images: Vec<serde_json::Value> = all_images
            .iter()
            .map(|img| {
                let mut val = serde_json::json!({
                    "filename": img.filename,
                    "width": img.width,
                    "height": img.height,
                    "format": format!("{:?}", img.format),
                    "rotation": img.rotation,
                    "matrix": img.matrix,
                });
                if let Some(bbox) = img.bbox {
                    val.as_object_mut().unwrap().insert(
                        "bbox".to_string(),
                        serde_json::json!({
                            "x": bbox.x,
                            "y": bbox.y,
                            "width": bbox.width,
                            "height": bbox.height
                        }),
                    );
                }
                val
            })
            .collect();
        let json_out = serde_json::json!({
            "file": file.display().to_string(),
            "output_dir": out_dir.display().to_string(),
            "images_extracted": total_images,
            "area": area,
            "images": json_images,
        });
        super::write_output(&serde_json::to_string_pretty(&json_out).unwrap(), None)?;
    } else {
        eprintln!("Extracted {} image(s) to {}", total_images, out_dir.display());
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
