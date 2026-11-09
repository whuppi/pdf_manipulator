//! What a full rewrite carries from the source trailer: `/ID` and `/Info`.

use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::object::{Object, ObjectRef};

const FILE_ID: &str = "<000102030405060708090A0B0C0D0E0F> <F0E0D0C0B0A090807060504030201000>";

fn source_bytes() -> Vec<u8> {
    let objects = [
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n",
        "4 0 obj\n<< /Title (Source Title) /Author (Source Author) >>\nendobj\n",
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for o in objects {
        offsets.push(out.len());
        out.extend_from_slice(o.as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size 5 /Root 1 0 R /Info 4 0 R /ID [{FILE_ID}] >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    out
}

fn options(object_streams: bool) -> SaveOptions {
    SaveOptions { object_streams, ..SaveOptions::full_rewrite() }
}

fn trailer_entry(doc: &PdfDocument, key: &str) -> Object {
    doc.trailer().as_dict().and_then(|d| d.get(key)).cloned().unwrap_or(Object::Null)
}

fn info_of(doc: &PdfDocument) -> (u32, std::collections::HashMap<String, Object>) {
    let Object::Reference(r) = trailer_entry(doc, "Info") else {
        panic!("the saved trailer has no /Info reference");
    };
    let Object::Dictionary(d) = doc.load_object(r).unwrap() else {
        panic!("/Info is not a dictionary");
    };
    (r.id, d)
}

fn text(s: &str) -> Object {
    Object::String(s.as_bytes().to_vec())
}

/// No Dart battery sees it: the trailer's `/ID` is read by no Dart call, and the save is compared with the source's trailer entry by entry.
#[test]
fn a_full_rewrite_carries_the_source_id_and_info() {
    let source = PdfDocument::from_bytes(source_bytes()).unwrap();
    let id = trailer_entry(&source, "ID");
    assert!(matches!(id, Object::Array(ref a) if a.len() == 2), "the fixture has an /ID");
    for object_streams in [true, false] {
        let bytes = DocumentEditor::from_bytes(source_bytes())
            .unwrap()
            .save_to_bytes_with_options(options(object_streams))
            .unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        assert_eq!(trailer_entry(&saved, "ID"), id, "/ID, object streams {object_streams}");
        let (_, info) = info_of(&saved);
        assert_eq!(info.get("Title"), Some(&text("Source Title")), "object streams {object_streams}");
        assert_eq!(info.get("Author"), Some(&text("Source Author")), "object streams {object_streams}");
    }
}

/// No Dart battery sees it: it counts the dictionaries of the saved file that hold a `/Title`, to show no second copy of the source's information dictionary is left beside the new one.
#[test]
fn a_changed_title_replaces_the_source_info_object_in_place() {
    for object_streams in [true, false] {
        let mut editor = DocumentEditor::from_bytes(source_bytes()).unwrap();
        editor.set_title("New Title");
        let bytes = editor.save_to_bytes_with_options(options(object_streams)).unwrap();
        let saved = PdfDocument::from_bytes(bytes).unwrap();
        let (id, info) = info_of(&saved);
        assert_eq!(id, 4, "the new dictionary takes the source's id, object streams {object_streams}");
        assert_eq!(info.get("Title"), Some(&text("New Title")));
        assert_eq!(info.get("Author"), Some(&text("Source Author")), "the other entries stay");
        let with_title = saved
            .all_object_ids()
            .into_iter()
            .filter(|&id| match saved.load_object(ObjectRef::new(id, 0)) {
                Ok(Object::Dictionary(d)) => d.contains_key("Title"),
                _ => false,
            })
            .count();
        assert_eq!(with_title, 1, "one information dictionary, object streams {object_streams}");
    }
}
