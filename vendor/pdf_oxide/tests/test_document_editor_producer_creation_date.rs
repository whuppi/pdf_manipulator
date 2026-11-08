//! Regression test: `DocumentEditor::set_producer` and
//! `set_creation_date` round-trip through save + reopen.
//!
//! The FFI entries at `src/ffi.rs:532-586` previously returned
//! ERR_SUCCESS but did nothing. Round-trip is the signal that the
//! setters actually persist into the saved PDF.

use pdf_oxide::api::Pdf;
use pdf_oxide::editor::DocumentEditor;

fn make_doc() -> Vec<u8> {
    Pdf::from_text("hello").unwrap().into_bytes()
}

#[test]
fn producer_round_trips_through_save() {
    let bytes = make_doc();
    let mut editor = DocumentEditor::from_bytes(bytes).expect("open");
    editor.set_producer("pdf_oxide unit-test");

    // Save and reopen to verify persistence, not just in-memory read-back.
    let new_bytes = editor.save_to_bytes().expect("save");
    let mut reopened = DocumentEditor::from_bytes(new_bytes).expect("reopen");
    let producer = reopened.producer().expect("read producer");
    assert_eq!(producer.as_deref(), Some("pdf_oxide unit-test"));
}

#[test]
fn creation_date_round_trips_through_save() {
    let bytes = make_doc();
    let mut editor = DocumentEditor::from_bytes(bytes).expect("open");
    editor.set_creation_date("D:20260421120000Z");

    let new_bytes = editor.save_to_bytes().expect("save");
    let mut reopened = DocumentEditor::from_bytes(new_bytes).expect("reopen");
    let date = reopened.creation_date().expect("read creation_date");
    assert_eq!(date.as_deref(), Some("D:20260421120000Z"));
}

#[test]
fn producer_in_memory_readback() {
    // Setting producer then reading it back in the same session should
    // reflect the new value even before save, matching the existing
    // set_title / set_author semantics.
    let bytes = make_doc();
    let mut editor = DocumentEditor::from_bytes(bytes).expect("open");
    editor.set_producer("in-memory");
    assert_eq!(editor.producer().expect("read").as_deref(), Some("in-memory"));
}
