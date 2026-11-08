//! A form field's tooltip, maximum length and colours are removed by the same
//! edits that set them, given no value: the saved field has no `/TU`, no
//! `/MaxLen`, and no `/BG` or `/BC` in its `/MK`, and an `/MK` left with no
//! entry is gone.

use super::redact_save::{obj, write_pdf};
use super::support::open_editor_from_bytes;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::extractors::forms::FormExtractor;
use crate::host::dispatch::{
    edit_set_form_field_background_color, edit_set_form_field_border_color,
    edit_set_form_field_max_length, edit_set_form_field_tooltip,
};
use crate::object::Object;
use crate::PdfDocument;
use std::collections::HashMap;

const YELLOW: [f32; 3] = [1.0, 1.0, 0.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// A page with the text field `both`, which carries a tooltip, a maximum
/// length and both colours, and the text field `bg`, whose `/MK` holds a
/// background colour only.
fn form() -> Vec<u8> {
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R 6 0 R] /DA (/Helv 12 Tf 0 g) >> >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Annots [5 0 R 6 0 R] >>",
        ),
        obj(
            5,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (both) /TU (tip) /MaxLen 8 \
             /MK << /BG [1 1 0] /BC [1 0 0] >> /Rect [20 20 120 40] /F 4 /P 3 0 R >>",
        ),
        obj(
            6,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (bg) /MK << /BG [0 1 0] >> \
             /Rect [20 60 120 80] /F 4 /P 3 0 R >>",
        ),
    ])
}

fn editor() -> DocumentEditor {
    open_editor_from_bytes(&form(), "editor_field_removals")
}

/// The field's own dictionary in the editor's saved file, reopened.
fn saved_field(editor: &mut DocumentEditor, name: &str) -> HashMap<String, Object> {
    let bytes = editor
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            object_streams: false,
            ..SaveOptions::full_rewrite()
        })
        .expect("the editor saves");
    let doc = PdfDocument::from_bytes(bytes).expect("the file reopens");
    let field = FormExtractor::extract_fields(&doc)
        .expect("the fields read")
        .into_iter()
        .find(|f| f.full_name == name)
        .unwrap_or_else(|| panic!("field {name} is in the file"));
    doc.load_object(field.object_ref.expect("the field has an object"))
        .expect("the field loads")
        .as_dict()
        .cloned()
        .expect("the field is a dictionary")
}

/// The entries of the field's `/MK`, or `None` when it has no `/MK`.
fn mk(field: &HashMap<String, Object>) -> Option<HashMap<String, Object>> {
    match field.get("MK") {
        Some(Object::Dictionary(d)) => Some(d.clone()),
        Some(other) => panic!("/MK is a dictionary, got {other:?}"),
        None => None,
    }
}

/// No Dart battery sees it: whether a key is absent from the saved field's
/// dictionary is a property of the written objects, and the form reader
/// returns the same null for an absent tooltip as for an empty one.
#[test]
fn a_tooltip_set_then_removed_leaves_no_tu() {
    let mut editor = editor();
    edit_set_form_field_tooltip(&mut editor, "both", Some("changed")).unwrap();
    edit_set_form_field_tooltip(&mut editor, "both", None).unwrap();
    let field = saved_field(&mut editor, "both");
    assert!(!field.contains_key("TU"), "a removed tooltip stays: {field:?}");
}

/// No Dart battery sees it: see the tooltip case.
#[test]
fn a_max_length_set_then_removed_leaves_no_max_len() {
    let mut editor = editor();
    edit_set_form_field_max_length(&mut editor, "both", Some(3)).unwrap();
    edit_set_form_field_max_length(&mut editor, "both", None).unwrap();
    let field = saved_field(&mut editor, "both");
    assert!(!field.contains_key("MaxLen"), "a removed limit stays: {field:?}");
}

/// No Dart battery sees it: see the tooltip case.
#[test]
fn a_background_set_then_removed_leaves_no_bg_and_keeps_the_border() {
    let mut editor = editor();
    edit_set_form_field_background_color(&mut editor, "both", Some(RED)).unwrap();
    edit_set_form_field_background_color(&mut editor, "both", None).unwrap();
    let field = saved_field(&mut editor, "both");
    let entries = mk(&field).expect("the border colour keeps the /MK");
    assert!(!entries.contains_key("BG"), "a removed background stays: {entries:?}");
    assert!(entries.contains_key("BC"), "the border colour was removed too: {entries:?}");
}

/// No Dart battery sees it: see the tooltip case.
#[test]
fn a_border_set_then_removed_leaves_no_bc_and_keeps_the_background() {
    let mut editor = editor();
    edit_set_form_field_border_color(&mut editor, "both", Some(YELLOW)).unwrap();
    edit_set_form_field_border_color(&mut editor, "both", None).unwrap();
    let field = saved_field(&mut editor, "both");
    let entries = mk(&field).expect("the background colour keeps the /MK");
    assert!(!entries.contains_key("BC"), "a removed border stays: {entries:?}");
    assert!(entries.contains_key("BG"), "the background was removed too: {entries:?}");
}

/// An `/MK` left with no entry is dropped.
///
/// No Dart battery sees it: see the tooltip case.
#[test]
fn removing_the_last_colour_drops_the_mk() {
    let mut editor = editor();
    edit_set_form_field_background_color(&mut editor, "bg", None).unwrap();
    let field = saved_field(&mut editor, "bg");
    assert!(mk(&field).is_none(), "an empty /MK stays: {field:?}");

    let mut both = self::editor();
    edit_set_form_field_background_color(&mut both, "both", None).unwrap();
    edit_set_form_field_border_color(&mut both, "both", None).unwrap();
    assert!(mk(&saved_field(&mut both, "both")).is_none());
}

/// A value set after a removal wins, and the other properties are untouched.
///
/// No Dart battery sees it: see the tooltip case.
#[test]
fn a_value_set_after_a_removal_is_written() {
    let mut editor = editor();
    edit_set_form_field_tooltip(&mut editor, "both", None).unwrap();
    edit_set_form_field_tooltip(&mut editor, "both", Some("again")).unwrap();
    edit_set_form_field_max_length(&mut editor, "both", None).unwrap();
    let field = saved_field(&mut editor, "both");
    assert!(matches!(field.get("TU"), Some(Object::String(s)) if s == b"again"), "{field:?}");
    assert!(!field.contains_key("MaxLen"));
    assert!(mk(&field).is_some_and(|m| m.contains_key("BG") && m.contains_key("BC")));
}
