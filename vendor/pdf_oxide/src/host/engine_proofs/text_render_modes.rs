//! Text rendering modes 0 to 7 (ISO 32000-1 §9.3.6, Table 106) paint the
//! glyph outline the way the mode says.
//!
//! A 160 pt Helvetica "H" is shown in each mode with a red fill, a blue
//! stroke and `4 w`, then a green rectangle is filled after `ET`. The oracle
//! for each page is the same page with the glyph drawn as a path: the
//! outline is read with `ttf_parser` from the exact face the rasteriser draws
//! an unembedded "Helvetica" with (`TextRasterizer::unembedded_face`, the
//! rasteriser's own resolution over its own font database), scaled by
//! 160/units-per-em and placed at the text origin, then painted with
//! `f`, `S` or `B` and, for the clipping modes, clipped with `W n`. The pages
//! are rendered at 72 dpi on a transparent background and must be the same
//! pixels.
//!
//! No Dart battery can observe this: the batteries see a rendered page only
//! as an image they may not look into pixel by pixel, and the oracle needs
//! the glyph outline of the face the engine picked.

use super::band_proof::{Fixture, HELVETICA};
use crate::document::PdfDocument;
use crate::rendering::{render_page, RenderOptions, TextRasterizer};
use std::fmt::Write;

const SIZE: f32 = 160.0;
const ORIGIN: (f32, f32) = (30.0, 40.0);
const COLOURS: &str = "1 0 0 rg 0 0 1 RG";
const RECT: &str = "0 1 0 rg 0 0 200 200 re f";

struct Ops {
    text: String,
    scale: f32,
    at: (f32, f32),
}

impl Ops {
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (ORIGIN.0 + x * self.scale, ORIGIN.1 + y * self.scale)
    }
}

impl ttf_parser::OutlineBuilder for Ops {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        let _ = write!(self.text, "{x:.5} {y:.5} m ");
        self.at = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        let _ = write!(self.text, "{x:.5} {y:.5} l ");
        self.at = (x, y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (cx, cy) = self.point(cx, cy);
        let (x, y) = self.point(x, y);
        let (px, py) = self.at;
        let (c1x, c1y) = (px + 2.0 / 3.0 * (cx - px), py + 2.0 / 3.0 * (cy - py));
        let (c2x, c2y) = (x + 2.0 / 3.0 * (cx - x), y + 2.0 / 3.0 * (cy - y));
        let _ = write!(self.text, "{c1x:.5} {c1y:.5} {c2x:.5} {c2y:.5} {x:.5} {y:.5} c ");
        self.at = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        let _ = write!(self.text, "{x1:.5} {y1:.5} {x2:.5} {y2:.5} {x:.5} {y:.5} c ");
        self.at = (x, y);
    }
    fn close(&mut self) {
        self.text.push_str("h ");
    }
}

/// The "H" outline as path operators in page space.
fn outline() -> String {
    let (data, index) = TextRasterizer::unembedded_face("Helvetica")
        .expect("the rasteriser has a face for an unembedded Helvetica; without one it draws nothing");
    let face = ttf_parser::Face::parse(&data, index).expect("face parses");
    let glyph = face.glyph_index('H').expect("face has an H");
    let mut ops = Ops { text: String::new(), scale: SIZE / face.units_per_em() as f32, at: (0.0, 0.0) };
    face.outline_glyph(glyph, &mut ops).expect("H has an outline");
    ops.text
}

/// The eight pages the band identity proof renders too.
pub(super) fn fixtures() -> Vec<Fixture> {
    const NAMES: [&str; 8] = [
        "text mode 0",
        "text mode 1",
        "text mode 2",
        "text mode 3",
        "text mode 4",
        "text mode 5",
        "text mode 6",
        "text mode 7",
    ];
    NAMES
        .iter()
        .enumerate()
        .map(|(mode, name)| {
            // A mark off the glyph, so the invisible mode's page is not blank.
            let content = format!("0 0 0 rg 5 5 10 10 re f {}", text_page(mode as u8, ""));
            let mut fixture = Fixture::new(name, &content);
            fixture.resources = HELVETICA.to_string();
            fixture
        })
        .collect()
}

