//! A region render cuts the same pixels from a page as the full render has
//! there, whatever the page's `/CropBox` and `/Rotate`.
//!
//! `render_page_region` takes its rectangle in PDF user-space points, origin
//! at the bottom-left of the page's coordinate system (the `/MediaBox`
//! origin is user-space zero), the coordinates content is drawn in. The
//! raster is the page's crop box turned by `/Rotate`; the oracle here
//! converts the rectangle to raster pixels by hand and cuts it from the
//! full-page render.
//!
//! No Dart battery can observe this: the Dart API hands back region PNGs
//! only, with no second render of the same page to compare pixel for pixel
//! and no way to place marks in the page at known points.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::rendering::{render_page, render_page_region, RenderOptions};

/// Four marks at known points: one in the cropped-away margin, three inside
/// the crop box, each a different colour so a misplaced cut shows.
const MARKS: &str = "0 g 20 20 60 60 re f\n\
    1 0 0 rg 120 120 40 40 re f\n\
    0 1 0 rg 200 300 30 30 re f\n\
    0 0 1 rg 350 450 40 40 re f\n";

fn page(boxes: &str) -> PdfDocument {
    let bytes = pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!("3 0 obj\n<< /Type /Page /Parent 2 0 R {boxes} /Contents 4 0 R >>\nendobj\n")
            .into_bytes(),
        stream_object(4, "", MARKS.as_bytes()),
    ]);
    PdfDocument::from_bytes(bytes).expect("fixture parses")
}

fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).expect("a PNG frame");
    assert_eq!(info.color_type, png::ColorType::Rgba, "RGBA pixels");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

/// Renders `rect` (user-space points) as a region and compares it to the
/// pixels `expected` (x, y, w, h in raster pixels) of the full render at
/// 72 dpi, one pixel per point, and returns the full raster's size.
fn prove(doc: &PdfDocument, rect: (f32, f32, f32, f32), expected: (u32, u32, u32, u32)) -> (u32, u32) {
    let options = RenderOptions::with_dpi(72);
    let full = render_page(doc, 0, &options).expect("full render");
    let (fw, fh, full_px) = decode(&full.data);
    let region = render_page_region(doc, 0, rect, &options).expect("region render");
    let (rw, rh, region_px) = decode(&region.data);

    let (x, y, w, h) = expected;
    assert_eq!((rw, rh), (w, h), "region size for {rect:?}");
    let mut cut = Vec::new();
    for row in y..y + h {
        let start = ((row * fw + x) * 4) as usize;
        cut.extend_from_slice(&full_px[start..start + w as usize * 4]);
    }
    assert!(cut == region_px, "region pixels differ from the full render's {expected:?}");
    // A cut of blank page would prove nothing.
    assert!(
        cut.chunks_exact(4).any(|p| p[..3] != [255, 255, 255]),
        "the cut holds a mark"
    );
    (fw, fh)
}

/// A page with no crop box is cut exactly as before the fix: the rectangle's
/// origin is the page's bottom-left. Guards the unchanged path; a Dart test
/// cannot compare against a full render of the same page.
#[test]
fn a_page_without_a_crop_box_is_cut_where_it_always_was() {
    let doc = page("/MediaBox [0 0 600 800]");
    // Red mark (120,120)-(160,160) and green (200,300)-(230,330).
    let (fw, fh) = prove(&doc, (100.0, 100.0, 150.0, 250.0), (100, 800 - 350, 150, 250));
    assert_eq!((fw, fh), (600, 800));
}

/// With `/CropBox [100 100 400 500]` the raster is the 300 x 400 crop, so
/// the rectangle must be placed against the crop box's corner and height,
/// not the media box's. The cut contains the red and green marks.
#[test]
fn a_region_of_a_cropped_page_is_cut_from_the_cropped_raster() {
    let doc = page("/MediaBox [0 0 600 800] /CropBox [100 100 400 500]");
    // Rect (110,110)-(260,360): raster x 10..160, y (500-360)..(500-110).
    let (fw, fh) = prove(&doc, (110.0, 110.0, 150.0, 250.0), (10, 140, 150, 250));
    assert_eq!((fw, fh), (300, 400));
}

/// On a cropped page turned by `/Rotate 90` the raster is 400 x 300 and the
/// rectangle is turned with it: PDF (x, y) lands at device
/// ((y - 100), (x - 100)).
#[test]
fn a_region_of_a_cropped_rotated_page_is_cut_from_the_turned_raster() {
    let doc = page("/MediaBox [0 0 600 800] /CropBox [100 100 400 500] /Rotate 90");
    // Rect x 110..260, y 110..360 -> device x 10..260, y 10..160.
    let (fw, fh) = prove(&doc, (110.0, 110.0, 150.0, 250.0), (10, 10, 250, 150));
    assert_eq!((fw, fh), (400, 300));
}
