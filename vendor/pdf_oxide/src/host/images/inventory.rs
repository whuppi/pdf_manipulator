//! Which images a document draws, and where.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Two views. `page_image_xobject_names` answers "which `Do` names on this
//! page are images" for `editorPageImages` (upstream's `get_page_images`
//! records every `Do`, forms included). `document_images` groups every
//! painted image XObject across the given pages by object, with one
//! placement per `Do`, for the reducer: a shared image is decided once.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::content::Matrix;
use crate::document::PdfDocument;
use crate::error::Result;
use crate::object::{Object, ObjectRef};

/// One painted image XObject and every place it is drawn.
pub struct Entry {
    /// The image XObject.
    pub object: ObjectRef,
    /// One CTM per `Do`, forms composed in, across all given pages.
    pub placements: Vec<Matrix>,
}

/// Every image XObject painted on `source_pages` (source page indices),
/// grouped by object, ascending object id. Inline images have no object
/// and are not listed.
pub fn document_images(source: &PdfDocument, source_pages: &[usize]) -> Result<Vec<Entry>> {
    let mut by_object: BTreeMap<u32, Entry> = BTreeMap::new();
    for &page in source_pages {
        for handle in source.page_image_handles(page)? {
            let Some(object) = handle.object_ref() else {
                continue;
            };
            by_object
                .entry(object.id)
                .or_insert_with(|| Entry {
                    object,
                    placements: Vec::new(),
                })
                .placements
                .push(handle.ctm());
        }
    }
    Ok(by_object.into_values().collect())
}

/// The names in the page's `/XObject` resources whose target stream has
/// `/Subtype /Image`.
///
/// `/Resources` is inheritable (PDF 32000-1 §7.7.3.4): when the page has
/// none, the nearest ancestor `Pages` node that has one applies.
/// Malformed entries (a name that resolves to nothing, or to a
/// non-stream) are not images and are skipped, never an error.
pub fn page_image_xobject_names(
    source: &PdfDocument,
    page_ref: ObjectRef,
) -> Result<HashSet<String>> {
    let mut names = HashSet::new();
    let Some(resources) = inherited_resources(source, page_ref)? else {
        return Ok(names);
    };
    let Some(xobjects) = resources
        .get("XObject")
        .and_then(|x| xobject_dict(source, x))
    else {
        return Ok(names);
    };
    for (name, target) in xobjects {
        let Ok(obj) = source.resolve_object(&target) else {
            continue;
        };
        let Object::Stream { dict, .. } = obj else {
            continue;
        };
        if dict.get("Subtype").and_then(|s| s.as_name()) == Some("Image") {
            names.insert(name);
        }
    }
    Ok(names)
}

/// The page's own `/Resources`, else the first one found walking `/Parent`.
fn inherited_resources(
    source: &PdfDocument,
    page_ref: ObjectRef,
) -> Result<Option<HashMap<String, Object>>> {
    let mut node = source.load_object(page_ref)?;
    // A cycle in /Parent is malformed; the page tree is never deeper than
    // this in practice, and the bound keeps a hostile file finite.
    for _ in 0..64 {
        let Some(dict) = node.as_dict() else {
            return Ok(None);
        };
        if let Some(resources) = dict.get("Resources") {
            let resolved = source.resolve_object(resources)?;
            return Ok(resolved.as_dict().cloned());
        }
        match dict.get("Parent").and_then(|p| p.as_reference()) {
            Some(parent) => node = source.load_object(parent)?,
            None => return Ok(None),
        }
    }
    Ok(None)
}

fn xobject_dict(source: &PdfDocument, value: &Object) -> Option<HashMap<String, Object>> {
    source.resolve_object(value).ok()?.as_dict().cloned()
}
