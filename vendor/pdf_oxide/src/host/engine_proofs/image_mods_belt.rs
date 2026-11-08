//! Image bounds edited on the belt: the page renders the image at its new
//! bounds and the rest as it was, on a page redacted too the secret is gone,
//! and a content stream that cannot be read fails the save instead of being
//! written as it was.

use super::redact_path_save::{assert_absent_everywhere, save};
use super::redact_save::{obj, render, stream, write_pdf, DPI, PAGE};
use super::support::{open_editor_from_bytes, stream_object};

const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// A page with a gray inline image at 200,100 (60 points square), a solid red
/// image `Im1` at 10,10 (50 points square) and `content` after them.
fn fixture(content: &[u8]) -> Vec<u8> {
    let mut page = b"q 60 0 0 60 200 100 cm BI /W 2 /H 2 /BPC 8 /CS /G ID \x10\x80\x40\xf0 EI Q\n\
                     q 50 0 0 50 10 10 cm /Im1 Do Q\n"
        .to_vec();
    page.extend_from_slice(content);
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /XObject << /Im1 6 0 R >> /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(4, &page),
        obj(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        (
            6,
            stream_object(
                6,
                "/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB \
                 /BitsPerComponent 8",
                &[255, 0, 0].repeat(4),
            ),
        ),
    ])
}

fn at(img: &image::RgbaImage, x: f32, y: f32) -> [u8; 4] {
    let scale = DPI as f32 / 72.0;
    img.get_pixel((x * scale) as u32, ((PAGE - y) * scale) as u32).0
}

fn is_red(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] < 64 && p[2] < 64
}

/// The red image moves to 200,200 at 80 points; the gray inline image is
/// where it was; with a redaction on the page the secret text is gone too and
/// the image still lands at its new bounds.
#[test]
fn an_image_moves_to_its_new_bounds_and_the_rest_of_the_page_stays() {
    for redacted in [false, true] {
        for garbage_collect in [true, false] {
            let source = fixture(b"BT /F1 12 Tf 1 0 0 1 20 150 Tm (SECRETWORD) Tj ET\n");
            let before = render(&source);
            assert!(is_red(at(&before, 30.0, 30.0)), "the fixture draws the image at 10,10");
            let mut editor = open_editor_from_bytes(&source, "image_mods_belt");
            editor.set_image_bounds(0, "Im1", 200.0, 200.0, 80.0, 80.0).unwrap();
            if redacted {
                editor.add_redaction(0, BOX, Some(RED)).unwrap();
                editor.apply_redactions_destructive(Default::default()).unwrap();
            }
            let out = save(&mut editor, garbage_collect);
            let after = render(&out);
            let case = format!("redacted={redacted} gc={garbage_collect}");
            assert!(is_red(at(&after, 240.0, 240.0)), "{case}: the image is not at its new bounds");
            assert!(!is_red(at(&after, 30.0, 30.0)), "{case}: the image is still where it was");
            assert_eq!(
                at(&before, 230.0, 130.0),
                at(&after, 230.0, 130.0),
                "{case}: the inline image changed"
            );
            if redacted {
                assert_absent_everywhere(&out, b"SECRETWORD", garbage_collect);
            }
        }
    }
}

/// A content stream the belt cannot read in full (an inline image that never
/// ends) fails the save; the original content is not written in its place.
#[test]
fn an_unreadable_content_stream_fails_the_save() {
    let source = fixture(b"BI /W 2 /H 2 /BPC 8 /CS /G ID \x10\x80");
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&source, "image_mods_error");
        editor.set_image_bounds(0, "Im1", 200.0, 200.0, 80.0, 80.0).unwrap();
        let result = editor.save_to_bytes_with_options(crate::editor::SaveOptions {
            compress: false,
            garbage_collect,
            object_streams: false,
            ..crate::editor::SaveOptions::full_rewrite()
        });
        assert!(result.is_err(), "gc={garbage_collect}: the save wrote a page it could not read");
    }
}
