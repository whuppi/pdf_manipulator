//! Stamps and flattening over a set of pages are all or nothing: every page is
//! checked before the first is changed, the stamps land on the pages the
//! output indexes name after pages were removed, and the pages flattened are
//! the ones asked for.

use super::support::{open_editor_from_bytes, pdf_from_objects, stream_object};
use crate::annotation_types::AnnotationSubtype;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::error::Error;
use crate::host::dispatch::{
    edit_add_image_stamp, edit_add_stamp, edit_flatten_annotations, edit_flatten_forms,
    edit_select_pages,
};
use crate::PdfDocument;

/// Three pages, each with a text widget `f0`, `f1`, `f2` and a text note.
fn three_pages() -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [10 0 R 11 0 R 12 0 R] \
          /DA (/Helv 12 Tf 0 g) /DR << /Font << /Helv 9 0 R >> >> >> >>\nendobj\n"
            .to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>\nendobj\n".to_vec(),
    ];
    for (page, widget, note) in [(3, 10, 13), (4, 11, 14), (5, 12, 15)] {
        objects.push(
            format!(
                "{page} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] \
                 /Contents 6 0 R /Annots [{widget} 0 R {note} 0 R] >>\nendobj\n"
            )
            .into_bytes(),
        );
    }
    objects.push(stream_object(6, "", b"q Q\n"));
    // Objects 7 and 8 are unused: the placeholders keep the numbering.
    objects.push(b"7 0 obj\nnull\nendobj\n".to_vec());
    objects.push(b"8 0 obj\nnull\nendobj\n".to_vec());
    objects.push(b"9 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec());
    for (widget, page, name) in [(10, 3, "f0"), (11, 4, "f1"), (12, 5, "f2")] {
        objects.push(
            format!(
                "{widget} 0 obj\n<< /Type /Annot /Subtype /Widget /FT /Tx /T ({name}) /V (value) \
                 /Rect [20 20 120 40] /F 4 /P {page} 0 R /DA (/Helv 12 Tf 0 g) >>\nendobj\n"
            )
            .into_bytes(),
        );
    }
    for note in [13, 14, 15] {
        objects.push(
            format!(
                "{note} 0 obj\n<< /Type /Annot /Subtype /Text /Rect [150 150 170 170] \
                 /Contents (note) /AP << /N 16 0 R >> >>\nendobj\n"
            )
            .into_bytes(),
        );
    }
    objects.push(stream_object(
        16,
        "/Type /XObject /Subtype /Form /BBox [0 0 20 20]",
        b"0 0 20 20 re f\n",
    ));
    pdf_from_objects(&objects)
}

fn editor() -> DocumentEditor {
    open_editor_from_bytes(&three_pages(), "editor_content_sets")
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

/// The number of annotations of `subtype` on page `page` of a saved file.
fn count(bytes: &[u8], page: usize, subtype: AnnotationSubtype) -> usize {
    PdfDocument::from_bytes(bytes.to_vec())
        .expect("the file parses")
        .get_annotations(page)
        .expect("the annotations read")
        .iter()
        .filter(|a| a.subtype_enum == subtype)
        .count()
}

fn png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 20, 20, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).expect("the PNG encodes");
    out.into_inner()
}

/// No Dart battery sees it: the page a stamp lands on after pages were removed
/// is a source-page lookup in the editor, and a Dart battery asks only the
/// stamp's page count.
#[test]
fn an_image_stamp_lands_on_the_pages_the_output_indexes_name() {
    let mut editor = editor();
    edit_select_pages(&mut editor, &[1, 2]).expect("the first page goes");
    edit_add_image_stamp(&mut editor, &[0, 1], png(), 10.0, 10.0, 40.0, 40.0, 1.0)
        .expect("both pages stamp");
    let bytes = saved(&mut editor);
    assert_eq!(count(&bytes, 0, AnnotationSubtype::Stamp), 1, "output page 0");
    assert_eq!(count(&bytes, 1, AnnotationSubtype::Stamp), 1, "output page 1");
}

