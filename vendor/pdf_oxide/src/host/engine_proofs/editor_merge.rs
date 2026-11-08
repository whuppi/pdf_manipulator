//! Merged pages: copies, page-list edits and what the save writes.

use super::support::{pdf_from_objects, stream_object};
use crate::editor::{DocumentEditor, PageKey, PageSlot};
use crate::object::{Object, ObjectRef};
use std::collections::HashSet;

/// One page per marker, all sharing one font. The page text is the marker.
fn marker_pdf_bytes(markers: &[&str]) -> Vec<u8> {
    let font_id = 3 + 2 * markers.len();
    let kids: Vec<String> = (0..markers.len()).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        format!(
            "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
            kids.join(" "),
            markers.len()
        )
        .into_bytes(),
    ];
    for (i, marker) in markers.iter().enumerate() {
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Contents {} 0 R \
                 /Resources << /Font << /F1 {font_id} 0 R >> >> >>\nendobj\n",
                3 + 2 * i,
                4 + 2 * i
            )
            .into_bytes(),
        );
        let content = format!("BT\n/F1 18 Tf\n1 0 0 1 20 200 Tm\n({marker}) Tj\nET\n");
        objects.push(stream_object(4 + 2 * i as u32, "", content.as_bytes()));
    }
    objects.push(
        format!("{font_id} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n")
            .into_bytes(),
    );
    pdf_from_objects(&objects)
}

/// Two source pages (`S0`, `S1`) with three merged pages (`M0`, `M1`,
/// `M2`) after them.
fn editor_with_three_merged_pages() -> DocumentEditor {
    let mut editor = DocumentEditor::from_bytes(marker_pdf_bytes(&["S0", "S1"])).unwrap();
    assert_eq!(editor.merge_from_bytes(&marker_pdf_bytes(&["M0", "M1", "M2"])).unwrap(), 3);
    editor
}

/// The text of every page of a saved file, in page order.
fn page_texts(bytes: &[u8]) -> Vec<String> {
    let doc = crate::document::PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    (0..doc.page_count().unwrap())
        .map(|i| doc.extract_text(i).unwrap().trim().to_string())
        .collect()
}

/// No Dart battery sees it: it asserts `size_of::<PageSlot>()` and the `source`/`merged`/`key` round trip of a type the editor keeps private, and no Dart call observes the layout of a Rust type.
#[test]
fn a_page_slot_holds_both_kinds_of_page_in_four_bytes() {
    assert_eq!(std::mem::size_of::<PageSlot>(), 4);
    for index in [0, 1, 9_999, (1 << 31) - 1] {
        assert_eq!(PageSlot::source(index).key(), PageKey::Source(index));
        assert_eq!(PageSlot::merged(index).key(), PageKey::Merged(index));
    }
    assert_ne!(PageSlot::source(3), PageSlot::merged(3));
}

/// No Dart battery sees it: `duplicate_page` is a method of the Rust editor that no public op reaches (Dart's `keepPages` refuses a page listed twice and no Dart editor call duplicates a page), so the saved `/Kids` of a duplicated page can only be listed from Rust.
#[test]
fn a_duplicated_page_is_a_new_page_object_and_every_page_is_reachable() {
    use crate::editor::EditableDocument;
    let mut editor = editor_with_three_merged_pages();
    assert_eq!(editor.duplicate_page(0).unwrap(), 5);
    assert_eq!(editor.duplicate_page(3).unwrap(), 6);
    assert_eq!(editor.current_page_count(), 7);

    let bytes = editor.save_to_bytes().unwrap();
    let objects = saved_objects(&bytes);
    let pages = objects
        .iter()
        .find(|o| o.as_dict().and_then(|d| d.get("Type")).and_then(|t| t.as_name()) == Some("Pages"))
        .unwrap();
    let kids = pages.as_dict().and_then(|d| d.get("Kids")).and_then(|k| k.as_array()).unwrap();
    assert_eq!(kids.len(), 7);
    let ids: HashSet<u32> = kids.iter().filter_map(|k| k.as_reference()).map(|r| r.id).collect();
    assert_eq!(ids.len(), 7, "a page object is listed in /Kids once: {kids:?}");

    let doc = crate::document::PdfDocument::from_bytes(bytes).unwrap();
    assert_eq!(doc.page_count().unwrap(), 7);
    let texts: Vec<String> =
        (0..7).map(|i| doc.extract_text(i).unwrap().trim().to_string()).collect();
    assert_eq!(texts, ["S0", "S1", "M0", "M1", "M2", "S0", "M1"]);
}

/// No Dart battery sees it: no public op calls `extract_pages_to_bytes` on an editor that holds merged pages; Dart's page extraction (`split`) opens its own editor on the source it is given, which has none.
#[test]
fn extracting_pages_of_a_merged_document_leaves_the_editor_unchanged() {
    let mut editor = editor_with_three_merged_pages();
    let extracted = editor.extract_pages_to_bytes(&[1, 3]).unwrap();
    assert_eq!(page_texts(&extracted), ["S1", "M1"]);
    let reversed = editor.extract_pages_to_bytes(&[4, 0]).unwrap();
    assert_eq!(page_texts(&reversed), ["M2", "S0"]);

    assert_eq!(editor.current_page_count(), 5);
    assert_eq!(page_texts(&editor.save_to_bytes().unwrap()), ["S0", "S1", "M0", "M1", "M2"]);
    assert!(editor.extract_pages_to_bytes(&[5]).is_err());
}

/// Every object the saved file holds, once.
fn saved_objects(bytes: &[u8]) -> Vec<Object> {
    let doc = crate::document::PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    doc.all_object_ids()
        .into_iter()
        .filter(|&id| id != 0 && !doc.is_free_entry(id))
        .map(|id| doc.load_object(ObjectRef::new(id, 0)).unwrap())
        .collect()
}

// ── a full rewrite writes everything a kept page reaches ──
