//! The host's redaction ops: a mark with no overlay draws no box, `redact`
//! applies only its own regions and keeps the marks queued beside them, and a
//! refused `redact` changes nothing.

use crate::cancel::CancelCheck;
use super::support::{open_editor_from_bytes, pdf_from_objects, stream_object};
use crate::editor::{DocumentEditor, SaveOptions};
use crate::error::Error;
use crate::host::dispatch::{
    edit_add_redaction, edit_apply_redactions, edit_clear_redactions, edit_redact,
};
use crate::rendering::{render_page, RenderOptions};
use crate::PdfDocument;

const PAGE: f32 = 300.0;
/// Around the line SECRET, and around the line PUBLIC.
const SECRET_BOX: [f32; 4] = [15.0, 135.0, 140.0, 170.0];
const PUBLIC_BOX: [f32; 4] = [15.0, 55.0, 140.0, 90.0];

/// One page with the lines SECRET at y 150 and PUBLIC at y 60.
fn page() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
          /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n"
            .to_vec(),
        stream_object(
            4,
            "",
            b"BT /F1 24 Tf 1 0 0 1 20 150 Tm (SECRET) Tj 1 0 0 1 20 60 Tm (PUBLIC) Tj ET\n",
        ),
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ])
}

fn editor() -> DocumentEditor {
    open_editor_from_bytes(&page(), "editor_redact_overlay")
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

fn text(bytes: &[u8]) -> String {
    PdfDocument::from_bytes(bytes.to_vec()).unwrap().extract_text(0).unwrap()
}

fn render(bytes: &[u8]) -> image::RgbaImage {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    let rendered = render_page(&doc, 0, &RenderOptions::with_dpi(72)).unwrap();
    let img = image::load_from_memory(&rendered.data).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (PAGE as u32, PAGE as u32), "one pixel a point");
    img
}

/// The number of pixels of the page-point box that are not white.
fn ink_in(img: &image::RgbaImage, [l, b, r, t]: [f32; 4]) -> usize {
    let mut count = 0;
    for x in l as u32..r as u32 {
        for y in (PAGE - t) as u32..(PAGE - b) as u32 {
            if img.get_pixel(x, y).0[..3] != [255, 255, 255] {
                count += 1;
            }
        }
    }
    count
}

/// No Dart battery sees it: whether a region carries no ink is a count of the
/// rendered page's pixels, and whether the text is out of the file is the
/// extractor's answer on the saved bytes.
#[test]
fn an_overlay_of_none_removes_the_text_and_draws_no_box() {
    let mut plain = editor();
    let before = ink_in(&render(&saved(&mut plain)), SECRET_BOX);
    assert!(before > 0, "the secret line has ink");

    let mut erased = editor();
    edit_redact(&mut erased, 0, &[SECRET_BOX], None, &CancelCheck::none()).expect("the region redacts");
    let bytes = saved(&mut erased);
    assert!(!text(&bytes).contains("SECRET"), "the text is still in the file: {:?}", text(&bytes));
    assert!(text(&bytes).contains("PUBLIC"), "the other line was removed");
    let img = render(&bytes);
    assert_eq!(ink_in(&img, SECRET_BOX), 0, "an erased region draws no box");
    assert!(ink_in(&img, PUBLIC_BOX) > 0, "the line outside kept its ink");

    let mut boxed = editor();
    edit_redact(&mut boxed, 0, &[SECRET_BOX], Some([0.0, 0.0, 0.0]), &CancelCheck::none()).expect("the region redacts");
    let filled = ink_in(&render(&saved(&mut boxed)), SECRET_BOX);
    assert!(filled > (SECRET_BOX[2] - SECRET_BOX[0]) as usize * 20, "a fill overlay draws its box: {filled}");
}

/// A mark queued without an overlay draws none when it is applied either.
///
/// No Dart battery sees it: see the case above.
#[test]
fn a_mark_without_an_overlay_draws_no_box_when_applied() {
    let mut editor = editor();
    edit_add_redaction(&mut editor, 0, SECRET_BOX, None).unwrap();
    edit_add_redaction(&mut editor, 0, PUBLIC_BOX, Some([0.0, 0.0, 0.0])).unwrap();
    edit_apply_redactions(&mut editor, &CancelCheck::none()).expect("the marks apply");
    let bytes = saved(&mut editor);
    assert!(!text(&bytes).contains("SECRET") && !text(&bytes).contains("PUBLIC"));
    let img = render(&bytes);
    assert_eq!(ink_in(&img, SECRET_BOX), 0, "the mark with no overlay drew a box");
    assert!(ink_in(&img, PUBLIC_BOX) > 0, "the mark with a fill drew none");
}

