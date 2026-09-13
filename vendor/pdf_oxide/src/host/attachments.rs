//! Embedded-file (attachment) reads for the bridge.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! The catalog's `/Names /EmbeddedFiles` name tree is the only place a
//! document-level attachment lives (ISO 32000-1 §7.11.4). Listing walks
//! the tree and reads each file spec's declared metadata; it never
//! decodes a stream, so a listing costs nothing on a document with a
//! 50 MB attachment. Extraction decodes exactly the one stream asked for.

use crate::document::PdfDocument;
use crate::error::{Error, Result};
use crate::object::Object;

/// One attachment in wire vocabulary.
pub struct AttachmentRow {
    /// `/UF`, or `/F` when the file spec carries no Unicode name.
    pub name: String,
    /// `/EF /F /Params /Size`, or −1 when the file spec does not declare one.
    pub size: i64,
    /// `/Desc`, or `""` for none.
    pub description: String,
    /// The embedded stream's `/Subtype` as a MIME type, or `""` for none.
    pub mime_type: String,
}

/// Every attachment reachable through the catalog's embedded-files name
/// tree, in tree order. A document with no such tree has none.
pub fn list_attachments(doc: &PdfDocument) -> Result<Vec<AttachmentRow>> {
    let mut rows = Vec::new();
    for spec in file_specs(doc)? {
        let Some(dict) = spec.as_dict() else { continue };
        let Some(name) = spec_name(doc, dict) else { continue };
        let description = dict
            .get("Desc")
            .and_then(|d| doc.resolve_object(d).ok())
            .and_then(|d| d.as_string().map(text_string))
            .unwrap_or_default();
        let (size, mime_type) = match embedded_stream(doc, dict) {
            Some(stream) => stream_facts(doc, &stream),
            None => (-1, String::new()),
        };
        rows.push(AttachmentRow { name, size, description, mime_type });
    }
    Ok(rows)
}

/// The decoded bytes of the attachment named `name` (`/UF` or `/F`).
pub fn attachment_bytes(doc: &PdfDocument, name: &str) -> Result<Vec<u8>> {
    for spec in file_specs(doc)? {
        let Some(dict) = spec.as_dict() else { continue };
        if spec_name(doc, dict).as_deref() != Some(name) {
            continue;
        }
        let stream = embedded_stream(doc, dict)
            .ok_or_else(|| Error::InvalidPdf(format!("attachment {name} has no embedded stream")))?;
        return stream.decode_stream_data();
    }
    Err(Error::InvalidPdf(format!("no attachment named {name}")))
}

/// Collect every `/Filespec` in the catalog's embedded-files name tree,
/// resolving both node forms: a leaf's flat `/Names [key spec …]` array
/// and an intermediate node's `/Kids`.
fn file_specs(doc: &PdfDocument) -> Result<Vec<Object>> {
    let catalog = doc.catalog()?;
    let Some(cat_dict) = catalog.as_dict() else {
        return Ok(Vec::new());
    };
    let names = cat_dict.get("Names").and_then(|n| doc.resolve_object(n).ok());
    let Some(root) = names
        .as_ref()
        .and_then(|n| n.as_dict())
        .and_then(|d| d.get("EmbeddedFiles"))
        .and_then(|e| doc.resolve_object(e).ok())
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    collect_specs(doc, &root, &mut out, 0);
    Ok(out)
}

fn collect_specs(doc: &PdfDocument, node: &Object, out: &mut Vec<Object>, depth: u8) {
    // A malformed tree can point back at itself; 32 levels is far past
    // any real name tree's depth.
    if depth > 32 {
        return;
    }
    let Ok(node) = doc.resolve_object(node) else { return };
    let Some(dict) = node.as_dict() else { return };
    if let Some(names) = dict.get("Names").and_then(|n| doc.resolve_object(n).ok()) {
        if let Some(arr) = names.as_array() {
            // Flat [key1 spec1 key2 spec2 …] — the specs are the odd indices.
            let mut i = 1;
            while i < arr.len() {
                if let Ok(spec) = doc.resolve_object(&arr[i]) {
                    out.push(spec);
                }
                i += 2;
            }
        }
    }
    if let Some(kids) = dict.get("Kids").and_then(|k| doc.resolve_object(k).ok()) {
        if let Some(arr) = kids.as_array() {
            for kid in arr {
                collect_specs(doc, kid, out, depth + 1);
            }
        }
    }
}

fn spec_name(
    doc: &PdfDocument,
    dict: &std::collections::HashMap<String, Object>,
) -> Option<String> {
    dict.get("UF")
        .or_else(|| dict.get("F"))
        .and_then(|n| doc.resolve_object(n).ok())
        .and_then(|n| n.as_string().map(text_string))
}

fn embedded_stream(
    doc: &PdfDocument,
    dict: &std::collections::HashMap<String, Object>,
) -> Option<Object> {
    let ef = dict.get("EF").and_then(|e| doc.resolve_object(e).ok())?;
    let ef_dict = ef.as_dict()?.clone();
    let entry = ef_dict.get("F").or_else(|| ef_dict.get("UF"))?;
    doc.resolve_object(entry).ok()
}

/// `/Params /Size` and the `/Subtype` MIME type of an embedded-file stream.
fn stream_facts(doc: &PdfDocument, stream: &Object) -> (i64, String) {
    let Some(dict) = stream.as_dict() else {
        return (-1, String::new());
    };
    let size = dict
        .get("Params")
        .and_then(|p| doc.resolve_object(p).ok())
        .and_then(|p| p.as_dict().and_then(|d| d.get("Size").cloned()))
        .and_then(|s| s.as_integer())
        .unwrap_or(-1);
    let mime = dict
        .get("Subtype")
        .and_then(|s| s.as_name().map(|n| n.to_string()))
        .unwrap_or_default();
    (size, mime)
}

/// A PDF text string as Rust text. Attachment names and descriptions are
/// text strings (§7.9.2.2): UTF-16BE when they carry the byte-order mark,
/// PDFDoc-encoded bytes otherwise.
fn text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}