fn render(content: &str) -> image::RgbaImage {
    let mut fixture = Fixture::new("text render modes", content);
    fixture.resources = HELVETICA.to_string();
    let doc = PdfDocument::from_bytes(fixture.pdf()).expect("fixture parses");
    let rendered =
        render_page(&doc, 0, &RenderOptions::with_dpi(72).with_transparent_background()).expect("renders");
    image::load_from_memory(&rendered.data).expect("PNG decodes").to_rgba8()
}

/// The page that shows the "H" in `mode`, under `style` (line operators).
fn text_page(mode: u8, style: &str) -> String {
    let clip = if mode >= 4 { RECT } else { "" };
    format!(
        "{COLOURS} 4 w {style} BT /F1 {SIZE} Tf {} {} Td {mode} Tr (H) Tj ET {clip}\n",
        ORIGIN.0, ORIGIN.1
    )
}

/// The oracle page for `mode`: the outline as a path, painted by `paint`
/// (`f`, `S`, `B` or `n`), and for the clipping modes the rectangle filled
/// under the outline as a clip.
fn path_page(mode: u8, style: &str, outline: &str) -> String {
    let paint = match mode % 4 {
        0 => "f",
        1 => "S",
        2 => "B",
        _ => "n",
    };
    let clip = if mode >= 4 { format!("q {outline} W n {RECT} Q") } else { String::new() };
    format!("{COLOURS} 4 w {style} {outline} {paint} {clip}\n")
}

/// The glyph the rasteriser places and the path the oracle places differ by
/// a sub-pixel at a few edge pixels (12 of 40000 in mode 0, by at most two
/// coverage steps of 16), so a pixel may differ by `TOLERANCE` in a channel
/// and at most `FEW` pixels may differ at all. A stroke painted as a fill, or
/// a missing one, differs in thousands of pixels by the full channel range.
const TOLERANCE: u8 = 48;
const FEW: u32 = 100;

fn assert_same(got: &image::RgbaImage, want: &image::RgbaImage, what: &str) {
    assert_eq!(got.dimensions(), want.dimensions());
    let mut differing = 0u32;
    let mut worst = 0u8;
    let mut first = None;
    for (x, y, px) in got.enumerate_pixels() {
        let o = want.get_pixel(x, y);
        // A colour channel only means something where the pixel is mostly opaque.
        let colours = if px[3] >= 128 && o[3] >= 128 { 0..4 } else { 3..4 };
        let step = colours.map(|c| px[c].abs_diff(o[c])).max().unwrap_or(0);
        if step > 0 {
            differing += 1;
            worst = worst.max(step);
            first.get_or_insert((x, y, *px, *o));
        }
    }
    assert!(
        differing <= FEW && worst <= TOLERANCE,
        "{what}: {differing} pixels differ from the path oracle, by up to {worst}, first (x, y, text, path): {first:?}"
    );
}

fn count(img: &image::RgbaImage, f: impl Fn(&image::Rgba<u8>) -> bool) -> usize {
    img.pixels().filter(|p| f(p)).count()
}

const RED: fn(&image::Rgba<u8>) -> bool = |p| p[3] == 255 && p[0] == 255 && p[1] == 0 && p[2] == 0;
const BLUE: fn(&image::Rgba<u8>) -> bool = |p| p[3] == 255 && p[0] == 0 && p[1] == 0 && p[2] == 255;
const GREEN: fn(&image::Rgba<u8>) -> bool = |p| p[3] == 255 && p[0] == 0 && p[1] == 255 && p[2] == 0;

/// Whether the pixel and its neighbours within `r` pixels are all opaque in
/// the fill oracle: the glyph's deep interior, clear of the stroke.
fn deep(fill: &image::RgbaImage, x: u32, y: u32, r: i64) -> bool {
    (-r..=r).all(|dy| {
        (-r..=r).all(|dx| {
            fill.get_pixel_checked((x as i64 + dx).max(0) as u32, (y as i64 + dy).max(0) as u32)
                .is_some_and(|p| p[3] == 255)
        })
    })
}

