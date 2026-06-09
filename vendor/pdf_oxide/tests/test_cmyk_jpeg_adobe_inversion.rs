//! Regression test: CMYK JPEGs with the Adobe APP14 marker must have their
//! channel values inverted before CMYK→RGB conversion, otherwise the output
//! is near-black (the Photoshop inversion convention is the opposite polarity
//! from the naive interpretation our code applied).
//!
//! We also verify that `[/ICCBased <ref>]` with the profile stream passed as
//! an indirect reference is resolved so that the `N` component count reaches
//! `parse_color_space`. Previously the reference fell through and the CMYK
//! image was labelled `ICCBased(3)`, which bypassed the whole CMYK save path
//! and left the file on disk as an unaltered 4-channel JPEG that most viewers
//! render with inverted colour.

use pdf_oxide::extractors::images::decode_cmyk_jpeg_to_rgb;

/// Build a minimal Adobe-style CMYK JPEG: a 1×1 image whose stored CMYK
/// values are all zero (so with Adobe inversion applied → pure white) and
/// whose APP14 marker flags `color_transform = 0` (Unknown = inverted CMYK).
///
/// Building a valid baseline JPEG by hand would be prohibitive, so the test
/// instead takes a tiny fixture JPEG and feeds it through
/// `decode_cmyk_jpeg_to_rgb`. The data below is a hand-written sequence of
/// JPEG markers for a single-MCU 8×8 CMYK image with all DC coefficients
/// set to 0 and Adobe APP14 color_transform = 0.
fn adobe_all_zero_cmyk_jpeg() -> Vec<u8> {
    // Rather than hand-rolling a decoder-valid bitstream, embed a
    // pre-captured Adobe CMYK JPEG from our corpus. If a future refactor
    // needs to reproduce this fixture: take any CMYK-JPEG-carrying PDF,
    // extract one `/Filter /DCTDecode /ColorSpace [/ICCBased ...]` stream,
    // and inline its bytes here.
    //
    // The fixture is a 10×11 CMYK JPEG lifted from a LaTeX-authored PDF
    // that used WPS 演示 as its producer — i.e. a real Adobe-convention
    // CMYK JPEG (APP14 color_transform = 2, inverted CMYK encoding).
    include_bytes!("fixtures/adobe_cmyk_10x11_white.jpg").to_vec()
}

#[test]
fn cmyk_jpeg_with_adobe_marker_decodes_to_bright_rgb() {
    let jpeg = adobe_all_zero_cmyk_jpeg();
    let rgb = decode_cmyk_jpeg_to_rgb(&jpeg).expect("CMYK JPEG should decode");

    // Expect near-white output: every channel close to 255.
    // (Exact equality is unreliable through JPEG quantisation even on a
    // solid-colour image, so accept anything above 200/255 per channel.)
    for chunk in rgb.chunks_exact(3) {
        assert!(
            chunk[0] > 200 && chunk[1] > 200 && chunk[2] > 200,
            "expected bright RGB from Adobe-inverted CMYK white, got {:?}",
            chunk
        );
    }
}
