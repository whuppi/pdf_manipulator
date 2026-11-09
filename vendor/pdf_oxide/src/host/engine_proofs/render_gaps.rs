//! What a render owes after it, and what a mask that cannot be read does.
//!
//! A render keeps its images open from band to band, so it must give them up
//! on every way out; a JPEG 2000 image's colour-key `/Mask` is tested against
//! its samples like any other; and a mask the file gives that cannot be read
//! hides nothing of the picture it was meant to refine.

use super::band_proof::Fixture;
use super::support::stream_object;
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::error::Error;
use crate::rendering::{render_page, PageRenderer, RenderOptions};

fn raw_rgb(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|i| [(i % width * 255 / width) as u8, (i / width * 255 / height) as u8, 90])
        .collect()
}

/// A page whose one image is turned, so that every band reads a rectangle of
/// it and the image stays open from band to band.
fn turned_image_page() -> Vec<u8> {
    let mut fixture = Fixture::new("gap", "q 120 90 -90 120 100 20 cm /Im Do Q\n");
    fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
    fixture.extra = vec![stream_object(
        5,
        "/Type /XObject /Subtype /Image /Width 200 /Height 200 /ColorSpace /DeviceRGB \
         /BitsPerComponent 8",
        &raw_rgb(200, 200),
    )];
    fixture.pdf()
}

/// Renders `pdf` in bands of 17 rows and fails in the callback of the band
/// after the first, as a failed write to a sink or a cancel would; returns what
/// the renderer still held.
fn bytes_held_after_a_render_that_fails(pdf: Vec<u8>) -> usize {
    let doc = PdfDocument::from_bytes(pdf).expect("fixture parses");
    let mut renderer = PageRenderer::new(RenderOptions::with_dpi(72).with_transparent_background());
    renderer.forced_rows = Some(17);
    let mut bands = 0;
    let result = renderer.render_bands(&doc, 0, &|_, height| 0..height, &CancelCheck::none(), &mut |_, _| {
        bands += 1;
        if bands == 4 {
            return Err(Error::InvalidPdf("the sink refused the band".to_string()));
        }
        Ok(())
    });
    assert!(result.is_err(), "the render was meant to fail");
    assert_eq!(bands, 4, "the render stopped where it was made to");
    renderer.open_image_bytes()
}

/// No Dart battery sees it: the renderer's own memory after a failed render is
/// not visible through the API.
#[test]
fn a_render_that_fails_leaves_no_open_image_behind() {
    assert_eq!(bytes_held_after_a_render_that_fails(turned_image_page()), 0);
}

fn page_pixels(pdf: Vec<u8>) -> image::RgbaImage {
    let doc = PdfDocument::from_bytes(pdf).expect("fixture parses");
    let rendered =
        render_page(&doc, 0, &RenderOptions::with_dpi(72).with_transparent_background()).expect("renders");
    image::load_from_memory(&rendered.data).expect("PNG decodes").to_rgba8()
}

/// A page of a red backdrop and the JPEG 2000 gray image over it, keyed by `key`.
fn jpx_page(key: &str) -> Vec<u8> {
    let jp2: &[u8] = include_bytes!("../../../tests/fixtures/jpx/sample_gray.jp2");
    let decoded = crate::decoders::jpx::decode_jpx_at(jp2, None).expect("decodes");
    let mut fixture =
        Fixture::new("jpxkey", "1 0 0 rg 0 0 200 200 re f q 200 0 0 200 0 0 cm /Im Do Q\n");
    fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
    fixture.extra = vec![stream_object(
        5,
        &format!(
            "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Filter /JPXDecode {key}",
            decoded.width, decoded.height
        ),
        jp2,
    )];
    fixture.pdf()
}

fn backdrop_pixels(pixels: &image::RgbaImage) -> usize {
    pixels.pixels().filter(|p| p.0 == [255, 0, 0, 255]).count()
}

/// No Dart battery sees it: it needs a JPEG 2000 image with a colour key, which
/// no fixture the Dart tests build has.
#[test]
fn a_jpx_colour_key_clears_the_pixels_whose_samples_are_in_range() {
    let plain = backdrop_pixels(&page_pixels(jpx_page("")));
    let all = backdrop_pixels(&page_pixels(jpx_page("/Mask [0 255]")));
    // The picture is a page of mostly white: a key below 255 clears its ink only.
    let some = backdrop_pixels(&page_pixels(jpx_page("/Mask [0 254]")));
    assert_eq!(plain, 0, "the image covers the backdrop");
    assert_eq!(all, 200 * 200, "a key that spans every sample clears every pixel");
    assert!(some > 0 && some < all, "a key below white clears some pixels, not all: {some}");
}

/// The page of a gray image, masked by the stencil `mask` stream with `dict`.
fn masked_page(dict: &str, mask: &[u8]) -> Vec<u8> {
    let mut fixture = Fixture::new("stencil", "1 0 0 rg 0 0 200 200 re f q 200 0 0 200 0 0 cm /Im Do Q\n");
    fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
    fixture.extra = vec![
        stream_object(
            5,
            "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Mask 6 0 R",
            &[0x80u8; 64],
        ),
        stream_object(6, dict, mask),
    ];
    fixture.pdf()
}

/// No Dart battery sees it: a mask whose stream cannot be decoded is not
/// something the Dart tests can write.
#[test]
fn a_stencil_mask_that_cannot_be_decoded_masks_nothing() {
    let unmasked = page_pixels({
        let mut fixture = Fixture::new("plain", "1 0 0 rg 0 0 200 200 re f q 200 0 0 200 0 0 cm /Im Do Q\n");
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![stream_object(
            5,
            "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray \
             /BitsPerComponent 8",
            &[0x80u8; 64],
        )];
        fixture.pdf()
    });
    assert_eq!(backdrop_pixels(&unmasked), 0);
    for (what, dict, data) in [
        (
            "a JBIG2 stream that is not one",
            "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true /Filter /JBIG2Decode",
            b"not a JBIG2 stream".as_slice(),
        ),
        (
            "a Flate stream that is not one",
            "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true /Filter /FlateDecode",
            b"not a Flate stream".as_slice(),
        ),
    ] {
        let masked = page_pixels(masked_page(dict, data));
        assert_eq!(masked, unmasked, "{what} must leave the picture as it is");
    }
}
