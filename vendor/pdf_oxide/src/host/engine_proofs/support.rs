//! Builders the engine proofs share.

use crate::editor::DocumentEditor;

pub(super) use crate::host::cpu_clock::{resolution_secs, thread_cpu_secs};

/// Objects are numbered from 1 in the order given; offsets are computed.
pub(super) fn pdf_from_objects(objs: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for o in objs {
        offsets.push(out.len());
        out.extend_from_slice(o);
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n", objs.len() + 1)
            .as_bytes(),
    );
    out
}

pub(super) fn stream_object(id: u32, dict: &str, data: &[u8]) -> Vec<u8> {
    [
        format!("{id} 0 obj\n<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes(),
        data.to_vec(),
        b"\nendstream\nendobj\n".to_vec(),
    ]
    .concat()
}

pub(super) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// An editor opened from a file written for it, the way a caller opens one
/// from disk.
pub(super) fn open_editor_from_bytes(bytes: &[u8], tag: &str) -> DocumentEditor {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("pdf_oxide_{tag}_{}_{id}.pdf", std::process::id()));
    std::fs::write(&tmp, bytes).unwrap();
    let editor = DocumentEditor::open(&tmp).unwrap();
    let _ = std::fs::remove_file(&tmp);
    editor
}

fn pdfs_under(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            pdfs_under(&path, out);
        } else if path.extension().is_some_and(|e| e == "pdf") {
            out.push(path);
        }
    }
}

/// Every PDF of the crate's fixtures and of the bench corpus when it is present.
pub(super) fn corpus_pdfs() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for dir in [root.join("tests/fixtures"), root.join("../../tool/bench/.corpus")] {
        if dir.is_dir() {
            pdfs_under(&dir, &mut out);
        } else {
            println!("skipped, absent: {}", dir.display());
        }
    }
    out
}
