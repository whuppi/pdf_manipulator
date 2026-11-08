// ── pdf_manipulator patch: hard_light and overlay do not overflow u16 on opaque pixels ──
use tiny_skia::*;

fn paint_over(mode: BlendMode, dst: u8, src: u8) -> PremultipliedColorU8 {
    let mut pixmap = Pixmap::new(1, 1).unwrap();
    pixmap.fill(Color::from_rgba8(dst, dst, dst, 255));

    let mut paint = Paint::default();
    paint.set_color_rgba8(src, src, src, 255);
    paint.blend_mode = mode;
    paint.anti_alias = false;

    let rect = Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap();
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    pixmap.pixel(0, 0).unwrap()
}

// Opaque white over opaque white is the smallest input: `2 * 255 * 255`
// does not fit in u16, and the lowp stage multiplied it in the lane it did
// not select (a debug build panicked).
#[test]
fn overlay_white_on_white() {
    let c = paint_over(BlendMode::Overlay, 255, 255);
    assert_eq!((c.red(), c.green(), c.blue(), c.alpha()), (255, 255, 255, 255));
}

#[test]
fn hard_light_white_on_white() {
    let c = paint_over(BlendMode::HardLight, 255, 255);
    assert_eq!((c.red(), c.green(), c.blue(), c.alpha()), (255, 255, 255, 255));
}

// Both branches of the selection, opaque: overlay is 2sd for d <= 128,
// else 255 - 2(255-s)(255-d)/255.
#[test]
fn overlay_both_branches() {
    assert_eq!(paint_over(BlendMode::Overlay, 100, 200).red(), 157);
    assert_eq!(paint_over(BlendMode::Overlay, 200, 100).red(), 188);
}
// ── end pdf_manipulator patch ──