/// `redact` applies its own regions and leaves the queued marks, and a later
/// apply of those marks on the same page keeps what `redact` removed.
///
/// No Dart battery sees it: the second apply replaces the page's record in the
/// editor, and only the saved page's text shows whether the first removal
/// survived it.
#[test]
fn redact_keeps_the_marks_queued_and_survives_a_later_apply_on_the_page() {
    let mut editor = editor();
    edit_add_redaction(&mut editor, 0, PUBLIC_BOX, None).unwrap();
    let report = edit_redact(&mut editor, 0, &[SECRET_BOX], None, &CancelCheck::none()).expect("the region redacts");
    assert_eq!(report.regions, 1, "the report counts the one region of this call");
    assert!(report.glyphs_removed > 0);
    assert_eq!(editor.redaction_count(0).unwrap(), 1, "the queued mark stays queued");
    let after_redact = text(&saved(&mut editor));
    assert!(!after_redact.contains("SECRET"), "{after_redact:?}");
    assert!(after_redact.contains("PUBLIC"), "redact applied a mark it does not own: {after_redact:?}");

    let applied = edit_apply_redactions(&mut editor, &CancelCheck::none()).expect("the mark applies");
    assert_eq!(applied.regions, 1, "the apply reports its own mark only");
    let after_apply = text(&saved(&mut editor));
    assert!(!after_apply.contains("PUBLIC"), "{after_apply:?}");
    assert!(!after_apply.contains("SECRET"), "the apply brought the redacted text back: {after_apply:?}");
}

/// Two `redact` calls on one page each keep the other's removal.
///
/// No Dart battery sees it: see the case above.
#[test]
fn two_redactions_on_one_page_both_hold() {
    let mut editor = editor();
    edit_redact(&mut editor, 0, &[SECRET_BOX], None, &CancelCheck::none()).unwrap();
    let second = edit_redact(&mut editor, 0, &[PUBLIC_BOX], None, &CancelCheck::none()).unwrap();
    assert_eq!(second.regions, 1);
    let out = text(&saved(&mut editor));
    assert!(!out.contains("SECRET") && !out.contains("PUBLIC"), "{out:?}");
}

/// `clearRedactions` drops every queued mark and leaves the content alone.
///
/// No Dart battery sees it: see the case above.
#[test]
fn clearing_the_marks_drops_them_all_and_applies_nothing() {
    let mut editor = editor();
    edit_add_redaction(&mut editor, 0, SECRET_BOX, None).unwrap();
    edit_add_redaction(&mut editor, 0, PUBLIC_BOX, None).unwrap();
    assert_eq!(editor.redaction_count(0).unwrap(), 2);
    edit_clear_redactions(&mut editor);
    assert_eq!(editor.redaction_count(0).unwrap(), 0);
    edit_apply_redactions(&mut editor, &CancelCheck::none()).expect("nothing to apply");
    let out = text(&saved(&mut editor));
    assert!(out.contains("SECRET") && out.contains("PUBLIC"), "{out:?}");
}

/// A page the pass refuses is refused by `redact` too, and the editor and its
/// queued marks are as they were.
///
/// No Dart battery sees it: its documents come from the Dart builder, which
/// writes no path the pass cannot place.
#[test]
fn a_refused_redact_changes_nothing() {
    let bytes = pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
          >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", b"100 100 m 200 100 l\n"),
    ]);
    let mut editor = open_editor_from_bytes(&bytes, "editor_redact_refused");
    edit_add_redaction(&mut editor, 0, SECRET_BOX, None).unwrap();
    let before = saved(&mut editor);
    let error = edit_redact(&mut editor, 0, &[[90.0, 90.0, 210.0, 110.0]], None, &CancelCheck::none())
        .expect_err("the page is refused");
    assert!(matches!(error, Error::Unsupported(_)), "{error:?}");
    assert!(saved(&mut editor) == before, "a refused redact changed the editor");
    assert_eq!(editor.redaction_count(0).unwrap(), 1, "the queued mark is gone");
}
