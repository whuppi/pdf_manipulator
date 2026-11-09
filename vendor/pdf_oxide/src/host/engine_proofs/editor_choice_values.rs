//! The editor sets the value of a combo box, a list box and a multi-select
//! list box: `/V` the option, `/I` its index for a list box, the value read
//! back by the engine's own form reader, the widget drawn from it, and a value
//! the field does not offer refused before anything changes.

use super::redact_save::{obj, stream, write_pdf};
use super::support::open_editor_from_bytes;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::error::Error;
use crate::extractors::forms::FormExtractor;
use crate::host::dispatch::{
    doc_form_fields, edit_clear_form_field_value, edit_flatten_forms, edit_set_form_field_choice,
    edit_set_form_field_choices, FormFieldRow,
};
use crate::object::Object;
use crate::rendering::{render_page, RenderOptions};
use crate::PdfDocument;

const COMBO: u32 = 1 << 17;
const EDIT: u32 = 1 << 18;
const MULTI: u32 = 1 << 21;
const PAGE: f32 = 300.0;
/// The colour a selected list row is drawn over, as bytes.
const HIGHLIGHT: [u8; 3] = [153, 191, 217];

/// Where each field sits: the combo box, the single list box, the multi-select
/// list box and the editable combo box.
const COMBO_RECT: [f32; 4] = [20.0, 250.0, 120.0, 270.0];
const LIST_RECT: [f32; 4] = [20.0, 150.0, 120.0, 230.0];
const MULTI_RECT: [f32; 4] = [150.0, 150.0, 250.0, 230.0];

fn widget(id: u32, name: &str, flags: u32, rect: [f32; 4], value: &str) -> (u32, Vec<u8>) {
    obj(
        id,
        &format!(
            "<< /Type /Annot /Subtype /Widget /FT /Ch /T ({name}) /Ff {flags} /V ({value}) \
             /Opt [(red) (green) (blue) (amber)] /Rect [{} {} {} {}] /F 4 /P 3 0 R \
             /DA (/Helv 12 Tf 0 g) >>",
            rect[0], rect[1], rect[2], rect[3]
        ),
    )
}

/// One page with a combo box `combo`, a list box `list`, a multi-select list
/// box `multi` and an editable combo box `free`, each with its options.
fn form() -> Vec<u8> {
    write_pdf(&[
        obj(
            1,
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [6 0 R 7 0 R 8 0 R 9 0 R] \
             /DR << /Font << /Helv 5 0 R >> >> /DA (/Helv 12 Tf 0 g) >> >>",
        ),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
             /Annots [6 0 R 7 0 R 8 0 R 9 0 R] >>",
        ),
        stream(4, b"q Q\n"),
        obj(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        widget(6, "combo", COMBO, COMBO_RECT, "red"),
        widget(7, "list", 0, LIST_RECT, "red"),
        widget(8, "multi", MULTI, MULTI_RECT, "red"),
        widget(9, "free", COMBO | EDIT, [20.0, 100.0, 120.0, 120.0], "red"),
    ])
}

fn saved(editor: &mut DocumentEditor) -> Vec<u8> {
    editor
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            object_streams: false,
            ..SaveOptions::full_rewrite()
        })
        .expect("the editor saves")
}

fn field_row(bytes: &[u8], name: &str) -> FormFieldRow {
    let mut doc = PdfDocument::from_bytes(bytes.to_vec()).expect("the file parses");
    doc_form_fields(&mut doc)
        .expect("the fields read")
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("field {name} is in the file"))
}

/// The field's own dictionary in a saved file.
fn field_dict(bytes: &[u8], name: &str) -> std::collections::HashMap<String, Object> {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).expect("the file parses");
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

fn indexes(dict: &std::collections::HashMap<String, Object>) -> Option<Vec<i64>> {
    match dict.get("I") {
        Some(Object::Array(items)) => Some(
            items
                .iter()
                .map(|i| match i {
                    Object::Integer(n) => *n,
                    other => panic!("an index is an integer, got {other:?}"),
                })
                .collect(),
        ),
        _ => None,
    }
}

fn render(bytes: &[u8]) -> image::RgbaImage {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).expect("the file parses");
    let rendered = render_page(&doc, 0, &RenderOptions::with_dpi(72)).expect("the page renders");
    let img = image::load_from_memory(&rendered.data).expect("the render decodes").to_rgba8();
    assert_eq!(img.dimensions(), (PAGE as u32, PAGE as u32), "one pixel a point");
    img
}

/// Whether any pixel inside the page-point rectangle is darker than mid gray.
fn has_ink(img: &image::RgbaImage, [l, b, r, t]: [f32; 4]) -> bool {
    (l as u32..r as u32).any(|x| {
        ((PAGE - t) as u32..(PAGE - b) as u32).any(|y| img.get_pixel(x, y).0[..3].iter().all(|c| *c < 100))
    })
}

/// Whether the pixel at page point (`x`, `y`) is the selection highlight.
fn is_highlight(img: &image::RgbaImage, x: f32, y: f32) -> bool {
    let px = img.get_pixel(x as u32, (PAGE - y) as u32).0;
    (0..3).all(|c| (px[c] as i32 - HIGHLIGHT[c] as i32).abs() <= 6)
}

/// The page point inside row `row` of a list box with the options of `form()`,
/// left of the row's text, where only the highlight can be.
fn row_point(rect: [f32; 4], row: usize) -> (f32, f32) {
    let row_height = 12.0 * 1.15;
    (rect[0] + 3.0, rect[3] - 2.0 - row as f32 * row_height - row_height / 2.0)
}

