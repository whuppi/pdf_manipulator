// ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
//! A pixmap with an origin shows a part of the device plane. Whatever is drawn
//! into a window must be, pixel for pixel, what the same drawing leaves in the
//! same rows of a canvas that holds the whole plane. These tests draw one
//! picture onto a whole canvas and onto windows of several heights and
//! positions, and compare the bytes.

use tiny_skia::*;

const W: u32 = 160;
const H: u32 = 240;

/// A small deterministic generator, so that a failure names the same case on
/// every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }

    fn point(&mut self) -> (f32, f32) {
        // A little beyond the plane on every side.
        (self.next() * (W as f32 + 40.0) - 20.0, self.next() * (H as f32 + 40.0) - 20.0)
    }
}

type Scene = fn(&mut Pixmap, u64);

fn paint(anti_alias: bool, rgba: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
    p.anti_alias = anti_alias;
    p
}

fn scene_hairlines(pixmap: &mut Pixmap, seed: u64) {
    let mut rng = Lcg(seed);
    for _ in 0..40 {
        let (a, b) = (rng.point(), rng.point());
        let mut pb = PathBuilder::new();
        pb.move_to(a.0, a.1);
        pb.line_to(b.0, b.1);
        let path = pb.finish().unwrap();
        let mut stroke = Stroke::default();
        stroke.width = 0.3 + rng.next() * 0.6;
        pixmap.stroke_path(&path, &paint(true, [20, 90, 160, 220]), &stroke, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    for _ in 0..6 {
        let (c, e) = (rng.point(), rng.point());
        let (s,) = (rng.point(),);
        pb.move_to(s.0, s.1);
        pb.quad_to(c.0, c.1, e.0, e.1);
        let (c1, c2, e2) = (rng.point(), rng.point(), rng.point());
        pb.move_to(e.0, e.1);
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e2.0, e2.1);
    }
    let path = pb.finish().unwrap();
    let mut stroke = Stroke::default();
    stroke.width = 0.4;
    pixmap.stroke_path(&path, &paint(true, [160, 40, 40, 255]), &stroke, Transform::identity(), None);
}

fn scene_fills(pixmap: &mut Pixmap, seed: u64) {
    let mut rng = Lcg(seed);
    for i in 0..25 {
        let mut pb = PathBuilder::new();
        let s = rng.point();
        pb.move_to(s.0, s.1);
        match i % 3 {
            0 => {
                let (a, b) = (rng.point(), rng.point());
                pb.line_to(a.0, a.1);
                pb.line_to(b.0, b.1);
            }
            1 => {
                let (c, e) = (rng.point(), rng.point());
                pb.quad_to(c.0, c.1, e.0, e.1);
            }
            _ => {
                let (c1, c2, e) = (rng.point(), rng.point(), rng.point());
                pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e.0, e.1);
            }
        }
        pb.close();
        let path = pb.finish().unwrap();
        let rule = if i % 2 == 0 { FillRule::Winding } else { FillRule::EvenOdd };
        let aa = i % 4 != 3;
        pixmap.fill_path(&path, &paint(aa, [200 - i as u8 * 6, 60 + i as u8 * 5, 90, 200]), rule, Transform::identity(), None);
    }
    // A transform with fractional parts, as a page transform has.
    let rect = PathBuilder::from_rect(Rect::from_xywh(3.3, 4.7, 70.1, 60.9).unwrap());
    let ts = Transform::from_row(1.37, 0.21, -0.18, 0.93, 11.31, 17.49);
    pixmap.fill_path(&rect, &paint(true, [10, 150, 60, 180]), FillRule::Winding, ts, None);
    let mut stroke = Stroke::default();
    stroke.width = 2.7;
    stroke.line_cap = LineCap::Round;
    stroke.line_join = LineJoin::Round;
    pixmap.stroke_path(&rect, &paint(true, [90, 10, 150, 230]), &stroke, ts, None);
    stroke.dash = StrokeDash::new(vec![7.0, 3.5], 1.0);
    pixmap.stroke_path(&rect, &paint(true, [0, 0, 0, 255]), &stroke, Transform::from_scale(1.9, 2.3), None);
}