/// The shape each of the four modes paints, read off the pixels: mode 0 only
/// red, mode 1 blue on the outline and nothing inside, mode 2 red inside and
/// blue over it on the outline, mode 3 nothing.
fn assert_shape(mode: u8, img: &image::RgbaImage, fill: &image::RgbaImage) {
    let deep_px: Vec<(u32, u32)> = fill
        .enumerate_pixels()
        .filter(|(x, y, _)| deep(fill, *x, *y, 4))
        .map(|(x, y, _)| (x, y))
        .collect();
    assert!(!deep_px.is_empty(), "the glyph has no interior deep enough to tell an outline from a fill");
    let in_deep = |f: fn(&image::Rgba<u8>) -> bool| deep_px.iter().filter(|(x, y)| f(img.get_pixel(*x, *y))).count();
    match mode % 4 {
        0 => {
            assert_eq!(in_deep(RED), deep_px.len(), "mode {mode}: the interior is not all the fill colour");
            assert_eq!(count(img, BLUE), 0, "mode {mode}: the stroke colour appears");
        }
        1 => {
            assert!(count(img, BLUE) > 1000, "mode {mode}: no stroke colour on the outline");
            assert_eq!(count(img, RED), 0, "mode {mode}: the fill colour appears");
            assert!(deep_px.iter().all(|(x, y)| img.get_pixel(*x, *y)[3] == 0), "mode {mode}: the interior is painted");
        }
        2 => {
            assert_eq!(in_deep(RED), deep_px.len(), "mode {mode}: the interior is not all the fill colour");
            assert!(count(img, BLUE) > 1000, "mode {mode}: no stroke colour on the outline");
        }
        _ => assert_eq!(count(img, |p| p[3] != 0), 0, "mode {mode}: invisible text paints"),
    }
}

fn prove_modes(modes: std::ops::Range<u8>, style: &str) {
    let outline = outline();
    let fill_oracle = render(&path_page(0, "", &outline));
    for mode in modes {
        let img = render(&text_page(mode, style));
        let want = render(&path_page(mode, style, &outline));
        assert_same(&img, &want, &format!("mode {mode}"));
        if style.is_empty() && mode < 4 {
            assert_shape(mode, &img, &fill_oracle);
        }
    }
}

/// The font the oracle reads is the font the rasteriser painted: mode 0
/// text is the outline filled as a path, pixel for pixel. No Dart battery
/// can read the glyph outline of the face the engine picked.
#[test]
fn the_oracle_outline_is_the_glyph_the_rasteriser_fills() {
    let outline = outline();
    assert_same(&render(&text_page(0, "")), &render(&path_page(0, "", &outline)), "mode 0");
}

/// Modes 0 to 3 paint fill, stroke, fill then stroke and nothing: each page
/// is the outline drawn as a path with `f`, `S`, `B` and `n`, and the pixels
/// show the fill colour only, the stroke colour on the outline with the
/// interior clear, the stroke over the fill, and nothing. Modes 1 and 2
/// used to come out solid in the fill colour. No Dart battery can read the
/// glyph outline of the face the engine picked.
#[test]
fn modes_0_to_3_paint_the_outline_the_mode_says() {
    prove_modes(0..4, "");
}

/// Modes 4 to 7 paint as 0 to 3 and add the glyph to the clip, so a
/// rectangle filled after `ET` shows only inside the glyph: the pages equal
/// the outline painted as a path and the rectangle clipped by it. No Dart
/// battery can read the glyph outline of the face the engine picked.
#[test]
fn modes_4_to_7_paint_as_0_to_3_and_clip_to_the_glyph() {
    prove_modes(4..8, "");
    let clipped = render(&text_page(7, ""));
    assert!(count(&clipped, GREEN) > 1000, "the rectangle does not show inside the glyph");
    assert_eq!(count(&clipped, |p| p[3] == 255 && !GREEN(p)), 0, "mode 7 paints something besides the clipped rectangle");
}

/// The stroke takes the line style of the state, caps and joins as well as
/// the width: a wide round-join stroke equals the path stroked alike. No
/// Dart battery can read the glyph outline of the face the engine picked.
#[test]
fn the_glyph_stroke_takes_the_line_cap_and_join() {
    let outline = outline();
    let style = "12 w 1 j 1 J";
    for mode in [1u8, 2] {
        let img = render(&text_page(mode, style));
        let want = render(&path_page(mode, style, &outline));
        assert_same(&img, &want, &format!("mode {mode} with round joins"));
    }
}
