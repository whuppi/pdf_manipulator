//! Incremental save: the source bytes, unchanged, then one update section.
//!
//! ISO 32000-1 §7.5.6: an update appends the changed and new objects, a
//! cross-reference section for them and a trailer whose `/Prev` points at
//! the previous section. Every original byte stays where it was, so a
//! signature over them stays valid, and so does everything the edit meant
//! to remove: [`DocumentEditor::incremental_changes`] refuses those edits.
//!
//! The update's cross-reference section matches the source's last one: a
//! classic table after a table (a hybrid file ends in one), a
//! cross-reference stream after a stream, since a table cannot follow a
//! stream-only file. The source is copied through a read-buffer-sized
//! chunk, so memory stays bounded whatever its size.

use std::collections::HashMap;

use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::error::{Error, Result};
use crate::host::constants::READ_BUF_CAPACITY;
use crate::host::positioned_write::PositionedWrite;
use crate::object::{Object, ObjectRef};
use crate::writer::ObjectSerializer;

/// What one incremental save appends.
pub(crate) struct IncrementalChanges {
    /// Changed and new objects, sorted by id.
    pub objects: Vec<(u32, Object)>,
    /// The id the `/Info` dictionary is written at, when it changed.
    pub info_id: Option<u32>,
    /// The first object id the editor has not used.
    pub next_id: u32,
}

/// Why an incremental save failed.
pub(crate) enum IncrementalError {
    /// The edits cannot be appended; a full rewrite can save them. The
    /// bridge answers with its own status, so the caller can catch it by
    /// type and fall back.
    Refused(String),
    /// Anything else.
    Failed(Error),
}

impl From<Error> for IncrementalError {
    fn from(e: Error) -> Self {
        Self::Failed(e)
    }
}

impl From<std::io::Error> for IncrementalError {
    fn from(e: std::io::Error) -> Self {
        Self::Failed(e.into())
    }
}

impl From<IncrementalError> for Error {
    fn from(e: IncrementalError) -> Self {
        match e {
            IncrementalError::Refused(why) => Error::InvalidOperation(why),
            IncrementalError::Failed(e) => e,
        }
    }
}

fn refused(why: &str) -> IncrementalError {
    IncrementalError::Refused(format!(
        "incremental save refused: {why}; save with a full rewrite instead"
    ))
}

/// One appended update section, built without an editor: everything
/// [`write`] and the signer emit after the source's own bytes.
pub(crate) struct UpdateSection {
    /// File offset of the section's first byte (the source length, plus
    /// one when a newline is added after a source that does not end in
    /// one).
    pub base: u64,
    /// The whole section: the objects in the order given, then the xref
    /// table+trailer or xref stream, then startxref and %%EOF.
    pub bytes: Vec<u8>,
    /// Offset of each object within `bytes`, in the order given.
    pub object_offsets: Vec<usize>,
}