/// No Dart battery sees it: see the case above.
#[test]
fn two_image_stamps_on_one_page_both_stay() {
    let mut editor = editor();
    edit_add_image_stamp(&mut editor, &[0], png(), 10.0, 10.0, 40.0, 40.0, 1.0).unwrap();
    edit_add_image_stamp(&mut editor, &[0], png(), 60.0, 10.0, 40.0, 40.0, 0.5).unwrap();
    let bytes = saved(&mut editor);
    assert_eq!(count(&bytes, 0, AnnotationSubtype::Stamp), 2);
}

/// Standard stamps over a set of pages.
///
/// No Dart battery sees it: see the first case.
#[test]
fn a_standard_stamp_lands_on_every_page_of_the_set_and_no_other() {
    let mut editor = editor();
    edit_add_stamp(&mut editor, &[0, 2], "Draft", 10.0, 10.0, 80.0, 30.0, 1.0).expect("both stamp");
    let bytes = saved(&mut editor);
    let stamps: Vec<usize> = (0..3).map(|p| count(&bytes, p, AnnotationSubtype::Stamp)).collect();
    assert_eq!(stamps, vec![1, 0, 1]);
}

/// A set with one bad page changes no page.
///
/// No Dart battery sees it: it compares the saved bytes of a refused call
/// against an untouched editor's.
#[test]
fn a_bad_page_set_stamps_and_flattens_no_page() {
    let mut editor = editor();
    let before = saved(&mut editor);
    let refused = [
        edit_add_stamp(&mut editor, &[0, 7], "Draft", 10.0, 10.0, 80.0, 30.0, 1.0),
        edit_add_stamp(&mut editor, &[0, 0], "Draft", 10.0, 10.0, 80.0, 30.0, 1.0),
        edit_add_stamp(&mut editor, &[0, 1], "NoSuchStamp", 10.0, 10.0, 80.0, 30.0, 1.0),
        edit_add_image_stamp(&mut editor, &[0, 7], png(), 10.0, 10.0, 40.0, 40.0, 1.0),
        edit_add_image_stamp(&mut editor, &[0, 1], b"not an image".to_vec(), 10.0, 10.0, 40.0, 40.0, 1.0),
        edit_flatten_forms(&mut editor, Some(&[0, 7])),
        edit_flatten_annotations(&mut editor, Some(&[1, 1])),
        edit_flatten_forms(&mut editor, Some(&[])),
    ];
    for (at, result) in refused.iter().enumerate() {
        assert!(
            matches!(result, Err(Error::InvalidOperation(_)) | Err(Error::InvalidPdf(_))),
            "call {at}: {result:?}"
        );
    }
    assert!(saved(&mut editor) == before, "a refused call changed the editor");
}

/// Flattening a set flattens those pages' widgets and leaves the others'.
///
/// No Dart battery sees it: see the first case.
#[test]
fn flattening_a_set_of_pages_flattens_only_those_widgets() {
    let mut editor = editor();
    edit_flatten_forms(&mut editor, Some(&[1])).expect("page one flattens");
    let bytes = saved(&mut editor);
    let widgets: Vec<usize> = (0..3).map(|p| count(&bytes, p, AnnotationSubtype::Widget)).collect();
    assert_eq!(widgets, vec![1, 0, 1]);
}

/// Flattening every page after a page was removed flattens the pages that are
/// left.
///
/// No Dart battery sees it: see the first case.
#[test]
fn flattening_every_page_after_a_page_was_removed_flattens_the_rest() {
    let mut editor = editor();
    edit_select_pages(&mut editor, &[1, 2]).expect("the first page goes");
    edit_flatten_forms(&mut editor, None).unwrap();
    edit_flatten_annotations(&mut editor, None).unwrap();
    let bytes = saved(&mut editor);
    for page in 0..2 {
        assert_eq!(count(&bytes, page, AnnotationSubtype::Widget), 0, "widgets on output page {page}");
        assert_eq!(count(&bytes, page, AnnotationSubtype::Text), 0, "notes on output page {page}");
    }
}
