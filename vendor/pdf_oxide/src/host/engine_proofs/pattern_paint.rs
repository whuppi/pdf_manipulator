//! A stroke, filled text and stroked text painted with a tiling pattern show
//! the pattern, inside the shape and nowhere else.
//!
//! The pages are rendered at 150 dpi and read pixel by pixel. The shape of a
//! stroke is known from its geometry; the shape of the glyphs is the coverage
//! the same page paints in flat black (the oracle), so the pattern page is
//! compared to coverage the engine itself drew without a pattern.
//!
//! No Dart battery can observe this: the batteries see a rendered page only
//! as an image they may not look into pixel by pixel, and what is proved here
//! is which pixels of a known shape carry which of the pattern's colours.

use super::band_proof::{Fixture, HELVETICA};
use super::support::stream_object;
use crate::document::PdfDocument;
use crate::rendering::{render_page, RenderOptions};

/// Pixels to a point at 150 dpi.
const SCALE: f32 = 150.0 / 72.0;

/// Two colours, red and blue, in 4 pt squares on an 8 pt lattice.
fn checker_coloured() -> Vec<u8> {
    stream_object(
        5,
        "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 8 8] \
         /XStep 8 /YStep 8 /Resources << >>",
        b"1 0 0 rg 0 0 4 4 re f 4 4 4 4 re f 0 0 1 rg 4 0 4 4 re f 0 4 4 4 re f",
    )
}

/// The same squares as a stencil, coloured by the `SCN` that selects it.
fn checker_uncoloured() -> Vec<u8> {
    stream_object(
        6,
        "/Type /Pattern /PatternType 1 /PaintType 2 /TilingType 1 /BBox [0 0 8 8] \
         /XStep 8 /YStep 8 /Resources << >>",
        b"0 0 4 4 re f 4 4 4 4 re f",
    )
}

fn page(name: &'static str, content: &str) -> Fixture {
    let mut fixture = Fixture::new(name, content);
    fixture.resources = format!(
        "{HELVETICA} /Pattern << /P1 5 0 R /P2 6 0 R >> /ColorSpace << /CS1 [/Pattern /DeviceRGB] >>"
    );
    fixture.extra = vec![checker_coloured(), checker_uncoloured()];
    fixture
}

const STROKE: &str = "/Pattern CS /P1 SCN 12 w 20 100 m 180 100 l S\n";
const TEXT: &str = "BT /F1 160 Tf 30 40 Td /Pattern cs /P1 scn (H) Tj ET\n";
const STROKED_TEXT: &str = "BT /F1 160 Tf 30 40 Td 1 Tr 4 w /Pattern CS /P1 SCN (H) Tj ET\n";
const UNCOLOURED_STROKE: &str = "/CS1 CS 0 0.6 0 /P2 SCN 12 w 20 100 m 180 100 l S\n";

/// The pages the band identity proof renders too.
pub(super) fn fixtures() -> Vec<Fixture> {
    vec![
        page("pattern stroke", STROKE),
        page("pattern text", TEXT),
        page("pattern stroked text", STROKED_TEXT),
        page("pattern uncoloured stroke", UNCOLOURED_STROKE),
    ]
}

fn render(fixture: &Fixture) -> image::RgbaImage {
    let doc = PdfDocument::from_bytes(fixture.pdf()).expect("fixture parses");
    let rendered =
        render_page(&doc, 0, &RenderOptions::with_dpi(150).with_transparent_background()).expect("renders");
    image::load_from_memory(&rendered.data).expect("PNG decodes").to_rgba8()
}

/// Whether a painted pixel is `rgb`, or on the seam where it meets the other
/// colour (black is neither). The cell is laid down at fractional
/// offsets, so a pixel on a seam is a little off in alpha and in the other
/// colour; it still has to be the colour and nothing else.
fn is(px: &image::Rgba<u8>, rgb: [u8; 3]) -> bool {
    px[3] >= 128 && (0..3).all(|c| px[c].abs_diff(rgb[c]) <= 130)
}

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const GREEN: [u8; 3] = [0, 153, 0];

/// The page with its pattern painted in flat black instead: the same shape,
/// as the engine covers it.
fn flat_oracle(name: &'static str, content: &str) -> image::RgbaImage {
    let flat = content
        .replace("/Pattern cs /P1 scn", "0 g")
        .replace("/Pattern CS /P1 SCN", "0 G");
    let mut fixture = page(name, &flat);
    fixture.resources = HELVETICA.to_string();
    fixture.extra.clear();
    render(&fixture)
}

/// Whether a pixel lies in the stroke's rectangle, grown by `grow` pixels.
fn near_the_stroke(x: u32, y: u32, grow: f32) -> bool {
    let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
    let (left, right) = (20.0 * SCALE, 180.0 * SCALE);
    let (top, bottom) = ((200.0 - 106.0) * SCALE, (200.0 - 94.0) * SCALE);
    x >= left - grow && x <= right + grow && y >= top - grow && y <= bottom + grow
}

