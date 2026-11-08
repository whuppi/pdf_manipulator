//! The scrub of a Type 3 font a flattened appearance carries beside it.
//!
//! No public path hands the scrub a Type 3 font among the extra objects of a
//! flat set (the fallback fonts the flattening embeds are always Type 0), so
//! the proof supplies the objects through the test seam of the flat set, as the
//! flattening of a widget does, and redacts and saves the page.

use super::redact_path_save::{assert_absent_everywhere, save};
use super::redact_save::{obj, render, stream, write_pdf, DPI, PAGE};
use super::support::{contains, open_editor_from_bytes, stream_object};
use crate::host::flat_extras_seam::TEST_FLAT_EXTRAS;
use crate::object::{Object, ObjectRef};
use crate::PdfDocument;
use std::collections::HashMap;

const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];
/// The ids of the extra objects: the font and the procedures of `a` and `b`.
const FONT: u32 = 900;
const SECRET_PROC: u32 = 901;
const PUBLIC_PROC: u32 = 902;

fn name(value: &str) -> Object {
    Object::Name(value.to_string())
}

fn numbers(values: &[i64]) -> Object {
    Object::Array(values.iter().map(|v| Object::Integer(*v)).collect())
}

fn reference(id: u32) -> Object {
    Object::Reference(ObjectRef::new(id, 0))
}

fn procedure(marker: &str) -> Object {
    let body = format!("1000 0 0 0 1000 1000 d1 0 0 1000 1000 re f % {marker}\n");
    Object::stream(HashMap::new(), bytes::Bytes::from(body.into_bytes()))
}

/// A Type 3 font with two glyphs, `a` (code 97) and `b` (code 98), each a
/// solid square, named by `/Differences`.
fn type3_extras() -> Vec<(u32, Object)> {
    let mut procs = HashMap::new();
    procs.insert("a".to_string(), reference(SECRET_PROC));
    procs.insert("b".to_string(), reference(PUBLIC_PROC));
    let mut encoding = HashMap::new();
    encoding.insert("Type".to_string(), name("Encoding"));
    encoding.insert(
        "Differences".to_string(),
        Object::Array(vec![Object::Integer(97), name("a"), name("b")]),
    );
    let mut font = HashMap::new();
    font.insert("Type".to_string(), name("Font"));
    font.insert("Subtype".to_string(), name("Type3"));
    font.insert("FontBBox".to_string(), numbers(&[0, 0, 1000, 1000]));
    font.insert(
        "FontMatrix".to_string(),
        Object::Array([0.001, 0.0, 0.0, 0.001, 0.0, 0.0].iter().map(|v| Object::Real(*v)).collect()),
    );
    font.insert("FirstChar".to_string(), Object::Integer(97));
    font.insert("LastChar".to_string(), Object::Integer(98));
    font.insert("Widths".to_string(), numbers(&[1000, 1000]));
    font.insert("Encoding".to_string(), Object::Dictionary(encoding));
    font.insert("CharProcs".to_string(), Object::Dictionary(procs));
    font.insert("Resources".to_string(), Object::Dictionary(HashMap::new()));
    vec![
        (FONT, Object::Dictionary(font)),
        (SECRET_PROC, procedure("SECRETGLYPH")),
        (PUBLIC_PROC, procedure("PUBLICGLYPH")),
    ]
}