/// Builds one update section over the source `doc` holds. `objects`
/// carries already serialized `N G obj … endobj` bytes, sorted by id;
/// `info_id` is the id the `/Info` dictionary is written at, when it
/// changed; `next_id` is the first id the caller has not used.
pub(crate) fn build_update(
    doc: &PdfDocument,
    objects: &[(u32, Vec<u8>)],
    info_id: Option<u32>,
    next_id: u32,
) -> Result<UpdateSection> {
    let len = doc.source_len()?;
    let mut last = [0u8; 1];
    let newline = if len == 0 {
        false
    } else {
        doc.read_source_at(len - 1, &mut last)?;
        last[0] != b'\n' && last[0] != b'\r'
    };
    let base = len + u64::from(newline);

    let prev = last_startxref(doc, len)?;
    let mut head = [0u8; 4];
    doc.read_source_at(prev, &mut head)?;
    let table = &head == b"xref";

    let serializer = ObjectSerializer::compact();
    let mut bytes = Vec::new();
    let mut object_offsets = Vec::with_capacity(objects.len());
    let mut entries: Vec<(u32, u64, u16)> = Vec::with_capacity(objects.len() + 1);
    let mut digest = <sha2::Sha256 as sha2::Digest>::new();
    for (id, serialized) in objects {
        let gen = doc.object_generation(*id).unwrap_or(0);
        object_offsets.push(bytes.len());
        entries.push((*id, base + bytes.len() as u64, gen));
        sha2::Digest::update(&mut digest, serialized);
        bytes.extend_from_slice(serialized);
    }

    let mut trailer = carried_trailer(doc.trailer(), table);
    trailer.insert("Prev".into(), Object::Integer(prev as i64));
    // The first half of /ID names the document, the second this version
    // of it (ISO 32000-1 §14.4), so an update keeps one and renews the
    // other: from the first half and the appended bytes, so one edit of
    // one document always gives the same file.
    if let Some(Object::Array(id)) = trailer.get_mut("ID") {
        if id.len() == 2 {
            if let Some(first) = id[0].as_string() {
                sha2::Digest::update(&mut digest, first);
            }
            let hash = sha2::Digest::finalize(digest);
            id[1] = Object::String(hash[..16].to_vec());
        }
    }
    if let Some(id) = info_id {
        let gen = doc.object_generation(id).unwrap_or(0);
        trailer.insert("Info".into(), Object::Reference(ObjectRef::new(id, gen)));
    }
    let source_size = doc
        .trailer()
        .as_dict()
        .and_then(|d| d.get("Size"))
        .and_then(|s| s.as_integer())
        .unwrap_or(0)
        .max(0) as u32;
    let max_id = entries.iter().map(|e| e.0).max().unwrap_or(0);
    let mut size = source_size.max(max_id + 1).max(next_id);

    let xref_at = base + bytes.len() as u64;
    if table {
        trailer.insert("Size".into(), Object::Integer(size as i64));
        bytes.extend_from_slice(xref_table(&entries).as_bytes());
        bytes.extend_from_slice(b"trailer\n");
        bytes.extend_from_slice(&serializer.serialize(&Object::Dictionary(trailer)));
        bytes.extend_from_slice(b"\n");
    } else {
        // The stream lists itself, so it takes the next free id.
        let stream_id = size;
        size += 1;
        entries.push((stream_id, xref_at, 0));
        trailer.insert("Type".into(), Object::Name("XRef".into()));
        trailer.insert("Size".into(), Object::Integer(size as i64));
        let wide = if xref_at > u32::MAX as u64 { 8 } else { 4 };
        let (index, data) = xref_stream_rows(&entries, wide);
        trailer.insert(
            "W".into(),
            Object::Array(
                vec![1, wide as i64, 2]
                    .into_iter()
                    .map(Object::Integer)
                    .collect(),
            ),
        );
        trailer.insert(
            "Index".into(),
            Object::Array(index.into_iter().map(Object::Integer).collect()),
        );
        trailer.insert("Length".into(), Object::Integer(data.len() as i64));
        let stream = Object::stream(trailer, data);
        bytes.extend_from_slice(&serializer.serialize_indirect(stream_id, 0, &stream));
    }
    bytes.extend_from_slice(format!("startxref\n{xref_at}\n%%EOF\n").as_bytes());
    Ok(UpdateSection {
        base,
        bytes,
        object_offsets,
    })
}

