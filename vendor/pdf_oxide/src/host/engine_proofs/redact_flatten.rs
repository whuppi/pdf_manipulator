//! A flattened widget on a redacted page: the flattened appearance is drawn
//! by the page's content before the redaction decides it, so a value under a
//! box is removed with the rest and the twin outside the box stays.

use super::redact_path_save::{assert_absent_everywhere, save};
use super::redact_save::{obj, stream, write_pdf};
use super::support::{contains, open_editor_from_bytes, stream_object};
use crate::PdfDocument;

const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// A page with two text widgets that carry appearance streams: `fin` under
/// the box with the value `FLATSECRET`, `fout` outside it with `FLATKEEP`.
fn fixture() -> Vec<u8> {
    let form = |id: u32, text: &str| {
        (
            id,
            stream_object(
                id,
                "/Type /XObject /Subtype /Form /BBox [0 0 100 20] \
                 /Resources << /Font << /Helv 12 0 R >> >>",
                format!("BT /Helv 12 Tf 2 4 Td ({text}) Tj ET\n").as_bytes(),
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
        obj(
            8,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (fin) /V (FLATSECRET) \
             /Rect [20 142 120 162] /F 4 /P 3 0 R /AP << /N 10 0 R >> >>",
        ),
        obj(
            9,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (fout) /V (FLATKEEP) \
             /Rect [20 20 120 40] /F 4 /P 3 0 R /AP << /N 11 0 R >> >>",
        ),
        form(10, "FLATSECRET"),
        form(11, "FLATKEEP"),
        obj(12, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    ])
}

/// The value under the box is in no decoded stream of the saved file, the twin
/// outside the box is, no widget is left on the page, and the page reads as
/// the twin alone. Both orders of queueing give one result: the redaction
/// applied before the flattening, and after it. The flattened forms are
/// objects the saved file holds; no battery sees which of them it kept.
#[test]
fn a_flattened_value_under_the_box_is_removed_and_its_twin_stays() {
    for flatten_first in [true, false] {
        for garbage_collect in [true, false] {
            let mut editor = open_editor_from_bytes(&fixture(), "redact_flatten");
            if flatten_first {
                editor.flatten_forms_on_page(0).unwrap();
            }
            editor.add_redaction(0, BOX, Some(RED)).unwrap();
            editor.apply_redactions_destructive(Default::default()).unwrap();
            if !flatten_first {
                editor.flatten_forms_on_page(0).unwrap();
            }
            let out = save(&mut editor, garbage_collect);
            let case = format!("flatten_first={flatten_first} gc={garbage_collect}");
            assert_absent_everywhere(&out, b"FLATSECRET", garbage_collect);
            assert!(contains(&out, b"FLATKEEP"), "{case}: the twin is gone");
            let doc = PdfDocument::from_bytes(out).unwrap();
            let text = doc.extract_text(0).unwrap();
            assert!(text.contains("FLATKEEP"), "{case}: the twin does not read: {text:?}");
            assert!(!text.contains("FLATSECRET"), "{case}: the value reads: {text:?}");
            let page = doc.load_object(doc.get_page_ref(0).unwrap()).unwrap();
            let annots = page.as_dict().unwrap().get("Annots");
            assert!(
                annots.is_none_or(|a| a.as_array().is_some_and(|list| list.is_empty())),
                "{case}: a widget is left: {annots:?}"
            );
        }
    }
}
