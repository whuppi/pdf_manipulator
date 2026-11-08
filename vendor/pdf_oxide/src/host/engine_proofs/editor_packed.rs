//! A full rewrite packs its objects into object streams and ends in a
//! cross-reference stream.

use super::support::{contains, pdf_from_objects};
use crate::document::PdfDocument;
use crate::editor::{
    DocumentEditor, EncryptionAlgorithm, EncryptionConfig, SaveOptions,
};
use crate::object::ObjectRef;

/// Dictionaries 4.. hold a string each and name their successor; the catalog
/// reaches the first, so the rewrite keeps all of them and fills more than
/// one object stream.
const EXTRA: u32 = 300;

fn source_bytes() -> Vec<u8> {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra 4 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
    ];
    for id in 4..4 + EXTRA {
        let next = if id + 1 < 4 + EXTRA { format!("{} 0 R", id + 1) } else { "null".to_string() };
        objects.push(
            format!("{id} 0 obj\n<< /V (string {id}) /Next {next} >>\nendobj\n").into_bytes(),
        );
    }
    pdf_from_objects(&objects)
}

fn editor() -> DocumentEditor {
    DocumentEditor::from_bytes(source_bytes()).unwrap()
}

/// Every extra object of `saved` reads back as in the source.
fn assert_objects_equal(saved: &PdfDocument, what: &str) {
    let source = PdfDocument::from_bytes(source_bytes()).unwrap();
    for id in 1..4 + EXTRA {
        let r = ObjectRef::new(id, 0);
        assert_eq!(saved.load_object(r).unwrap(), source.load_object(r).unwrap(), "{what}: object {id}");
    }
}

/// No Dart battery sees it: it compares the object graph of the saved file with the source's, id by id, and counts the object streams.
#[test]
fn a_full_rewrite_packs_objects_and_reads_back_equal() {
    let bytes = editor().save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    assert!(bytes.starts_with(b"%PDF-1.5"), "header raised to 1.5");
    assert!(contains(&bytes, b"/Type /XRef"));
    assert!(!contains(&bytes, b"\nxref\n"), "no classic table");
    assert!(!contains(&bytes, b"(string 4)"), "packed objects are deflated");
    let streams = bytes.windows(b"/Type /ObjStm".len()).filter(|w| *w == b"/Type /ObjStm").count();
    assert_eq!(streams, 3, "{EXTRA} extras plus the tree are 303 objects in 128s");
    assert_objects_equal(&PdfDocument::from_bytes(bytes).unwrap(), "packed");
}

/// No Dart battery sees it: it reads the saved file's cross-reference rows.
#[test]
fn the_classic_layout_is_one_option_away() {
    let options = SaveOptions { object_streams: false, ..SaveOptions::full_rewrite() };
    let bytes = editor().save_to_bytes_with_options(options).unwrap();
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert!(contains(&bytes, b"\nxref\n"));
    assert!(!contains(&bytes, b"/Type /XRef") && !contains(&bytes, b"/ObjStm"));
    assert_objects_equal(&PdfDocument::from_bytes(bytes).unwrap(), "classic");
}

/// No Dart battery sees it: it reads each algorithm's objects back by id, and an RC4 or AES key is derived per object number inside the engine.
#[test]
fn an_encrypted_full_rewrite_reads_back_through_its_object_streams() {
    for algorithm in [
        EncryptionAlgorithm::Rc4_40,
        EncryptionAlgorithm::Rc4_128,
        EncryptionAlgorithm::Aes128,
        EncryptionAlgorithm::Aes256,
    ] {
        let config = EncryptionConfig::new("user", "owner").with_algorithm(algorithm);
        let bytes = editor()
            .save_to_bytes_with_options(SaveOptions::with_encryption(config))
            .unwrap();
        assert!(contains(&bytes, b"/Type /ObjStm"), "{algorithm:?}");
        assert!(contains(&bytes, b"/Type /XRef"), "{algorithm:?}");
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        assert!(saved.authenticate(b"user").unwrap(), "{algorithm:?}");
        assert_objects_equal(&saved, &format!("{algorithm:?}"));
    }
}

/// No Dart battery sees it: it needs a source whose user password is empty and a save that keeps its file key, then compares objects by id.
#[test]
fn a_save_that_keeps_the_source_encryption_packs_too() {
    let config = EncryptionConfig::new("", "owner").with_algorithm(EncryptionAlgorithm::Aes256);
    let encrypted = editor().save_to_bytes_with_options(SaveOptions::with_encryption(config)).unwrap();
    let mut again = DocumentEditor::from_bytes(encrypted).unwrap();
    let kept = again
        .save_to_bytes_with_options(SaveOptions {
            keep_source_encryption: true,
            ..SaveOptions::full_rewrite()
        })
        .unwrap();
    assert!(contains(&kept, b"/Type /ObjStm"));
    let saved = PdfDocument::from_bytes(kept).unwrap();
    assert!(saved.authenticate(b"").unwrap());
    assert_objects_equal(&saved, "kept");
}