fn scene_rects(pixmap: &mut Pixmap, seed: u64) {
    let mut rng = Lcg(seed);
    for _ in 0..20 {
        let (a, b) = (rng.point(), rng.point());
        if let Some(r) = Rect::from_ltrb(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0) + 0.5, a.1.max(b.1) + 0.5) {
            pixmap.fill_rect(r, &paint(true, [30, 30, 200, 120]), Transform::identity(), None);
            pixmap.fill_rect(r, &paint(false, [200, 30, 30, 90]), Transform::identity(), None);
        }
    }
}

fn scene_gradients(pixmap: &mut Pixmap, seed: u64) {
    let mut rng = Lcg(seed);
    let stops = vec![
        GradientStop::new(0.0, Color::from_rgba8(255, 0, 0, 255)),
        GradientStop::new(0.5, Color::from_rgba8(0, 200, 0, 255)),
        GradientStop::new(1.0, Color::from_rgba8(0, 0, 255, 255)),
    ];
    for i in 0..4 {
        let (a, b) = (rng.point(), rng.point());
        let shader = if i % 2 == 0 {
            LinearGradient::new(Point::from_xy(a.0, a.1), Point::from_xy(b.0, b.1), stops.clone(), SpreadMode::Pad, Transform::identity())
        } else {
            RadialGradient::new(Point::from_xy(a.0, a.1), 0.0, Point::from_xy(a.0 + 3.0, a.1 + 2.0), 20.0 + rng.next() * 80.0, stops.clone(), SpreadMode::Reflect, Transform::identity())
        };
        let mut p = Paint::default();
        p.shader = shader.unwrap();
        p.anti_alias = true;
        let rect = Rect::from_xywh(rng.next() * 60.0, rng.next() * 60.0, 120.0, 150.0).unwrap();
        pixmap.fill_path(&PathBuilder::from_rect(rect), &p, FillRule::Winding, Transform::from_translate(0.37, 0.61), None);
    }
}

fn scene_pixmaps(pixmap: &mut Pixmap, seed: u64) {
    let mut rng = Lcg(seed);
    let mut src = Pixmap::new(9, 7).unwrap();
    for (i, p) in src.pixels_mut().iter_mut().enumerate() {
        *p = PremultipliedColorU8::from_rgba((i * 29 % 256) as u8, (i * 53 % 256) as u8, (i * 7 % 256) as u8, 255).unwrap();
    }
    for _ in 0..4 {
        let a = rng.point();
        let ts = Transform::from_row(8.1 + rng.next() * 6.0, rng.next() * 5.0, -rng.next() * 4.0, 9.7 + rng.next() * 5.0, a.0, a.1);
        let mut paint = PixmapPaint::default();
        paint.quality = FilterQuality::Bilinear;
        pixmap.draw_pixmap(0, 0, src.as_ref(), &paint, ts, None);
    }
}

fn scene_masks(pixmap: &mut Pixmap, seed: u64) {
    let mut rng = Lcg(seed);
    let mut mask = Mask::new_like(pixmap).unwrap();
    for _ in 0..3 {
        let mut pb = PathBuilder::new();
        let s = rng.point();
        pb.move_to(s.0, s.1);
        let (c1, c2, e) = (rng.point(), rng.point(), rng.point());
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e.0, e.1);
        pb.close();
        mask.fill_path(&pb.finish().unwrap(), FillRule::Winding, true, Transform::from_scale(1.0, 1.0));
    }
    let rect = PathBuilder::from_rect(Rect::from_xywh(0.0, 0.0, W as f32, H as f32).unwrap());
    pixmap.fill_path(&rect, &paint(true, [255, 120, 0, 255]), FillRule::Winding, Transform::identity(), Some(&mask));
}

fn whole(scene: Scene, seed: u64) -> Pixmap {
    let mut canvas = Pixmap::new(W, H).unwrap();
    scene(&mut canvas, seed);
    canvas
}

fn window(scene: Scene, seed: u64, top: u32, rows: u32) -> Pixmap {
    let mut window = Pixmap::new_at(W, rows, (0, top as i32)).unwrap();
    scene(&mut window, seed);
    window
}

