//! What a saved file holds of the paths, clips, text-as-clip and Type 3 glyphs
//! under a region, and what the page renders as. Every covered coordinate is
//! written once, in a mark under the box, so its bytes exist in the source only
//! there; the outside marks keep theirs. The coordinates are exact in binary, so
//! the writer's f32 round trip keeps every digit. The render proofs read the
//! fills, the text clip and the glyphs back from pixels.

use super::redact_save::{obj, render, stream, write_pdf, DPI, PAGE};
use super::support::{contains, open_editor_from_bytes, stream_object};
use crate::editor::{DocumentEditor, SaveOptions};
use crate::object::ObjectRef;
use crate::redaction::{RedactionOptions, RedactionRegion};
use crate::PdfDocument;

pub(super) const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
pub(super) const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// Coordinates of the marks under the box: a filled square, a clip inside, and
/// the inside vertex of a fill that straddles the box.
const COVERED: &[&str] = &[
    "41.125000",
    "58.625000",
    "141.125000",
    "158.250000",
    "45.250000",
    "55.875000",
    "140.125000",
    "160.375000",
    "118.250000",
];

/// Coordinates of the marks outside the box, which the saved file keeps.
const PUBLIC: &[&str] = &["20.500000", "100.250000", "40.750000", "200.000000"];

fn fixture() -> Vec<u8> {
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R >>"),
        stream(4, &content()),
    ])
}

/// A filled square under the box, a square outside it, a clip inside the box
/// over a page-sized fill, and a blue triangle that straddles the box's right
/// and top edges.
fn content() -> Vec<u8> {
    b"41.125000 141.125000 m 58.625000 141.125000 l 58.625000 158.250000 l 41.125000 158.250000 l h f\n\
      20.500000 20.500000 m 100.250000 20.500000 l 100.250000 40.750000 l 20.500000 40.750000 l h f\n\
      q 45.250000 140.125000 m 55.875000 140.125000 l 55.875000 160.375000 l h W n \
      0.000000 0.000000 300.000000 300.000000 re f Q\n\
      0 0 1 rg 118.250000 150.000000 m 200.000000 150.000000 l 200.000000 190.000000 l h f\n"
        .to_vec()
}

pub(super) fn save(editor: &mut DocumentEditor, garbage_collect: bool) -> Vec<u8> {
    editor
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            garbage_collect,
            object_streams: false,
            ..SaveOptions::full_rewrite()
        })
        .unwrap()
}

/// The secret is in neither the file's bytes nor any stream it decodes to. The
/// file's objects are read up to its largest id, since the save renumbers the
/// page content.
pub(super) fn assert_absent_everywhere(out: &[u8], secret: &[u8], garbage_collect: bool) {
    let shown = String::from_utf8_lossy(secret);
    assert!(!contains(out, secret), "gc={garbage_collect}: {shown:?} in the bytes");
    let doc = PdfDocument::from_bytes(out.to_vec()).unwrap();
    for id in 1..=largest_id(out) {
        if let Ok(object) = doc.load_object(ObjectRef::new(id, 0)) {
            if let Ok(data) = object.decode_stream_data() {
                assert!(
                    !contains(&data, secret),
                    "gc={garbage_collect}: {shown:?} in the stream of object {id}"
                );
            }
        }
    }
}

/// The largest object number the file writes as `N 0 obj`.
fn largest_id(out: &[u8]) -> u32 {
    let text = String::from_utf8_lossy(out);
    text.split(" 0 obj")
        .filter_map(|part| part.rsplit(|c: char| !c.is_ascii_digit()).next()?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

/// Every coordinate under the box leaves the saved file, every outside one
/// stays, with garbage collection on and off.
#[test]
fn covered_path_and_clip_coordinates_leave_the_saved_file() {
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&fixture(), "redact_path_save");
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, garbage_collect);
        for public in PUBLIC {
            assert!(contains(&out, public.as_bytes()), "gc={garbage_collect}: {public} is gone");
        }
        for secret in COVERED {
            assert_absent_everywhere(&out, secret.as_bytes(), garbage_collect);
        }
    }
}

