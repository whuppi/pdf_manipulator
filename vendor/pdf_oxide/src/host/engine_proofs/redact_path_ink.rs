//! The ink of a stroke through a region, read from a saved file's render. The
//! stroke's width comes from an ExtGState, so the reach the cut uses is the
//! one the page sets. The render is the proof: a centreline that stops at the
//! padded edge leaves its ink in the band the padding covers, and only the
//! pixels show it.

use super::redact_save::{obj, render, stream, write_pdf, DPI, PAGE};
use super::support::open_editor_from_bytes;
use crate::editor::SaveOptions;
use crate::redaction::{RedactionOptions, RedactionRegion};

/// The box every proof redacts; its padded region is two points wider on each side.
const BOX: [f32; 4] = [100.0, 140.0, 200.0, 160.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// A horizontal stroke at y = 161, four points wide: its ink covers y 159 to
/// 163, so it crosses the box's top band. The width is set by `4 w` in the
/// content, because the renderer does not read `/LW` from an ExtGState; the
/// `gs` still sets the same width in the pass, which the operator tests cover.
fn fixture() -> Vec<u8> {
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /ExtGState << /GS1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(4, b"/GS1 gs 4 w 0 161 m 300 161 l S\n"),
        obj(5, "<< /LW 4 >>"),
    ])
}

/// Whether the page point (`x`, `y`) lies in `rect` grown by `grow` on every side.
fn inside(x: f32, y: f32, rect: [f32; 4], grow: f32) -> bool {
    x >= rect[0] - grow && x <= rect[2] + grow && y >= rect[1] - grow && y <= rect[3] + grow
}

/// Redacts the box, saves, renders, and reads the pixels: inside the box every
/// pixel is the fill, in the padding's band outside the box no pixel is ink,
/// and outside the padded box plus three points the render is the source's.
#[test]
fn stroke_ink_stays_out_of_the_padded_box_and_matches_the_source_outside() {
    let src = fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_stroke_ink");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    let after = render(&out);

    let scale = DPI as f32 / 72.0;
    let padded = RedactionRegion::from_rect(BOX[0], BOX[1], BOX[2], BOX[3], None)
        .padded_rect(RedactionOptions::default().edge_padding);
    let padded = [padded.left(), padded.top(), padded.right(), padded.bottom()];
    let (mut fill_pixels, mut band_pixels, mut outside_pixels) = (0usize, 0usize, 0usize);
    for y in 0..after.height() {
        for x in 0..after.width() {
            let px = x as f32 / scale;
            let py = PAGE - y as f32 / scale;
            let pixel = after.get_pixel(x, y).0;
            if inside(px, py, BOX, -1.0) {
                fill_pixels += 1;
                assert_eq!(pixel, [255, 0, 0, 255], "box pixel ({x}, {y}) is not the fill");
            } else if inside(px, py, padded, 0.0) {
                // The band is half a point wide, so its pixels also carry the
                // overlay's antialiased red edge; ink is the dark stroke colour.
                band_pixels += 1;
                let dark = pixel[0] < 128 && pixel[1] < 128 && pixel[2] < 128;
                assert!(!(dark && pixel[3] > 0), "ink in the padding band at ({x}, {y}): {pixel:?}");
            } else if !inside(px, py, padded, 3.0) {
                outside_pixels += 1;
                assert_eq!(
                    before.get_pixel(x, y),
                    after.get_pixel(x, y),
                    "pixel ({x}, {y}) outside the padded box differs from the source"
                );
            }
        }
    }
    assert!(pixel_is_ink(&before, 50.0, 161.0), "the fixture draws the stroke");
    assert!(fill_pixels > 0 && band_pixels > 0 && outside_pixels > 0, "every region was read");
}

/// A 40-point stroke under `2 0 1 1 0 0 cm`, which maps user (x, y) to page
/// (2x + y, y). The line runs along user (-1, 2), which lands on the vertical
/// page line x = 56; its ink reaches sqrt(5) * 20 = 44.7 points to each side,
/// the matrix's largest stretch. The longest column of the matrix, 2, would
/// bound the reach at 40 and leave the line uncut, 56 + 44.7 = 100.7 being
/// inside the box.
fn sheared_fixture() -> Vec<u8> {
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R >>"),
        stream(4, b"2 0 1 1 0 0 cm 40 w 28 0 m -122 300 l S\n"),
    ])
}

/// A thick stroke under a sheared matrix, straddling the box by its width
/// alone, leaves no ink in the box or its padding band, and still draws away
/// from the box.
#[test]
fn stroke_under_a_sheared_matrix_leaves_no_ink_in_the_padded_box() {
    let src = sheared_fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_sheared_stroke");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    let after = render(&out);

    let scale = DPI as f32 / 72.0;
    let padded = RedactionRegion::from_rect(BOX[0], BOX[1], BOX[2], BOX[3], None)
        .padded_rect(RedactionOptions::default().edge_padding);
    let padded = [padded.left(), padded.top(), padded.right(), padded.bottom()];
    let (mut fill_pixels, mut band_pixels) = (0usize, 0usize);
    for y in 0..after.height() {
        for x in 0..after.width() {
            let px = x as f32 / scale;
            let py = PAGE - y as f32 / scale;
            let pixel = after.get_pixel(x, y).0;
            if inside(px, py, BOX, -1.0) {
                fill_pixels += 1;
                assert_eq!(pixel, [255, 0, 0, 255], "box pixel ({x}, {y}) is not the fill");
            } else if inside(px, py, padded, 0.0) {
                band_pixels += 1;
                let dark = pixel[0] < 128 && pixel[1] < 128 && pixel[2] < 128;
                assert!(!(dark && pixel[3] > 0), "ink in the padding band at ({x}, {y}): {pixel:?}");
            }
        }
    }
    assert!(pixel_is_ink(&before, 99.0, 150.0), "the fixture's ink reaches into the box");
    assert!(pixel_is_ink(&after, 56.0, 40.0), "the stroke away from the box was removed");
    assert!(fill_pixels > 0 && band_pixels > 0, "every region was read");
}

/// Whether the render has a dark, opaque pixel at page point (`x`, `y`).
fn pixel_is_ink(img: &image::RgbaImage, x: f32, y: f32) -> bool {
    let scale = DPI as f32 / 72.0;
    let [r, g, b, a] = img.get_pixel((x * scale) as u32, ((PAGE - y) * scale) as u32).0;
    a == 255 && r < 128 && g < 128 && b < 128
}