/// Writes the source and one update section to `out`. A refusal is
/// decided before the first byte is written and leaves the editor as it was.
pub(crate) fn write(
    editor: &mut DocumentEditor,
    out: &mut impl PositionedWrite,
    options: &crate::editor::SaveOptions,
    cancel: &crate::cancel::CancelCheck,
) -> std::result::Result<(), IncrementalError> {
    if options.encryption.is_some() {
        return Err(refused("an encryption change rewrites every object"));
    }
    if let Some(why) = editor.incremental_refusal() {
        return Err(refused(why));
    }
    check_doc_mdp(editor)?;
    let changes = editor.incremental_objects(cancel)?;

    let doc = editor.source();
    let serializer = ObjectSerializer::compact();
    let mut objects: Vec<(u32, Vec<u8>)> = Vec::with_capacity(changes.objects.len());
    for (id, obj) in &changes.objects {
        cancel.check()?;
        let gen = doc.object_generation(*id).unwrap_or(0);
        objects.push((*id, serializer.serialize_indirect(*id, gen, obj)));
    }

    let len = doc.source_len()?;
    let mut chunk = vec![0u8; READ_BUF_CAPACITY];
    let mut at = 0u64;
    while at < len {
        cancel.check()?;
        let n = doc.read_source_at(at, &mut chunk)?;
        if n == 0 {
            return Err(Error::InvalidPdf("source ended before its length".into()).into());
        }
        out.write_all(&chunk[..n])?;
        at += n as u64;
    }
    // An edit that changes nothing appends nothing.
    if changes.objects.is_empty() {
        return Ok(());
    }
    let section = build_update(doc, &objects, changes.info_id, changes.next_id)?;
    if section.base > len {
        out.write_all(b"\n")?;
    }
    out.write_all(&section.bytes)?;
    Ok(())
}