/// The straddling fill renders as the fill inside the padded box, leaves no
/// blue in the padding, and renders as the source outside the padded box plus
/// three points. The cut re-polygonises the fill's diagonal edge, so a partly
/// covered edge pixel on it may move by a few levels; any other pixel must
/// match exactly.
#[test]
fn straddling_fill_renders_as_the_fill_inside_and_the_source_outside() {
    let src = fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_path_render");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    let after = render(&out);

    let scale = DPI as f32 / 72.0;
    let at = |img: &image::RgbaImage, x: f32, y: f32| {
        img.get_pixel((x * scale) as u32, ((PAGE - y) * scale) as u32).0
    };
    assert!(at(&before, 180.0, 160.0)[2] > 200, "the fixture draws the blue fill");
    assert_fill_inside_source_outside(&before, &after);
}

/// The text clip over an image: the secret's glyphs leave the clip, the public
/// glyphs still reveal the image, and the render is the fill inside the box and
/// the source outside it.
#[test]
fn text_clip_over_an_image_loses_the_secret_and_keeps_the_public_glyphs() {
    let src = text_clip_fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_text_clip");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, true);
    assert_absent_everywhere(&out, b"SECRET", true);
    assert!(contains(&out, b"PUBLIC"), "the public glyphs are gone");
    let after = render(&out);
    let scale = DPI as f32 / 72.0;
    let padded = padded_box();
    let blue = |img: &image::RgbaImage| {
        (0..img.height())
            .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let (px, py) = (x as f32 / scale, PAGE - y as f32 / scale);
                !inside(px, py, padded, 3.0) && {
                    let p = img.get_pixel(x, y).0;
                    p[2] > 200 && p[0] < 64 && p[1] < 160 && p[3] == 255
                }
            })
            .count()
    };
    assert!(blue(&before) > 0, "the fixture reveals the image through the public glyphs");
    assert!(blue(&after) > 0, "the public glyphs stop revealing the image");
    assert_fill_inside_source_outside(&before, &after);
}

/// A Type 3 bitmap glyph under the box leaves the page's content and is not
/// drawn; the same glyph outside the box keeps rendering. The font's CharProcs
/// are not rewritten here, so the bitmap bytes stay in the font.
#[test]
fn type3_bitmap_glyph_under_the_box_is_removed_and_one_outside_keeps_rendering() {
    let src = type3_fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_type3");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, true);
    assert!(!contains(&out, b"(S)"), "the glyph under the box is still shown");
    assert!(contains(&out, b"(O)"), "the glyph outside the box is gone");
    let after = render(&out);

    let scale = DPI as f32 / 72.0;
    let green = |img: &image::RgbaImage, x: f32, y: f32| {
        let p = img.get_pixel((x * scale) as u32, ((PAGE - y) * scale) as u32).0;
        p[1] > 200 && p[0] < 64 && p[2] < 64
    };
    assert!(green(&before, 206.0, 152.0), "the fixture draws the outside glyph");
    assert!(green(&before, 46.0, 152.0), "the fixture draws the glyph under the box");
    assert!(green(&after, 206.0, 152.0), "the outside glyph stopped rendering");
    assert!(!green(&after, 46.0, 152.0), "the glyph under the box still renders");
    assert_fill_inside_source_outside(&before, &after);
}

/// A page showing two Type 3 bitmap glyphs, `S` under the box and `O` outside.
/// Each CharProc paints a 4 by 4 green image in a 1000-unit glyph square.
fn type3_fixture() -> Vec<u8> {
    let bitmap = vec![[0u8, 255, 0]; 16].concat();
    let mut s_proc = b"1000 0 0 0 1000 1000 d1\nq 1000 0 0 1000 0 0 cm\nBI /W 4 /H 4 /BPC 8 /CS /RGB ID ".to_vec();
    s_proc.extend_from_slice(&bitmap);
    s_proc.extend_from_slice(b"\nEI Q\n");
    let o_proc = s_proc.clone();
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F3 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(4, b"BT /F3 12 Tf 1 0 0 1 40 150 Tm (S) Tj 1 0 0 1 200 150 Tm (O) Tj ET\n"),
        obj(
            5,
            "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] \
             /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << /S 6 0 R /O 7 0 R >> \
             /Encoding << /Type /Encoding /Differences [79 /O 83 /S] >> \
             /FirstChar 79 /LastChar 83 /Widths [1000 0 0 0 1000] /Resources << >> >>",
        ),
        stream(6, &s_proc),
        stream(7, &o_proc),
    ])
}

