//! The file save with `incremental`.

use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EditableDocument, SaveOptions};

/// No Dart battery sees it: it saves to a file path through the Rust API, while Dart's incremental save runs through `host/incremental.rs`.
#[test]
fn incremental_file_save_writes_offsets_the_xref_can_follow() {
    // The incremental file save re-reads its source from disk, so the
    // source file lives until the save is done.
    let dir = std::env::temp_dir();
    let src = dir.join(format!("pdf_oxide_incremental_src_{}.pdf", std::process::id()));
    let out = dir.join(format!("pdf_oxide_incremental_out_{}.pdf", std::process::id()));
    // A source whose own xref is right, so a repair can only come
    // from the appended section.
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>",
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{body}\nendobj\n", i + 1));
    }
    let xref_at = pdf.len();
    pdf.push_str("xref\n0 4\n0000000000 65535 f \n");
    for off in offsets {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
    ));
    std::fs::write(&src, pdf).unwrap();
    assert!(!PdfDocument::open(&src).unwrap().xref_repaired(), "the source itself must be sound");
    let mut editor = DocumentEditor::open(&src).unwrap();
    editor.set_title("Appended");
    editor.save_with_options(&out, SaveOptions::incremental()).unwrap();
    let doc = PdfDocument::open(&out).unwrap();
    let repaired = doc.xref_repaired();
    let mut reopened = DocumentEditor::open(&out).unwrap();
    let title = reopened.title().unwrap();
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&out);
    assert!(!repaired, "the appended xref must point at its objects without repair");
    assert_eq!(title.as_deref(), Some("Appended"));
}