/// Draws `scene` on the whole canvas and in windows of several heights, and
/// fails with the first differing pixel.
fn assert_windows_equal_canvas(name: &str, scene: Scene) {
    for seed in 1..=6u64 {
        let canvas = whole(scene, seed);
        for rows in [1u32, 2, 3, 17, 64] {
            let mut top = 0;
            while top < H {
                let rows = rows.min(H - top);
                let part = window(scene, seed, top, rows);
                let cut = &canvas.data()[(top * W * 4) as usize..((top + rows) * W * 4) as usize];
                if part.data() != cut {
                    let at = part.data().iter().zip(cut).position(|(a, b)| a != b).unwrap();
                    let (x, y) = ((at / 4) as u32 % W, top + (at / 4) as u32 / W);
                    panic!(
                        "{name}, seed {seed}, window of {rows} rows at {top}: first differing pixel ({x}, {y}): \
                         window {:?} canvas {:?}",
                        &part.data()[at / 4 * 4..at / 4 * 4 + 4],
                        &cut[at / 4 * 4..at / 4 * 4 + 4],
                    );
                }
                top += rows;
            }
        }
    }
}

#[test]
fn window_hairlines_equal_the_canvas() {
    assert_windows_equal_canvas("hairlines", scene_hairlines);
}

#[test]
fn window_fills_equal_the_canvas() {
    assert_windows_equal_canvas("fills", scene_fills);
}

#[test]
fn window_rects_equal_the_canvas() {
    assert_windows_equal_canvas("rects", scene_rects);
}

#[test]
fn window_gradients_equal_the_canvas() {
    assert_windows_equal_canvas("gradients", scene_gradients);
}

#[test]
fn window_pixmaps_equal_the_canvas() {
    assert_windows_equal_canvas("pixmaps", scene_pixmaps);
}

#[test]
fn window_masks_equal_the_canvas() {
    assert_windows_equal_canvas("masks", scene_masks);
}

/// A path that starts above the window is not chopped there: the rows the
/// window shows are the canvas's rows, and a path wholly above or below draws
/// nothing.
#[test]
fn an_edge_enters_a_window_from_above_without_a_chop() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.3, -300.7);
    pb.line_to(150.9, 400.1);
    pb.line_to(20.2, 380.3);
    pb.close();
    let path = pb.finish().unwrap();
    let p = paint(true, [10, 10, 10, 255]);

    let mut canvas = Pixmap::new(W, 400).unwrap();
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    for top in [0u32, 120, 399] {
        let mut part = Pixmap::new_at(W, 1, (0, top as i32)).unwrap();
        part.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        assert_eq!(part.data(), &canvas.data()[(top * W * 4) as usize..((top + 1) * W * 4) as usize], "row {top}");
    }

    let mut far = Pixmap::new_at(W, 5, (0, 1000)).unwrap();
    far.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    assert!(far.data().iter().all(|b| *b == 0));
}

/// A raster wider than a tile is drawn in tiles, and a tile is a window: the
/// bands of a wide raster are the same rows of the whole wide raster, across
/// the tile seam, with nothing moved by a float. (A window narrower than the
/// raster is not promised the same: the sides of a path are still chopped at
/// the window's columns.)
#[test]
fn bands_of_a_raster_wider_than_a_tile_equal_the_whole_raster() {
    const WIDE: u32 = 9000;
    let mut pb = PathBuilder::new();
    pb.move_to(8100.37, 1.5);
    pb.cubic_to(8300.2, 30.0, 8000.9, 3.0, 8260.77, 9.6);
    pb.close();
    pb.move_to(10.5, 0.7);
    pb.line_to(8990.3, 11.2);
    pb.line_to(4000.1, 5.9);
    pb.close();
    let path = pb.finish().unwrap();
    let p = paint(true, [0, 80, 160, 255]);
    let mut stroke = Stroke::default();
    stroke.width = 0.4;

    let draw = |pixmap: &mut Pixmap| {
        pixmap.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        pixmap.stroke_path(&path, &paint(true, [200, 0, 0, 255]), &stroke, Transform::identity(), None);
    };
    let mut wide = Pixmap::new(WIDE, 12).unwrap();
    draw(&mut wide);

    for top in 0..12u32 {
        let mut band = Pixmap::new_at(WIDE, 1, (0, top as i32)).unwrap();
        draw(&mut band);
        assert_eq!(band.data(), &wide.data()[(top * WIDE * 4) as usize..((top + 1) * WIDE * 4) as usize], "row {top}");
    }
}

