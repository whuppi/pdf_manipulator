//! The public incremental file save copies its source through the document's
//! own source, not through a second read of the file by path.

use super::support::{open_editor_from_bytes, pdf_from_objects};
use crate::document::PdfDocument;
use crate::editor::{EditableDocument, SaveOptions};

fn one_page() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
    ])
}

/// The editor is opened from a file that is deleted before the save, so the
/// only way to the source bytes is the open document's own source: a save
/// that reads the whole file by path (`read_source_bytes`) fails with
/// "No such file". No Dart battery can observe this: Dart's incremental save
/// runs through `host/incremental.rs`, and the file path it saves to is not
/// the path this Rust-only entry point re-opens.
#[test]
fn the_incremental_file_save_reads_the_source_through_the_open_document() {
    let source = one_page();
    let mut editor = open_editor_from_bytes(&source, "mem_incremental");
    editor.set_title("Streamed");
    let out = std::env::temp_dir().join(format!("pdf_oxide_mem_inc_{}.pdf", std::process::id()));
    let saved = editor.save_with_options(&out, SaveOptions::incremental());
    let bytes = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    saved.expect("the save needs no second read of the source file");
    assert_eq!(&bytes[..source.len()], &source[..], "every source byte stays where it was");
    assert!(bytes.len() > source.len(), "the update section is appended");
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    assert!(!doc.xref_repaired(), "the appended xref must be followable");
}

/// An edit an append cannot carry is refused before a byte is written: the
/// source file is still on disk here, so a save that copies it and appends
/// the staged objects would succeed and silently keep the removed page. No
/// Dart battery can observe this: Dart reaches the refusal through the
/// bridge status, never through this file save.
#[test]
fn the_incremental_file_save_refuses_what_an_append_cannot_carry() {
    let dir = std::env::temp_dir();
    let src = dir.join(format!("pdf_oxide_mem_inc_src_{}.pdf", std::process::id()));
    let out = dir.join(format!("pdf_oxide_mem_inc_ref_{}.pdf", std::process::id()));
    std::fs::write(&src, one_page()).unwrap();
    let mut editor = crate::editor::DocumentEditor::open(&src).unwrap();
    editor.remove_page(0).unwrap();
    let saved = editor.save_with_options(&out, SaveOptions::incremental());
    let written = out.exists();
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&out);
    assert!(saved.is_err(), "a removed page would stay in the original bytes");
    assert!(!written, "a refusal is decided before the first byte is written");
}
