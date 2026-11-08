//! The same input saves to the same bytes where the order of a hash map could
//! reach the output.
//!
//! Each test saves twice from two fresh editors or writers. Every hash map
//! carries its own random seed, so an output order that follows one differs
//! between the two saves; the fixture holds enough entries that a random order
//! cannot agree by luck.

use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EditableDocument, SaveOptions};
use crate::elements::{FontSpec, TextContent, TextStyle};
use crate::geometry::Rect;
use crate::object::{Object, ObjectRef};

/// Objects are numbered from 1 in the order given; the xref is computed.
fn pdf_from_objects(bodies: &[String], catalog_extra: &str) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, body).as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", bodies.len() + 1).as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R {catalog_extra} >>\nstartxref\n{xref_at}\n%%EOF\n",
            bodies.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// One empty page.
fn one_page_pdf() -> Vec<u8> {
    pdf_from_objects(
        &[
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>".to_string(),
        ],
        "",
    )
}

fn font_objects(bytes: Vec<u8>) -> usize {
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    doc.all_object_ids()
        .into_iter()
        .filter(|&id| id != 0 && !doc.is_free_entry(id))
        .filter_map(|id| doc.load_object(ObjectRef::new(id, 0)).ok())
        .filter(|o| o.as_dict().and_then(|d| d.get("Type")).and_then(|t| t.as_name()) == Some("Font"))
        .count()
}

/// Saves the same edit twice from two editors and returns both outputs.
fn save_twice(open: impl Fn() -> DocumentEditor, edit: impl Fn(&mut DocumentEditor)) -> (Vec<u8>, Vec<u8>) {
    let run = || {
        let mut editor = open();
        edit(&mut editor);
        editor.save_to_bytes().unwrap()
    };
    (run(), run())
}

/// No Dart battery sees it: the objects it stages sit in the editor's own
/// `modified_objects` map, which Dart cannot fill with ids the source lacks.
///
/// 60 staged objects: the chance two random orders agree is 1/60!, about 1e-82.
#[test]
fn a_rewrite_writes_staged_objects_the_source_lacks_in_id_order() {
    let source = one_page_pdf();
    let (first, second) = save_twice(
        || DocumentEditor::from_bytes(source.clone()).unwrap(),
        |editor| {
            for id in 100..160u32 {
                editor.modified_objects_mut().insert(id, Object::Integer(id as i64));
            }
            editor.set_modified(true);
        },
    );
    let doc = PdfDocument::from_bytes(first.clone()).unwrap();
    assert!(
        matches!(doc.load_object(ObjectRef::new(159, 0)).unwrap(), Object::Integer(159)),
        "the staged objects must reach the output"
    );
    assert_eq!(first, second, "two saves of one edit differ");
}

/// No Dart battery sees it: Dart's incremental save runs through
/// `host/incremental.rs`; this is the Rust file save, `write_incremental`.
///
/// 60 staged objects: the chance two random orders agree is 1/60!, about 1e-82.
#[test]
fn an_incremental_file_save_writes_staged_objects_in_id_order() {
    let dir = std::env::temp_dir();
    let tag = std::process::id();
    let src = dir.join(format!("pdf_oxide_determinism_src_{tag}.pdf"));
    let outs = [
        dir.join(format!("pdf_oxide_determinism_first_{tag}.pdf")),
        dir.join(format!("pdf_oxide_determinism_second_{tag}.pdf")),
    ];
    std::fs::write(&src, one_page_pdf()).unwrap();
    for out in &outs {
        let mut editor = DocumentEditor::open(&src).unwrap();
        for id in 100..160u32 {
            editor.modified_objects_mut().insert(id, Object::Integer(id as i64));
        }
        editor.set_modified(true);
        editor.save_with_options(out, SaveOptions::incremental()).unwrap();
    }
    let saved: Vec<Vec<u8>> = outs.iter().map(|o| std::fs::read(o).unwrap()).collect();
    for path in outs.iter().chain([&src]) {
        let _ = std::fs::remove_file(path);
    }
    assert!(saved[0].len() > one_page_pdf().len(), "the update must append the staged objects");
    assert_eq!(saved[0], saved[1], "two incremental saves of one edit differ");
}

/// No Dart battery sees it: the overlay text comes from the page editor, a
/// Rust type that Dart does not expose.
///
/// 12 Standard-14 fonts are written, one object each: the chance two random
/// orders agree is 1/12!, about 2e-9.
#[test]
fn overlay_fonts_are_written_in_name_order() {
    const FONTS: [&str; 12] = [
        "Courier",
        "Courier-Bold",
        "Courier-BoldOblique",
        "Courier-Oblique",
        "Helvetica",
        "Helvetica-Bold",
        "Helvetica-BoldOblique",
        "Helvetica-Oblique",
        "Times-Bold",
        "Times-BoldItalic",
        "Times-Italic",
        "Times-Roman",
    ];
    let source = one_page_pdf();
    let (first, second) = save_twice(
        || DocumentEditor::from_bytes(source.clone()).unwrap(),
        |editor| {
            let mut page_editor = editor.page_editor(0).unwrap();
            for (i, name) in FONTS.iter().enumerate() {
                let bbox = Rect { x: 10.0, y: 10.0 + 12.0 * i as f32, width: 100.0, height: 10.0 };
                page_editor.page.add_text(TextContent::new(
                    format!("TEXT{i}"),
                    bbox,
                    FontSpec { name: name.to_string(), size: 10.0 },
                    TextStyle::default(),
                ));
            }
            let page = page_editor.done().unwrap();
            editor.save_page_from_editor(page).unwrap();
        },
    );
    assert!(font_objects(first.clone()) >= FONTS.len(), "every overlay font must be written");
    assert_eq!(first, second, "two saves of one edit differ");
}