fn editor() -> DocumentEditor {
    open_editor_from_bytes(&form(), "editor_choice_values")
}

/// No Dart battery sees it: a battery reads the value back through the same
/// reader, and none puts a pixel of a widget the value drew under a test.
#[test]
fn a_combo_box_takes_an_option_reads_back_and_draws_it() {
    let mut editor = editor();
    edit_set_form_field_choice(&mut editor, "combo", "green").expect("green is an option");
    let bytes = saved(&mut editor);

    let row = field_row(&bytes, "combo");
    assert_eq!((row.value_kind, row.text.as_str()), ("choice", "green"));
    let dict = field_dict(&bytes, "combo");
    assert_eq!(indexes(&dict), None, "a combo box carries no /I");
    assert!(has_ink(&render(&bytes), COMBO_RECT), "the chosen option is drawn in the box");
}

/// No Dart battery sees it: the saved /I entry and the highlighted rows are
/// in the file's dictionary and its pixels.
#[test]
fn a_list_box_takes_an_option_with_its_index_and_highlights_the_row() {
    let mut editor = editor();
    edit_set_form_field_choice(&mut editor, "list", "blue").expect("blue is an option");
    let bytes = saved(&mut editor);

    let row = field_row(&bytes, "list");
    assert_eq!((row.value_kind, row.text.as_str()), ("choice", "blue"));
    assert_eq!(indexes(&field_dict(&bytes, "list")), Some(vec![2]));

    let img = render(&bytes);
    for (at, selected) in [(0, false), (1, false), (2, true), (3, false)] {
        let (x, y) = row_point(LIST_RECT, at);
        assert_eq!(is_highlight(&img, x, y), selected, "row {at}");
    }
}

/// No Dart battery sees it: see the list box case above.
#[test]
fn a_multi_select_list_box_takes_several_options_as_an_array_and_ascending_indexes() {
    let mut editor = editor();
    let values = vec!["blue".to_string(), "red".to_string()];
    edit_set_form_field_choices(&mut editor, "multi", &values).expect("both are options");
    let bytes = saved(&mut editor);

    let row = field_row(&bytes, "multi");
    assert_eq!(row.value_kind, "multiChoice");
    assert_eq!(row.choices, values);
    let dict = field_dict(&bytes, "multi");
    assert!(matches!(dict.get("V"), Some(Object::Array(items)) if items.len() == 2));
    assert_eq!(indexes(&dict), Some(vec![0, 2]));

    let img = render(&bytes);
    for (at, selected) in [(0, true), (1, false), (2, true), (3, false)] {
        let (x, y) = row_point(MULTI_RECT, at);
        assert_eq!(is_highlight(&img, x, y), selected, "row {at}");
    }
}

/// No Dart battery sees it: the flattened page's pixels, from a value set and
/// flattened before any save.
#[test]
fn a_multi_select_value_set_and_flattened_in_one_session_draws_its_rows() {
    let mut editor = editor();
    let values = vec!["green".to_string(), "amber".to_string()];
    edit_set_form_field_choices(&mut editor, "multi", &values).expect("both are options");
    edit_flatten_forms(&mut editor, None).expect("the forms flatten");
    let bytes = saved(&mut editor);

    let img = render(&bytes);
    for (at, selected) in [(0, false), (1, true), (2, false), (3, true)] {
        let (x, y) = row_point(MULTI_RECT, at);
        assert_eq!(is_highlight(&img, x, y), selected, "row {at}");
    }
}

/// No Dart battery sees it: the saved dictionary has no /V and no /I.
#[test]
fn clearing_a_value_removes_the_value_and_the_indexes() {
    let mut editor = editor();
    edit_set_form_field_choice(&mut editor, "list", "green").unwrap();
    edit_clear_form_field_value(&mut editor, "list").expect("the value clears");
    edit_clear_form_field_value(&mut editor, "combo").expect("the value clears");
    let bytes = saved(&mut editor);

    for name in ["list", "combo"] {
        let dict = field_dict(&bytes, name);
        assert!(!dict.contains_key("V") && !dict.contains_key("I"), "{name}: {dict:?}");
        assert_eq!(field_row(&bytes, name).value_kind, "none", "{name}");
    }
}

/// An editable combo box takes any text; the others refuse what they do not
/// list, and the editor is as it was.
///
/// No Dart battery sees it: it compares the saved bytes of a refused edit
/// against an untouched editor's.
#[test]
fn a_value_the_field_does_not_offer_is_refused_and_changes_nothing() {
    let mut editor = editor();
    let before = saved(&mut editor);

    for (name, result) in [
        ("combo", edit_set_form_field_choice(&mut editor, "combo", "purple")),
        ("list", edit_set_form_field_choice(&mut editor, "list", "purple")),
        ("multi", edit_set_form_field_choices(&mut editor, "multi", &["red".into(), "purple".into()])),
        ("multi", edit_set_form_field_choices(&mut editor, "multi", &["red".into(), "red".into()])),
        ("list", edit_set_form_field_choices(&mut editor, "list", &["red".into()])),
        ("combo", edit_set_form_field_choices(&mut editor, "combo", &["red".into()])),
        ("nothing", edit_set_form_field_choice(&mut editor, "nothing", "red")),
    ] {
        assert!(matches!(result, Err(Error::InvalidOperation(_))), "{name}: {result:?}");
    }
    assert!(saved(&mut editor) == before, "a refused value changed the editor");

    edit_set_form_field_choice(&mut editor, "free", "anything typed").expect("an editable combo takes text");
    let bytes = saved(&mut editor);
    assert_eq!(field_row(&bytes, "free").text, "anything typed");
}