/// Two text widgets whose appearances draw the Type 3 font the flat set
/// carries (object 900, which the source does not hold): `secret` shows `a`
/// under the box and `keep` shows `b` outside it.
fn two_widgets() -> Vec<u8> {
    let widget = |id: u32, title: &str, rect: &str, appearance: u32| {
        obj(
            id,
            &format!(
                "<< /Type /Annot /Subtype /Widget /FT /Tx /T ({title}) /Rect {rect} /F 4 \
                 /P 3 0 R /AP << /N {appearance} 0 R >> >>"
            ),
        )
    };
    let form = |id: u32, content: &[u8]| {
        (
            id,
            stream_object(
                id,
                "/Type /XObject /Subtype /Form /BBox [0 0 180 20] \
                 /Resources << /Font << /F3 900 0 R >> >>",
                content,
            ),
        )
    };
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [8 0 R 9 0 R] >> >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
             /Annots [8 0 R 9 0 R] >>",
        ),
        stream(4, b"q Q\n"),
        widget(8, "secret", "[20 142 200 162]", 6),
        widget(9, "keep", "[20 20 200 40]", 7),
        form(6, b"/Tx BMC\nBT /F3 12 Tf 2 4 Td (a) Tj ET\nEMC\n"),
        form(7, b"/Tx BMC\nBT /F3 12 Tf 2 4 Td (b) Tj ET\nEMC\n"),
    ])
}

/// Flattens the widgets, redacts the box over `secret` and saves.
fn redacted(garbage_collect: bool) -> Vec<u8> {
    TEST_FLAT_EXTRAS.with(|extras| *extras.borrow_mut() = type3_extras());
    let mut editor = open_editor_from_bytes(&two_widgets(), "redact_extra_type3");
    editor.flatten_forms_on_page(0).unwrap();
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    save(&mut editor, garbage_collect)
}

/// The `/CharProcs` of the Type 3 font the saved file holds, and the data of
/// the stream each entry names.
fn saved_procs(doc: &PdfDocument, out: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    for id in 1..1000 {
        let Ok(object) = doc.load_object(ObjectRef::new(id, 0)) else { continue };
        let Some(dict) = object.as_dict() else { continue };
        if dict.get("Subtype").and_then(Object::as_name) != Some("Type3") {
            continue;
        }
        let table = doc.resolve_object(dict.get("CharProcs").unwrap()).unwrap();
        for (key, value) in table.as_dict().unwrap() {
            let data = doc.resolve_object(value).unwrap().decode_stream_data().unwrap();
            found.push((key.clone(), data));
        }
    }
    assert!(!out.is_empty());
    found
}

/// Whether the render has dark ink at the page point (`x`, `y`).
fn ink_at(img: &image::RgbaImage, x: f32, y: f32) -> bool {
    let scale = DPI as f32 / 72.0;
    let p = img.get_pixel((x * scale) as u32, ((PAGE - y) * scale) as u32).0;
    p[0] < 64 && p[1] < 64 && p[2] < 64
}

/// The procedure of a glyph only the removed text showed leaves a Type 3 font
/// a flattened appearance carries beside it (its name leaves `/CharProcs` and
/// its stream is not written), and the glyph the text outside the box shows
/// keeps its procedure and still paints. No battery can build the flat set:
/// the Dart flattening never carries a Type 3 font.
#[test]
fn an_extra_type3_font_loses_the_procedure_of_the_removed_glyph() {
    for garbage_collect in [true, false] {
        let out = redacted(garbage_collect);
        let case = format!("gc={garbage_collect}");
        assert_absent_everywhere(&out, b"SECRETGLYPH", garbage_collect);
        assert!(contains(&out, b"PUBLICGLYPH"), "{case}: the kept glyph's procedure is gone");
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let procs = saved_procs(&doc, &out);
        assert!(
            procs.iter().all(|(key, _)| key != "a"),
            "{case}: the removed glyph is still in /CharProcs: {:?}",
            procs.iter().map(|(key, _)| key).collect::<Vec<_>>()
        );
        let kept = procs.iter().find(|(key, _)| key == "b");
        let kept = kept.unwrap_or_else(|| panic!("{case}: the kept glyph left /CharProcs"));
        assert!(contains(&kept.1, b"PUBLICGLYPH"), "{case}: the kept glyph's procedure changed");
        let page = render(&out);
        assert!(ink_at(&page, 28.0, 30.0), "{case}: the kept glyph paints no ink");
    }
}
