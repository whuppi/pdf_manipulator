//! What a rewrite writes for a page of a nested page tree.

use super::support::{pdf_from_objects, stream_object};
use crate::editor::{DocumentEditor, EditableDocument, SaveOptions};
use crate::object::Object;

const ICC_BYTES: &[u8] = b"ICCPROFILEBYTES-0123456789";
const FONT_FILE_BYTES: &[u8] = b"FONTFILE2BYTES-9876543210";
const CONTENT_A: &[u8] = b"q 0 0 1 rg 0 0 50 50 re f Q % CONTENTA\n";
const CONTENT_B: &[u8] = b"q 1 0 0 rg 50 50 40 40 re f Q % CONTENTB\n";

/// Root `/Pages` -> inner `/Pages` -> two pages. Page 4 has an indirect
/// `/Contents` array of two streams, a `/ColorSpace` with an ICC stream
/// and a font with a descriptor and a font file.
pub(super) fn nested_resources_pdf_bytes() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 2 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Pages /Parent 2 0 R /Kids [4 0 R 5 0 R] /Count 2 >>\nendobj\n"
            .to_vec(),
        b"4 0 obj\n<< /Type /Page /Parent 3 0 R /MediaBox [0 0 100 100] /Contents 6 0 R \
          /Resources << /ColorSpace << /Cs6 [/ICCBased 8 0 R] >> /Font << /F1 9 0 R >> >> >>\nendobj\n"
            .to_vec(),
        b"5 0 obj\n<< /Type /Page /Parent 3 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec(),
        b"6 0 obj\n[7 0 R 10 0 R]\nendobj\n".to_vec(),
        stream_object(7, "", CONTENT_A),
        stream_object(8, "/N 3", ICC_BYTES),
        b"9 0 obj\n<< /Type /Font /Subtype /TrueType /BaseFont /TESTFONT /FontDescriptor 11 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(10, "", CONTENT_B),
        b"11 0 obj\n<< /Type /FontDescriptor /FontName /TESTFONT /Flags 32 /FontFile2 12 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(12, "", FONT_FILE_BYTES),
    ])
}

/// Asserts page 0 of `bytes` still reaches both content streams, the
/// ICC stream behind `/Cs6` and the font file behind the descriptor.
fn assert_nested_resources_survive(bytes: &[u8], what: &str) {
    let doc = crate::document::PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    let resolve = |o: &Object| match o {
        Object::Reference(r) => doc.load_object(*r).unwrap(),
        other => other.clone(),
    };
    let page = doc.load_object(doc.get_page_ref(0).unwrap()).unwrap();
    let page = page.as_dict().unwrap();

    let contents = resolve(&page["Contents"]);
    let streams: Vec<Vec<u8>> = contents
        .as_array()
        .unwrap_or_else(|| panic!("{what}: /Contents is not an array"))
        .iter()
        .map(|s| resolve(s).decode_stream_data().unwrap_or_else(|e| panic!("{what}: {e}")))
        .collect();
    assert_eq!(streams, vec![CONTENT_A.to_vec(), CONTENT_B.to_vec()], "{what}: content streams");

    let resources = resolve(&page["Resources"]);
    let resources = resources.as_dict().unwrap();
    let spaces = resolve(&resources["ColorSpace"]);
    let cs6 = spaces.as_dict().unwrap()["Cs6"].as_array().unwrap().clone();
    let icc = resolve(&cs6[1]).decode_stream_data().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(icc, ICC_BYTES, "{what}: ICC profile");

    let fonts = resolve(&resources["Font"]);
    let font = resolve(&fonts.as_dict().unwrap()["F1"]);
    let descriptor = resolve(&font.as_dict().unwrap()["FontDescriptor"]);
    let font_file = resolve(&descriptor.as_dict().unwrap()["FontFile2"]);
    assert_eq!(
        font_file.decode_stream_data().unwrap_or_else(|e| panic!("{what}: {e}")),
        FONT_FILE_BYTES,
        "{what}: font file"
    );
}

/// No Dart battery sees it: it lists every object of the saved bytes, which no Dart call can do.
#[test]
fn a_full_rewrite_of_a_nested_tree_writes_everything_a_kept_page_reaches() {
    for garbage_collect in [true, false] {
        for compress in [true, false] {
            let mut editor = DocumentEditor::from_bytes(nested_resources_pdf_bytes()).unwrap();
            let options = SaveOptions { garbage_collect, compress, ..SaveOptions::full_rewrite() };
            let bytes = editor.save_to_bytes_with_options(options).unwrap();
            assert_nested_resources_survive(
                &bytes,
                &format!("gc {garbage_collect}, compress {compress}"),
            );
        }
    }
}

/// No Dart battery sees it: it lists every object of the saved bytes, which no Dart call can do.
#[test]
fn a_nested_tree_save_that_drops_a_page_keeps_what_the_kept_page_reaches() {
    for garbage_collect in [true, false] {
        let mut editor = DocumentEditor::from_bytes(nested_resources_pdf_bytes()).unwrap();
        editor.remove_page(1).unwrap();
        let options = SaveOptions { garbage_collect, ..SaveOptions::full_rewrite() };
        let bytes = editor.save_to_bytes_with_options(options).unwrap();
        assert_nested_resources_survive(&bytes, &format!("gc {garbage_collect}, page 1 dropped"));
    }
}

/// No Dart battery sees it: it lists every object of the saved bytes, which no Dart call can do.
#[test]
fn extracting_a_page_of_a_nested_tree_keeps_what_it_reaches() {
    let mut editor = DocumentEditor::from_bytes(nested_resources_pdf_bytes()).unwrap();
    let bytes = editor.extract_pages_to_bytes(&[0]).unwrap();
    let doc = crate::document::PdfDocument::from_bytes(bytes.clone()).unwrap();
    assert_eq!(doc.page_count().unwrap(), 1);
    assert_nested_resources_survive(&bytes, "extract_pages");
}
