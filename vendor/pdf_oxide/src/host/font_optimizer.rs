//! Standard 14 font unembedding for the PDF editor.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! The PDF spec requires every reader to have the Standard 14 fonts
//! built-in. When a PDF embeds one of these, the font program bytes
//! are redundant. Removing them saves 20-400KB per font without
//! changing rendering.
//!
//! Only touches fonts whose PostScript name matches one of the 14.
//! Never touches non-standard fonts — that makes text invisible.
//!
//! Standard 14 fonts (ISO 32000-1:2008 §9.6.2.2):
//!   Courier, Courier-Bold, Courier-Oblique, Courier-BoldOblique
//!   Helvetica, Helvetica-Bold, Helvetica-Oblique, Helvetica-BoldOblique
//!   Times-Roman, Times-Bold, Times-Italic, Times-BoldItalic
//!   Symbol, ZapfDingbats

use crate::document::PdfDocument;
use crate::error::Result;
use crate::object::{Object, ObjectRef};
use std::collections::HashMap;

const STANDARD_14: &[&str] = &[
    "Courier", "Courier-Bold", "Courier-Oblique", "Courier-BoldOblique",
    "Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Helvetica-BoldOblique",
    "Times-Roman", "Times-Bold", "Times-Italic", "Times-BoldItalic",
    "Symbol", "ZapfDingbats",
];

/// Remove embedded font programs for Standard 14 fonts.
///
/// Walks all FontDescriptor objects. If the font's /FontName matches
/// one of the 14, removes /FontFile, /FontFile2, and /FontFile3.
/// Returns the number of font programs removed.
pub fn unembed_standard_fonts(
    source: &PdfDocument,
    modified_objects: &mut HashMap<u32, Object>,
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

        let dict = match &obj {
            Object::Dictionary(d) => d,
            _ => continue,
        };

        if dict.get("Type").and_then(|o| o.as_name()) != Some("FontDescriptor") {
            continue;
        }

        let font_name = match dict.get("FontName").and_then(|o| o.as_name()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Strip subset prefix "ABCDEF+" if present
        let base_name = if font_name.len() > 7 && font_name.as_bytes()[6] == b'+' {
            &font_name[7..]
        } else {
            &font_name
        };

        if !STANDARD_14.contains(&base_name) {
            continue;
        }

        let has_font_file = dict.contains_key("FontFile")
            || dict.contains_key("FontFile2")
            || dict.contains_key("FontFile3");

        if !has_font_file {
            continue;
        }

        let mut new_dict = dict.clone();
        new_dict.remove("FontFile");
        new_dict.remove("FontFile2");
        new_dict.remove("FontFile3");

        modified_objects.insert(obj_id, Object::Dictionary(new_dict));
        count += 1;
    }

    Ok(count)
}
