//! A field value set through the editor, the forms flattened and a redaction
//! applied, all before one save.
//!
//! Each case is saved with garbage collection on and off.

use super::redact_path_save::assert_absent_everywhere;
use super::redact_save::{obj, stream, write_pdf};
use super::support::{open_editor_from_bytes, stream_object};
use crate::editor::form_fields::FormFieldValue;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::PdfDocument;

const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];
const SECRET: &str = "SECRETVALUE";
const PUBLIC: &str = "PUBLICVALUE";

/// A one-page form with text fields `secret` (under `BOX`) and, when
/// `with_public`, `keep` (outside it), each with an empty appearance.
fn form(with_public: bool) -> Vec<u8> {
    let fields = if with_public { "[8 0 R 9 0 R]" } else { "[8 0 R]" };
    let mut objects = vec![
        obj(
            1,
            &format!(
                "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields {fields} \
                 /DR << /Font << /He 5 0 R >> >> /DA (/He 0 Tf 0 g) >> >>"
            ),
        ),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            &format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R /Annots {fields} >>"),
        ),
        stream(4, b"q Q\n"),
        obj(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        obj(
            8,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (secret) /Rect [20 142 200 162] \
             /F 4 /P 3 0 R /DA (/He 0 Tf 0 g) /AP << /N 6 0 R >> >>",
        ),
        (6, stream_object(6, "/Type /XObject /Subtype /Form /BBox [0 0 180 20]", b"/Tx BMC\nEMC\n")),
    ];
    if with_public {
        objects.push(obj(
            9,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (keep) /Rect [20 20 200 40] \
             /F 4 /P 3 0 R /DA (/He 0 Tf 0 g) /AP << /N 7 0 R >> >>",
        ));
        objects.push((7, stream_object(7, "/Type /XObject /Subtype /Form /BBox [0 0 180 20]", b"/Tx BMC\nEMC\n")));
    }
    write_pdf(&objects)
}

/// The host's `setFormFieldValue`, `flattenForms` (one page), `markRedaction`
/// and `applyRedactions`, in that order, before any save.
fn edit(bytes: &[u8], values: &[(&str, &str)], redaction: [f32; 4]) -> DocumentEditor {
    let mut editor = open_editor_from_bytes(bytes, "redact_after_value");
    for (name, value) in values {
        editor.set_form_field_value(name, FormFieldValue::Text((*value).into())).unwrap();
    }
    editor.flatten_forms_on_page(0).unwrap();
    editor.add_redaction(0, redaction, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    editor
}

fn saved(editor: &mut DocumentEditor, garbage_collect: bool) -> Vec<u8> {
    editor
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            garbage_collect,
            object_streams: false,
            ..SaveOptions::full_rewrite()
        })
        .unwrap_or_else(|e| panic!("gc={garbage_collect}: the save failed: {e}"))
}

/// A field with a value set whose widget the redaction does not touch saves,
/// and its flattened value reads. Only the writer's own check and the saved
/// file's text show it; the Dart ops reach the same calls, but a Dart test
/// cannot tell the writer's refusal from any other failed save.
#[test]
fn a_value_set_on_a_field_outside_the_box_saves() {
    for garbage_collect in [true, false] {
        let mut editor = edit(&form(false), &[("secret", SECRET)], [200.0, 250.0, 290.0, 290.0]);
        let out = saved(&mut editor, garbage_collect);
        let text = PdfDocument::from_bytes(out).unwrap().extract_text(0).unwrap();
        assert!(text.contains(SECRET), "gc={garbage_collect}: the value does not read: {text:?}");
    }
}

/// Two fields with values set, one under the box and one outside it, are
/// flattened and redacted: the save succeeds, the value under the box is in no
/// decoded stream of the file and the one outside reads. No Dart test reads a
/// file's decoded streams or the writer's refusal.
#[test]
fn a_value_set_under_the_box_leaves_every_stream_with_another_field_kept() {
    for garbage_collect in [true, false] {
        let mut editor =
            edit(&form(true), &[("secret", SECRET), ("keep", PUBLIC)], BOX);
        let out = saved(&mut editor, garbage_collect);
        assert_absent_everywhere(&out, SECRET.as_bytes(), garbage_collect);
        let text = PdfDocument::from_bytes(out).unwrap().extract_text(0).unwrap();
        assert!(!text.contains(SECRET), "gc={garbage_collect}: the value reads: {text:?}");
        assert!(text.contains(PUBLIC), "gc={garbage_collect}: the value outside does not read: {text:?}");
    }
}
