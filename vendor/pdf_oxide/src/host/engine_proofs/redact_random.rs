//! Random regions over random rectangles, lines, curves and clips, saved and
//! rendered: whatever the seed draws, a pixel inside a region is the fill and
//! a pixel well outside every region is the source's. The seed is printed with
//! every failure, so a failing page is rebuilt from it alone. A small count
//! runs with the suite; the large one is the nightly test.

use super::redact_save::{obj, render, stream, write_pdf, DPI, PAGE};
use super::support::open_editor_from_bytes;
use crate::editor::SaveOptions;
use crate::redaction::{RedactionOptions, RedactionRegion};

const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// A xorshift generator: the same seed draws the same page everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A multiple of a quarter point in `lo..=hi`, exact in binary and in the
    /// writer's six decimals.
    fn grid(&mut self, lo: i64, hi: i64) -> f32 {
        let steps = (hi - lo) * 4 + 1;
        (lo * 4 + (self.next() % steps as u64) as i64) as f32 / 4.0
    }

    fn pick(&mut self, count: u64) -> u64 {
        self.next() % count
    }
}

type Box4 = [f32; 4];

fn overlaps(a: Box4, b: Box4, grow: f32) -> bool {
    a[0] < b[2] + grow && a[2] > b[0] - grow && a[1] < b[3] + grow && a[3] > b[1] - grow
}

fn contains(outer: Box4, inner: Box4) -> bool {
    outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3]
}

/// One page: its content, the boxes to redact.
struct Page {
    content: String,
    boxes: Vec<Box4>,
}

fn pad(b: Box4) -> Box4 {
    let p = RedactionRegion::from_rect(b[0], b[1], b[2], b[3], None)
        .padded_rect(RedactionOptions::default().edge_padding);
    [p.left(), p.top(), p.right(), p.bottom()]
}

/// Draws a page from `seed`. Rectangles and lines are axis-aligned wherever
/// they fall, so the cut they take is exact. A curve is kept only where it
/// stays clear of every region or lies under one, since a curve cut at a box
/// edge becomes a polyline.
fn draw(seed: u64) -> Page {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut boxes: Vec<Box4> = Vec::new();
    for _ in 0..=rng.pick(3) {
        let (x, y) = (rng.grid(10, 200), rng.grid(10, 200));
        boxes.push([x, y, x + rng.grid(30, 90), y + rng.grid(20, 70)]);
    }
    let padded: Vec<Box4> = boxes.iter().map(|b| pad(*b)).collect();
    let clear = |bbox: Box4| padded.iter().all(|p| !overlaps(*p, bbox, 8.0));
    let under = |bbox: Box4| boxes.iter().any(|b| contains(*b, bbox));

    let mut content = String::new();
    for _ in 0..14 {
        match rng.pick(4) {
            0 => {
                let (x, y) = (rng.grid(0, 260), rng.grid(0, 260));
                let (w, h) = (rng.grid(4, 80), rng.grid(4, 80));
                content += &format!("0 0 1 rg {x:.6} {y:.6} {w:.6} {h:.6} re f\n");
            },
            1 => {
                let width = [1.0, 2.0, 4.0][rng.pick(3) as usize];
                let (x, y, len) = (rng.grid(0, 250), rng.grid(0, 250), rng.grid(20, 120));
                let (x1, y1) = if rng.pick(2) == 0 { (x + len, y) } else { (x, y + len) };
                content += &format!("0 0 1 RG {width} w {x:.6} {y:.6} m {x1:.6} {y1:.6} l S\n");
            },
            2 => {
                let (x, y) = (rng.grid(0, 250), rng.grid(0, 250));
                let mut points = [(x, y); 4];
                for p in points.iter_mut().skip(1) {
                    *p = (x + rng.grid(0, 40), y + rng.grid(0, 40));
                }
                let bbox = [
                    points.iter().map(|p| p.0).fold(f32::MAX, f32::min),
                    points.iter().map(|p| p.1).fold(f32::MAX, f32::min),
                    points.iter().map(|p| p.0).fold(f32::MIN, f32::max),
                    points.iter().map(|p| p.1).fold(f32::MIN, f32::max),
                ];
                if clear(bbox) || under(bbox) {
                    content += &format!(
                        "0 0 1 rg {:.6} {:.6} m {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} c f\n",
                        points[0].0, points[0].1, points[1].0, points[1].1, points[2].0,
                        points[2].1, points[3].0, points[3].1
                    );
                }
            },
            _ => {
                let (x, y) = (rng.grid(0, 220), rng.grid(0, 220));
                let (w, h) = (rng.grid(10, 80), rng.grid(10, 80));
                content += &format!(
                    "q {x:.6} {y:.6} {w:.6} {h:.6} re W n 0 0 1 rg {:.6} {:.6} {:.6} {:.6} re f Q\n",
                    x - 5.0,
                    y - 5.0,
                    w + 10.0,
                    h + 10.0
                );
            },
        }
    }
    Page { content, boxes }
}

fn fixture(content: &str) -> Vec<u8> {
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R >>"),
        stream(4, content.as_bytes()),
    ])
}

fn within(x: f32, y: f32, b: Box4, grow: f32) -> bool {
    x >= b[0] - grow && x <= b[2] + grow && y >= b[1] - grow && y <= b[3] + grow
}

/// Draws and redacts the page of each seed, then reads the pixels back.
fn check(seeds: std::ops::Range<u64>) {
    let scale = DPI as f32 / 72.0;
    for seed in seeds {
        let page = draw(seed);
        let src = fixture(&page.content);
        let before = render(&src);
        let mut editor = open_editor_from_bytes(&src, "redact_random");
        for b in &page.boxes {
            editor.add_redaction(0, *b, Some(RED)).unwrap();
        }
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = editor
            .save_to_bytes_with_options(SaveOptions {
                garbage_collect: seed % 2 == 0,
                ..SaveOptions::full_rewrite()
            })
            .unwrap();
        let after = render(&out);
        let padded: Vec<Box4> = page.boxes.iter().map(|b| pad(*b)).collect();
        for y in 0..after.height() {
            for x in 0..after.width() {
                let (px, py) = (x as f32 / scale, PAGE - y as f32 / scale);
                let pixel = after.get_pixel(x, y).0;
                let context = || format!("seed {seed}, pixel ({x}, {y}), boxes {:?}\n{}", page.boxes, page.content);
                if page.boxes.iter().any(|b| within(px, py, *b, -1.0)) {
                    assert_eq!(pixel, [255, 0, 0, 255], "not the fill: {}", context());
                } else if padded.iter().any(|p| within(px, py, *p, -0.5)) {
                    let ink = pixel[2] > 200 && pixel[0] < 64 && pixel[1] < 64;
                    assert!(!ink, "ink in the padding: {pixel:?} {}", context());
                } else if padded.iter().all(|p| !within(px, py, *p, 3.0)) {
                    assert_eq!(
                        before.get_pixel(x, y).0,
                        pixel,
                        "outside pixel differs from the source: {}",
                        context()
                    );
                }
            }
        }
    }
}

/// A few seeds with the suite.
#[test]
fn random_regions_render_as_the_fill_inside_and_the_source_outside() {
    check(0..8);
}

/// Many seeds, for the nightly run: `cargo nextest run --run-ignored only`.
#[test]
#[ignore = "nightly: hundreds of renders"]
fn nightly_random_regions_render_as_the_fill_inside_and_the_source_outside() {
    check(8..408);
}
