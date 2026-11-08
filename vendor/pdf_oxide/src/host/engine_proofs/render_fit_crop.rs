//! A fit render sizes the page by the box the raster is cut to.
//!
//! The raster of a page is its `/CropBox` clipped to its `/MediaBox`; the fit
//! scale used to come from the media box alone, so a cropped page rendered
//! smaller than the box asked for.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::rendering::{render_page_fit, RenderOptions};

fn page_pdf(boxes: &str) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!("3 0 obj\n<< /Type /Page /Parent 2 0 R {boxes} /Contents 4 0 R >>\nendobj\n")
            .into_bytes(),
        stream_object(4, "", b"0.5 g 0 0 600 800 re f"),
    ])
}

fn fit_size(boxes: &str) -> (u32, u32) {
    let doc = PdfDocument::from_bytes(page_pdf(boxes)).expect("fixture parses");
    let image = render_page_fit(&doc, 0, 300, 300, &RenderOptions::default()).expect("renders");
    (image.width, image.height)
}

/// A 300x400 crop box inside a 600x800 media box, fitted to 300x300, comes out
/// 225x300: the crop box's aspect scaled to fit. The media box alone gave
/// a scale of 0.375 applied to the 300x400 raster, i.e. 113x150. No Dart test can
/// see this because the scale never leaves the engine; only the raster's size
/// shows it, and the Dart side cannot tell which box the size came from.
#[test]
fn cropped_page_fits_the_crop_box() {
    assert_eq!(fit_size("/MediaBox [0 0 600 800] /CropBox [100 100 400 500]"), (225, 300));
}

/// Without a crop box the fit is the media box's, unchanged: 600x800 into
/// 300x300 is 225x300. Guards the fix against moving pages that never had a
/// crop box; no Dart test pins this engine-internal sizing path.
#[test]
fn uncropped_page_keeps_its_fit_size() {
    assert_eq!(fit_size("/MediaBox [0 0 600 800]"), (225, 300));
}

/// A cropped page turned a quarter fits with width and height swapped: the
/// 300x400 crop box becomes 400x300 on screen, so 300x300 holds it at 300x225.
/// Proves the crop box and `/Rotate` compose in the scale; the Dart side only
/// sees the final pixel size of an engine-internal scale.
#[test]
fn rotated_cropped_page_fits_swapped() {
    assert_eq!(
        fit_size("/MediaBox [0 0 600 800] /CropBox [100 100 400 500] /Rotate 90"),
        (300, 225)
    );
}
