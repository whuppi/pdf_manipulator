//! A redaction that is refused on one page redacts no page.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::error::Error;
use crate::redaction::RedactionOptions;

/// Two pages that each show content under a mark. Page one shows text, which
/// the pass redacts; page two draws a path it never paints, which the pass
/// refuses since it cannot show the path to lie outside the mark.
fn two_pages_the_second_refused() -> Vec<u8> {
    let objects = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
          /Resources << /Font << /F1 7 0 R >> >> >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", b"BT\n/F1 10 Tf\n1 0 0 1 100 700 Tm\n(TOPSECRET) Tj\nET\n"),
        b"5 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 6 0 R \
          >>\nendobj\n"
            .to_vec(),
        stream_object(6, "", b"100 700 m 300 700 l\n"),
        b"7 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ];
    pdf_from_objects(&objects)
}

fn marked_editor() -> DocumentEditor {
    let doc = PdfDocument::from_bytes(two_pages_the_second_refused()).expect("the fixture parses");
    let mut editor = DocumentEditor::from_document(doc).expect("the editor opens");
    for page in 0..2 {
        editor.add_redaction(page, [90.0, 695.0, 400.0, 715.0], None).expect("the mark queues");
    }
    editor
}

/// No Dart battery sees it: its documents come from the Dart builder, which
/// writes no composite font, so none puts a refusal on a later page behind a
/// page the pass accepts.
#[test]
fn a_refused_page_leaves_the_pages_before_it_unredacted() {
    let mut editor = marked_editor();
    let before = editor.save_to_bytes().expect("the editor saves");

    let error = editor
        .apply_redactions_destructive(RedactionOptions::default())
        .expect_err("page two is refused");
    assert!(matches!(error, Error::Unsupported(_)), "expected a refusal, got {error:?}");

    let after = editor.save_to_bytes().expect("the editor saves");
    assert!(after == before, "the refused apply left page one redacted");
    assert!(
        editor.is_page_marked_for_redaction(0) && editor.is_page_marked_for_redaction(1),
        "the marks stay queued, so a caller can fix the page and apply again"
    );
}