/// A raster taller than a tile: its bands are the same rows of it.
#[test]
fn bands_of_a_raster_taller_than_a_tile_equal_the_whole_raster() {
    const TALL: u32 = 8400;
    let mut pb = PathBuilder::new();
    pb.move_to(3.3, 8100.4);
    pb.cubic_to(70.2, 8300.9, 20.1, 8000.5, 90.7, 8290.3);
    pb.line_to(5.5, 8350.6);
    pb.close();
    let path = pb.finish().unwrap();
    let p = paint(true, [0, 80, 160, 255]);

    let mut tall = Pixmap::new(100, TALL).unwrap();
    tall.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    for top in [8000u32, 8185, 8190, 8191, 8192, 8300] {
        let mut band = Pixmap::new_at(100, 3, (0, top as i32)).unwrap();
        band.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        assert_eq!(band.data(), &tall.data()[(top * 100 * 4) as usize..((top + 3) * 100 * 4) as usize], "rows {top}..");
    }
}

/// A window of a source image, drawn where the whole image is drawn, gives the
/// pixels the whole image gives wherever the window holds the samples (and the
/// taps beside them) that a pixel is made of. The sample positions are the
/// floats the whole image's are, whatever the window's origin, so the bytes
/// are equal under a transform that makes them irrational.
#[test]
fn a_window_of_a_source_image_draws_the_pixels_of_the_whole_image() {
    let (sw, sh) = (64u32, 48u32);
    let mut src = Pixmap::new(sw, sh).unwrap();
    let mut rng = Lcg(7);
    for p in src.pixels_mut().iter_mut() {
        let v = (rng.next() * 255.0) as u8;
        *p = PremultipliedColorU8::from_rgba(v, v / 2, 255 - v, 255).unwrap();
    }
    let ts = Transform::from_row(2.37, 1.13, -0.91, 2.11, 31.3, 7.7);
    let inverse = ts.invert().unwrap();
    for quality in [FilterQuality::Nearest, FilterQuality::Bilinear, FilterQuality::Bicubic] {
        let mut paint = PixmapPaint::default();
        paint.quality = quality;
        let mut whole = Pixmap::new(200, 200).unwrap();
        whole.draw_pixmap(0, 0, src.as_ref(), &paint, ts, None);

        for (x0, x1, y0, y1) in [(0u32, 64u32, 9u32, 41u32), (6, 57, 0, 48), (13, 50, 13, 37)] {
            let mut window = Pixmap::new_at(x1 - x0, y1 - y0, (x0 as i32, y0 as i32)).unwrap();
            for y in 0..y1 - y0 {
                for x in 0..x1 - x0 {
                    let at = src.pixel(x0 + x, y0 + y).unwrap();
                    window.pixels_mut()[(y * (x1 - x0) + x) as usize] = at;
                }
            }
            let mut part = Pixmap::new(200, 200).unwrap();
            let area = Rect::from_xywh(0.0, 0.0, sw as f32, sh as f32).unwrap();
            part.draw_pixmap_in(area, window.as_ref(), &paint, ts, None);

            // A pixel is made of the window's samples when its position and
            // the taps three samples round it lie inside the window.
            let mut compared = 0;
            for y in 0..200u32 {
                for x in 0..200u32 {
                    let mut p = [Point::from_xy(x as f32 + 0.5, y as f32 + 0.5)];
                    inverse.map_points(&mut p);
                    let inside = |v: f32, lo: u32, hi: u32, size: u32| {
                        (v - 3.0 >= lo as f32 || lo == 0) && (v + 3.0 <= hi as f32 || hi == size)
                    };
                    if p[0].x >= 0.0
                        && p[0].y >= 0.0
                        && p[0].x < sw as f32
                        && p[0].y < sh as f32
                        && inside(p[0].x, x0, x1, sw)
                        && inside(p[0].y, y0, y1, sh)
                    {
                        compared += 1;
                        assert_eq!(
                            whole.pixel(x, y),
                            part.pixel(x, y),
                            "{quality:?} window {x0}..{x1} x {y0}..{y1} at ({x}, {y})"
                        );
                    }
                }
            }
            assert!(compared > 1000, "{quality:?}: only {compared} pixels compared");
        }
    }
}
// ── end pdf_manipulator patch ──
