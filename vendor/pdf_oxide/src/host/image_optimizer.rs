//! Image optimization for the PDF editor.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Walks all Image XObjects in the document. For each non-JPEG image
//! that meets the minimum dimension threshold, decodes the stream,
//! re-encodes as JPEG, and replaces the stream ONLY if JPEG is smaller.
//!
//! Rules:
//!   - Only /DeviceRGB, /DeviceGray, /DeviceCMYK color spaces
//!   - Only 8 bits per component
//!   - Skip images already JPEG (/DCTDecode)
//!   - Skip images below configurable minimum dimensions (default 128px)
//!   - CMYK → RGB conversion before JPEG encode
//!   - Keep original if JPEG is not smaller
//!   - Never resample (dimensions unchanged)
//!   - /SMask (transparency mask) is a separate object — never touched
//!
//! Memory: one image decoded + encoded at a time. O(largest single image).

#[cfg(feature = "rendering")]
use crate::document::PdfDocument;
#[cfg(feature = "rendering")]
use crate::error::Result;
#[cfg(feature = "rendering")]
use crate::object::{Object, ObjectRef};
#[cfg(feature = "rendering")]
use std::collections::HashMap;

/// Optimize images in the document's object graph.
///
/// `min_size` is the minimum width/height in pixels. Images below this
/// threshold are skipped (too small for JPEG recompression to help).
/// Default 128. Pass 0 to process all images regardless of size.
///
/// Returns the number of images recompressed. Modified objects are
/// inserted into `modified_objects` — the caller (DocumentEditor)
/// commits them on the next save.
#[cfg(feature = "rendering")]
pub fn optimize_images(
    source: &PdfDocument,
    modified_objects: &mut HashMap<u32, Object>,
    jpeg_quality: u8,
    min_size: u32,
) -> Result<usize> {
    let mut count = 0;

    for obj_id in source.all_object_ids() {
        let obj = if let Some(m) = modified_objects.get(&obj_id) {
            m.clone()
        } else if let Ok(loaded) = source.load_object(ObjectRef::new(obj_id, 0)) {
            loaded
        } else {
            continue;
        };

        let (dict, raw_data) = match &obj {
            Object::Stream { dict, data } => (dict.clone(), data.to_vec()),
            _ => continue,
        };

        if dict.get("Subtype").and_then(|o| o.as_name()) != Some("Image") {
            continue;
        }

        let width = match dict.get("Width").and_then(|o| o.as_integer()) {
            Some(w) => w as u32,
            None => continue,
        };
        let height = match dict.get("Height").and_then(|o| o.as_integer()) {
            Some(h) => h as u32,
            None => continue,
        };

        if width < min_size || height < min_size {
            continue;
        }

        let bpc = dict.get("BitsPerComponent")
            .and_then(|o| o.as_integer())
            .unwrap_or(8);
        if bpc != 8 {
            continue;
        }

        let filter = dict.get("Filter").and_then(|o| match o {
            Object::Name(n) => Some(n.as_str()),
            Object::Array(arr) if arr.len() == 1 => arr[0].as_name(),
            _ => None,
        }).unwrap_or("");

        if filter == "DCTDecode" {
            continue;
        }

        let colorspace = dict.get("ColorSpace")
            .and_then(|o| o.as_name())
            .unwrap_or("");
        let components: u8 = match colorspace {
            "DeviceGray" => 1,
            "DeviceRGB" => 3,
            "DeviceCMYK" => 4,
            _ => continue,
        };

        let decoded = match obj.decode_stream_data() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let original_size = raw_data.len();

        let jpeg_data = match encode_jpeg(&decoded, width, height, components, jpeg_quality) {
            Some(d) => d,
            None => continue,
        };

        if jpeg_data.len() >= original_size {
            continue;
        }

        let mut new_dict = dict.clone();
        new_dict.insert("Filter".to_string(), Object::Name("DCTDecode".to_string()));
        new_dict.insert("Length".to_string(), Object::Integer(jpeg_data.len() as i64));
        new_dict.remove("DecodeParms");

        if components == 4 {
            new_dict.insert("ColorSpace".to_string(), Object::Name("DeviceRGB".to_string()));
        }

        modified_objects.insert(obj_id, Object::Stream {
            dict: new_dict,
            data: bytes::Bytes::from(jpeg_data),
        });
        count += 1;
    }

    Ok(count)
}

#[cfg(feature = "rendering")]
fn encode_jpeg(data: &[u8], width: u32, height: u32, components: u8, quality: u8) -> Option<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::ExtendedColorType;
    use std::io::Cursor;

    let (pixels, color_type) = match components {
        1 => (data.to_vec(), ExtendedColorType::L8),
        3 => (data.to_vec(), ExtendedColorType::Rgb8),
        4 => {
            // CMYK → RGB (Rust image crate cannot encode CMYK JPEG).
            // QPDF uses libjpeg's JCS_CMYK — we don't have that in Rust.
            let mut rgb = Vec::with_capacity((width * height * 3) as usize);
            for chunk in data.chunks_exact(4) {
                let c = chunk[0] as f32 / 255.0;
                let m = chunk[1] as f32 / 255.0;
                let y = chunk[2] as f32 / 255.0;
                let k = chunk[3] as f32 / 255.0;
                rgb.push(((1.0 - c) * (1.0 - k) * 255.0) as u8);
                rgb.push(((1.0 - m) * (1.0 - k) * 255.0) as u8);
                rgb.push(((1.0 - y) * (1.0 - k) * 255.0) as u8);
            }
            (rgb, ExtendedColorType::Rgb8)
        }
        _ => return None,
    };

    let mut buf = Cursor::new(Vec::new());
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder.encode(&pixels, width, height, color_type).ok()?;
    Some(buf.into_inner())
}