/// The trailer entries an update carries over (ISO 32000-1 §7.5.6: all
/// but `/Prev`), less the ones this update writes itself and, after a
/// cross-reference stream, the stream's own keys.
fn carried_trailer(source: &Object, table: bool) -> HashMap<String, Object> {
    let own: &[&str] = if table {
        &["Size", "Prev", "XRefStm"]
    } else {
        &[
            "Size",
            "Prev",
            "XRefStm",
            "Type",
            "W",
            "Index",
            "Length",
            "Filter",
            "DecodeParms",
        ]
    };
    source
        .as_dict()
        .map(|d| {
            d.iter()
                .filter(|(k, _)| !own.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// A classic cross-reference section, one subsection per run of ids.
fn xref_table(entries: &[(u32, u64, u16)]) -> String {
    let mut out = String::from("xref\n");
    let mut i = 0;
    while i < entries.len() {
        let mut j = i + 1;
        while j < entries.len() && entries[j].0 == entries[j - 1].0 + 1 {
            j += 1;
        }
        out.push_str(&format!("{} {}\n", entries[i].0, j - i));
        for (_, offset, gen) in &entries[i..j] {
            out.push_str(&format!("{offset:010} {gen:05} n \n"));
        }
        i = j;
    }
    out
}

/// A cross-reference stream's `/Index` pairs and rows (`/W [1 wide 2]`):
/// type 1, a `wide`-byte offset, a two-byte generation.
fn xref_stream_rows(entries: &[(u32, u64, u16)], wide: usize) -> (Vec<i64>, Vec<u8>) {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| e.0);
    let mut index = Vec::new();
    let mut data = Vec::with_capacity(sorted.len() * (3 + wide));
    let mut i = 0;
    while i < sorted.len() {
        let mut j = i + 1;
        while j < sorted.len() && sorted[j].0 == sorted[j - 1].0 + 1 {
            j += 1;
        }
        index.push(sorted[i].0 as i64);
        index.push((j - i) as i64);
        for (_, offset, gen) in &sorted[i..j] {
            data.push(1);
            data.extend_from_slice(&offset.to_be_bytes()[8 - wide..]);
            data.extend_from_slice(&gen.to_be_bytes());
        }
        i = j;
    }
    (index, data)
}

/// The offset the source's last `startxref` names.
fn last_startxref(doc: &crate::document::PdfDocument, len: u64) -> Result<u64> {
    let tail_len = len.min(4096);
    let mut tail = vec![0u8; tail_len as usize];
    doc.read_source_at(len - tail_len, &mut tail)?;
    let key = b"startxref";
    let at = tail
        .windows(key.len())
        .rposition(|w| w == key)
        .ok_or_else(|| Error::InvalidPdf("incremental save: the source has no startxref".into()))?;
    let digits: String = tail[at + key.len()..]
        .iter()
        .skip_while(|b| b.is_ascii_whitespace())
        .take_while(|b| b.is_ascii_digit())
        .map(|&b| b as char)
        .collect();
    digits.parse().map_err(|_| {
        Error::InvalidPdf("incremental save: the source's startxref is unreadable".into())
    })
}

/// The source's DocMDP certification level (ISO 32000-1 §12.8.2.2),
/// when a certifying signature carries one: the /P of its
/// TransformParams, 2 when the params omit it.
pub(crate) fn doc_mdp_level(doc: &PdfDocument) -> Option<i64> {
    let resolve = |o: &Object| match o {
        Object::Reference(r) => doc.load_object(*r).ok(),
        other => Some(other.clone()),
    };
    doc.catalog()
        .ok()
        .and_then(|c| {
            c.as_dict()
                .and_then(|d| d.get("Perms"))
                .and_then(|p| resolve(p))
        })
        .and_then(|p| {
            p.as_dict()
                .and_then(|d| d.get("DocMDP"))
                .and_then(|s| resolve(s))
        })
        .and_then(|sig| {
            let refs = sig.as_dict()?.get("Reference").and_then(|r| resolve(r))?;
            refs.as_array()?
                .iter()
                .filter_map(|r| resolve(r))
                .find_map(|r| {
                    let d = r.as_dict()?;
                    (d.get("TransformMethod")?.as_name()? == "DocMDP").then(|| {
                        d.get("TransformParams")
                            .and_then(|p| resolve(p))
                            .and_then(|p| {
                                p.as_dict()
                                    .and_then(|d| d.get("P"))
                                    .and_then(|p| p.as_integer())
                            })
                            .unwrap_or(2)
                    })
                })
        })
}

/// Refuses an edit a certifying signature forbids (ISO 32000-1 §12.8.2.2).
/// Level 1 allows no change; levels 2 and 3 allow setting field values
/// (level 3 also annotations, which this save refuses anyway). So the only
/// question is level 1 against the rest, and whether the edit set values only.
fn check_doc_mdp(editor: &DocumentEditor) -> std::result::Result<(), IncrementalError> {
    let Some(level) = doc_mdp_level(editor.source()) else {
        return Ok(());
    };
    if level == 1 || !editor.incremental_value_only() {
        return Err(refused(&format!(
            "a certifying signature (DocMDP level {level}) forbids this change"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xref_table_groups_runs_and_keeps_entries_at_20_bytes() {
        let t = xref_table(&[(3, 10, 0), (4, 200, 1), (9, 3000, 0)]);
        let lines: Vec<&str> = t.split_inclusive('\n').collect();
        assert_eq!(lines[0], "xref\n");
        assert_eq!(lines[1], "3 2\n");
        assert_eq!(lines[2], "0000000010 00000 n \n");
        assert_eq!(lines[3], "0000000200 00001 n \n");
        assert_eq!(lines[4], "9 1\n");
        for entry in [lines[2], lines[3], lines[5]] {
            assert_eq!(entry.len(), 20, "{entry:?}");
        }
    }

    #[test]
    fn xref_stream_rows_follow_w_and_index() {
        let (index, data) = xref_stream_rows(&[(8, 0x0102_0304, 0), (7, 5, 2)], 4);
        assert_eq!(index, vec![7, 2]);
        assert_eq!(data, vec![1, 0, 0, 0, 5, 0, 2, 1, 1, 2, 3, 4, 0, 0]);
        let (_, wide) = xref_stream_rows(&[(1, 1 << 33, 0)], 8);
        assert_eq!(wide, vec![1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn carried_trailer_drops_the_stream_keys_after_a_stream() {
        let src = Object::Dictionary(HashMap::from([
            ("Root".to_string(), Object::Integer(1)),
            ("Prev".to_string(), Object::Integer(9)),
            ("W".to_string(), Object::Integer(0)),
            ("ID".to_string(), Object::Integer(2)),
        ]));
        let mut keys: Vec<String> = carried_trailer(&src, false).into_keys().collect();
        keys.sort();
        assert_eq!(keys, vec!["ID", "Root"]);
        assert!(carried_trailer(&src, true).contains_key("W"));
    }
}