/// Page with a 12-point secret inside the box and 40-point public text outside
/// it, both set with `7 Tr`, and a 4 by 4 blue image painted through the clip.
fn text_clip_fixture() -> Vec<u8> {
    let image = vec![[0u8, 128, 255]; 16].concat();
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(
            4,
            b"q BT 7 Tr /F1 12 Tf 1 0 0 1 40 150 Tm (SECRET) Tj \
              /F1 40 Tf 1 0 0 1 40 20 Tm (PUBLIC) Tj ET \
              q 300 0 0 300 0 0 cm /Im1 Do Q Q\n",
        ),
        obj(
            5,
            &format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
                 /FirstChar 32 /LastChar 126 /Widths [{}] >>",
                vec!["600"; 95].join(" ")
            ),
        ),
        (
            6,
            stream_object(
                6,
                "/Type /XObject /Subtype /Image /Width 4 /Height 4 /BitsPerComponent 8 \
                 /ColorSpace /DeviceRGB",
                &image,
            ),
        ),
    ])
}

pub(super) fn padded_box() -> [f32; 4] {
    let padded = RedactionRegion::from_rect(BOX[0], BOX[1], BOX[2], BOX[3], None)
        .padded_rect(RedactionOptions::default().edge_padding);
    [padded.left(), padded.top(), padded.right(), padded.bottom()]
}

/// The render after redaction is the fill inside the box, has no blue in the
/// padding, and is the source outside the padded box plus three points. The cut
/// re-polygonises a fill's diagonal edge, so a partly covered edge pixel on it
/// may move by a few levels; any other pixel must match exactly.
pub(super) fn assert_fill_inside_source_outside(before: &image::RgbaImage, after: &image::RgbaImage) {
    let scale = DPI as f32 / 72.0;
    let padded = padded_box();
    let mut diffs = Vec::new();
    let mut edge_diffs = 0usize;
    for y in 0..after.height() {
        for x in 0..after.width() {
            let px = x as f32 / scale;
            let py = PAGE - y as f32 / scale;
            let pixel = after.get_pixel(x, y).0;
            if inside(px, py, BOX, -1.0) {
                assert_eq!(pixel, [255, 0, 0, 255], "box pixel ({x}, {y}) is not the fill");
            } else if inside(px, py, padded, -0.5) {
                assert!(
                    !(pixel[2] > 200 && pixel[0] < 64 && pixel[1] < 64),
                    "blue in the padding at ({x}, {y}): {pixel:?}"
                );
            } else if !inside(px, py, padded, 3.0) {
                let (a, b) = (before.get_pixel(x, y).0, pixel);
                if a == b {
                    continue;
                }
                let edge = a[3] < 255 && b[3] < 255;
                let close = (0..4).all(|c| a[c].abs_diff(b[c]) <= 16);
                if edge && close {
                    edge_diffs += 1;
                } else {
                    diffs.push(format!("({x}, {y}) page ({px:.2}, {py:.2}) source {a:?} after {b:?}"));
                }
            }
        }
    }
    assert!(diffs.is_empty(), "{} outside pixels differ: {:?}", diffs.len(), &diffs[..diffs.len().min(12)]);
    assert!(edge_diffs <= 4, "{edge_diffs} edge pixels moved; the cut changed more than its edge");
}

/// Whether the page point (`x`, `y`) lies in `rect` grown by `grow` on every side.
pub(super) fn inside(x: f32, y: f32, rect: [f32; 4], grow: f32) -> bool {
    x >= rect[0] - grow && x <= rect[2] + grow && y >= rect[1] - grow && y <= rect[3] + grow
}