fn assert_stroke_shape(img: &image::RgbaImage, inks: &[[u8; 3]], what: &str) {
    let mut counts = vec![0u32; inks.len()];
    let mut clear = 0u32;
    let mut core = 0u32;
    for (x, y, px) in img.enumerate_pixels() {
        if !near_the_stroke(x, y, 2.0) {
            assert_eq!(px[3], 0, "{what}: pixel ({x}, {y}) outside the stroke is painted: {px:?}");
        }
        if near_the_stroke(x, y, -2.0) {
            core += 1;
            if px[3] == 0 {
                clear += 1;
            } else if px[3] < 128 && inks.len() == 1 {
                clear += 1;
            } else if let Some(i) = inks.iter().position(|ink| is(px, *ink)) {
                counts[i] += 1;
            } else {
                panic!("{what}: pixel ({x}, {y}) inside the stroke is neither pattern colour: {px:?}");
            }
        }
    }
    assert!(core > 0);
    for (ink, count) in inks.iter().zip(&counts) {
        assert!(*count * 10 > core, "{what}: colour {ink:?} covers {count} of {core} pixels in the stroke");
    }
    if inks.len() == 1 {
        assert!(clear * 10 > core, "{what}: the stencil's gaps cover {clear} of {core} pixels");
    }
}

/// A 12 pt stroke painted with a two-colour tiling pattern shows both
/// colours along its length and paints nothing off it; it used to come out
/// in one flat colour. No Dart battery looks at pixels of a known shape.
#[test]
fn a_stroke_painted_with_a_tiling_pattern_shows_the_pattern() {
    let img = render(&page("pattern stroke", STROKE));
    assert_stroke_shape(&img, &[RED, BLUE], "stroke");
}

/// An uncoloured pattern on a stroke takes the colour `SCN` gives with the
/// pattern name: every painted pixel is that colour, and the stencil's gaps
/// stay clear. No Dart battery looks at pixels of a known shape.
#[test]
fn an_uncoloured_pattern_on_a_stroke_takes_the_colour_of_the_scn() {
    let img = render(&page("pattern uncoloured stroke", UNCOLOURED_STROKE));
    assert_stroke_shape(&img, &[GREEN], "uncoloured stroke");
}

/// Large text filled with a tiling pattern shows both colours exactly where
/// the glyph's flat coverage is, and nothing elsewhere. No Dart battery looks
/// at pixels of a known shape.
#[test]
fn text_filled_with_a_tiling_pattern_shows_the_pattern_inside_the_glyph() {
    let img = render(&page("pattern text", TEXT));
    let oracle = flat_oracle("pattern text", TEXT);
    let (mut red, mut blue, mut inside) = (0u32, 0u32, 0u32);
    for (x, y, px) in img.enumerate_pixels() {
        let flat = oracle.get_pixel(x, y)[3];
        if flat == 0 {
            assert_eq!(px[3], 0, "pixel ({x}, {y}) outside the glyph is painted: {px:?}");
        } else if flat == 255 {
            inside += 1;
            if is(px, RED) {
                red += 1;
            } else if is(px, BLUE) {
                blue += 1;
            } else {
                panic!("pixel ({x}, {y}) inside the glyph is neither pattern colour: {px:?}");
            }
        }
    }
    assert!(inside > 0);
    assert!(red * 10 > inside && blue * 10 > inside, "red {red}, blue {blue} of {inside}");
}

/// Stroked text (`Tr 1`) painted with a tiling pattern shows both colours on
/// the outline only: the glyph's deep interior stays clear and nothing lies
/// beyond half the line width past the glyph. No Dart battery looks at pixels
/// of a known shape.
#[test]
fn stroked_text_painted_with_a_tiling_pattern_shows_the_pattern_on_the_outline() {
    let img = render(&page("pattern stroked text", STROKED_TEXT));
    let oracle = flat_oracle("pattern stroked text", &STROKED_TEXT.replace("1 Tr 4 w", "0 Tr"));
    let (w, h) = img.dimensions();
    let opaque = |x: i64, y: i64| {
        x >= 0 && y >= 0 && x < w as i64 && y < h as i64 && oracle.get_pixel(x as u32, y as u32)[3] == 255
    };
    let near_glyph = |x: i64, y: i64, r: i64| {
        (-r..=r).any(|dy| (-r..=r).any(|dx| oracle.get_pixel_checked((x + dx).max(0) as u32, (y + dy).max(0) as u32).is_some_and(|p| p[3] > 0)))
    };
    let half_line = (2.0 * SCALE).ceil() as i64;
    let (mut red, mut blue, mut deep) = (0u32, 0u32, 0u32);
    for (x, y, px) in img.enumerate_pixels() {
        let (xi, yi) = (x as i64, y as i64);
        if px[3] != 0 {
            assert!(
                near_glyph(xi, yi, half_line + 2),
                "pixel ({x}, {y}) lies more than half the line width from the glyph: {px:?}"
            );
        }
        if px[3] >= 128 {
            if is(px, RED) {
                red += 1;
            } else if is(px, BLUE) {
                blue += 1;
            } else {
                panic!("pixel ({x}, {y}) of the outline is neither pattern colour: {px:?}");
            }
        }
        let r = half_line + 4;
        if (-r..=r).step_by(2).all(|dy| (-r..=r).step_by(2).all(|dx| opaque(xi + dx, yi + dy))) {
            deep += 1;
            assert_eq!(px[3], 0, "pixel ({x}, {y}) in the glyph's deep interior is painted: {px:?}");
        }
    }
    assert!(deep > 0, "the glyph has no interior deep enough to tell an outline from a fill");
    assert!(red > 100 && blue > 100, "red {red}, blue {blue}");
}
