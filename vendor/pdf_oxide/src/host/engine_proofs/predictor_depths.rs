//! The PNG and TIFF predictors at depths other than 8 bits a component.
//!
//! Flate images with a known sample pattern are encoded here straight from
//! the formulas of ISO 32000-1 §7.4.4.3 (TIFF) and §7.4.4.4 (PNG, whose byte
//! distance is `ceil(Colors × BitsPerComponent / 8)`, at least 1), then read
//! through `PdfDocument::extract_images` and through the row cursor
//! `ImageRows`; both must give back the pattern.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::extractors::images::{ImageData, PixelFormat};
use crate::host::images::stream::{ImageRows, Output};
use crate::object::ObjectRef;
use std::collections::HashMap;
use std::io::Write;

/// Past the extractor's minimum image size of 8 pixels a side, and wide
/// enough that rows are not whole bytes at the shallow depths.
const WIDTH: usize = 9;
const HEIGHT: usize = 9;

/// The sample of column `x`, row `y`, component `c`, over the whole range of
/// `bits`.
fn sample(x: usize, y: usize, c: usize, bits: usize) -> u32 {
    let seed = (x as u64 * 7919 + y as u64 * 104_729 + c as u64 * 31_337 + 12_345)
        .wrapping_mul(2_654_435_761);
    ((seed >> 7) & ((1u64 << bits) - 1)) as u32
}

/// The packed rows of the pattern: samples MSB first, rows padded to a byte.
fn packed_rows(colors: usize, bits: usize) -> Vec<Vec<u8>> {
    (0..HEIGHT)
        .map(|y| {
            let mut row = vec![0u8; (WIDTH * colors * bits).div_ceil(8)];
            let mut at = 0;
            for x in 0..WIDTH {
                for c in 0..colors {
                    let v = sample(x, y, c, bits);
                    for b in (0..bits).rev() {
                        if (v >> b) & 1 == 1 {
                            row[at / 8] |= 0x80 >> (at % 8);
                        }
                        at += 1;
                    }
                }
            }
            row
        })
        .collect()
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i32::from(a), i32::from(b), i32::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// PNG-filters the rows with `tag` on every row (§7.4.4.4).
fn png_encode(rows: &[Vec<u8>], colors: usize, bits: usize, tag: u8) -> Vec<u8> {
    let bpp = (colors * bits).div_ceil(8).max(1);
    let mut out = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        out.push(tag);
        for i in 0..row.len() {
            let a = if i >= bpp { row[i - bpp] } else { 0 };
            let b = if y > 0 { rows[y - 1][i] } else { 0 };
            let c = if y > 0 && i >= bpp { rows[y - 1][i - bpp] } else { 0 };
            let prediction = match tag {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                _ => paeth(a, b, c),
            };
            out.push(row[i].wrapping_sub(prediction));
        }
    }
    out
}

/// TIFF-filters 16-bit rows: each sample is the difference from the same
/// component of the pixel to its left, modulo 65536 (§7.4.4.3).
fn tiff_encode_16(colors: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            for c in 0..colors {
                let v = sample(x, y, c, 16) as u16;
                let left = if x > 0 { sample(x - 1, y, c, 16) as u16 } else { 0 };
                out.extend_from_slice(&v.wrapping_sub(left).to_be_bytes());
            }
        }
    }
    out
}

fn flate(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(data).unwrap();
    z.finish().unwrap()
}

fn one_page_pdf(image_dict: &str, data: &[u8]) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 70 50] \
          /Resources << /XObject << /Im 5 0 R >> >> /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", b"q 70 0 0 50 0 0 cm /Im Do Q\n"),
        stream_object(5, image_dict, data),
    ])
}

/// What a sample of `bits` becomes as an 8-bit value.
fn to_8(v: u32, bits: usize) -> u8 {
    let max = (1u32 << bits) - 1;
    ((v * 255 + max / 2) / max) as u8
}

/// The RGBA the pattern should read as: grey for one colour, else RGB.
fn expected(colors: usize, bits: usize) -> Vec<Vec<u8>> {
    (0..HEIGHT)
        .map(|y| {
            let mut row = Vec::new();
            for x in 0..WIDTH {
                let at = |c: usize| to_8(sample(x, y, c, bits), bits);
                if colors == 1 {
                    row.extend_from_slice(&[at(0), at(0), at(0), 255]);
                } else {
                    row.extend_from_slice(&[at(0), at(1), at(2), 255]);
                }
            }
            row
        })
        .collect()
}

/// Reads the image both ways and compares each with the pattern.
fn prove(label: &str, colors: usize, bits: usize, parms: &str, encoded: &[u8]) {
    let space = if colors == 1 { "DeviceGray" } else { "DeviceRGB" };
    let dict = format!(
        "/Type /XObject /Subtype /Image /Width {WIDTH} /Height {HEIGHT} /ColorSpace /{space} \
         /BitsPerComponent {bits} /Filter /FlateDecode \
         /DecodeParms << /Colors {colors} /Columns {WIDTH} /BitsPerComponent {bits} {parms} >>"
    );
    let doc = PdfDocument::from_bytes(one_page_pdf(&dict, &flate(encoded))).expect("fixture parses");
    let want = expected(colors, bits);

    let images = doc.extract_images(0).expect("the extractor answers");
    assert_eq!(images.len(), 1, "{label}: one image");
    let ImageData::Raw { pixels, format } = images[0].data() else {
        panic!("{label}: raw pixels");
    };
    let width_bytes = match format {
        PixelFormat::Grayscale => 1,
        PixelFormat::RGB => 3,
        other => panic!("{label}: unexpected pixel format {other:?}"),
    };
    assert_eq!(pixels.len(), WIDTH * HEIGHT * width_bytes, "{label}: extract_images size");
    for y in 0..HEIGHT {
        let mut got = Vec::new();
        for x in 0..WIDTH {
            let p = &pixels[(y * WIDTH + x) * width_bytes..][..width_bytes];
            if width_bytes == 1 {
                got.extend_from_slice(&[p[0], p[0], p[0], 255]);
            } else {
                got.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        assert_eq!(got, want[y], "{label}: extract_images row {y}");
    }

    let xobject = doc.load_object(ObjectRef::new(5, 0)).expect("the image object");
    let mut rows = ImageRows::open(&doc, &xobject, &HashMap::new(), None, Output::Rgba, None)
        .expect("the cursor opens");
    for (y, want_row) in want.iter().enumerate() {
        let mut got = vec![0u8; WIDTH * 4];
        rows.read(&doc, &mut got).expect("a row");
        assert_eq!(&got, want_row, "{label}: ImageRows row {y}");
    }
}

fn png_depth(colors: usize, bits: usize) {
    let rows = packed_rows(colors, bits);
    for tag in 0..=4u8 {
        let label = format!("png tag {tag}, {colors} colour(s), {bits} bits");
        prove(&label, colors, bits, "/Predictor 15", &png_encode(&rows, colors, bits, tag));
    }
}

/// A 16-bit grey image under the PNG predictors (each tag in turn) must read
/// back its samples: the byte distance is 2, not 1. No Dart battery reaches
/// a predicted 16-bit stream with a known pattern, and what it extracts is
/// only checked as a file; the exact pixel values are the engine's side.
#[test]
fn png_predictor_16_bit_one_colour() {
    png_depth(1, 16);
}

/// A 16-bit RGB image under the PNG predictors: the byte distance is 6, not
/// 3. A Dart battery sees a picture, not whether each byte was reversed from
/// the right neighbour.
#[test]
fn png_predictor_16_bit_three_colours() {
    png_depth(3, 16);
}

/// A 2-bit RGB image under the PNG predictors: a pixel is 6 bits, so the
/// byte distance is 1, not 3. Only the exact decoded samples separate the
/// two, and a Dart battery does not compare them.
#[test]
fn png_predictor_2_bit_three_colours() {
    png_depth(3, 2);
}

/// A 4-bit RGB image under the PNG predictors: a pixel is 12 bits, so the
/// byte distance is 2, not 3. Only the exact decoded samples separate the
/// two, and a Dart battery does not compare them.
#[test]
fn png_predictor_4_bit_three_colours() {
    png_depth(3, 4);
}

/// A 16-bit grey image under the TIFF predictor adds 16-bit big-endian
/// samples modulo 65536, not bytes; the carry between the two bytes of a
/// sample is only visible in the exact decoded values.
#[test]
fn tiff_predictor_16_bit_one_colour() {
    prove("tiff 16 bits, 1 colour", 1, 16, "/Predictor 2", &tiff_encode_16(1));
}

/// A 16-bit RGB image under the TIFF predictor: the left neighbour of a
/// sample is the same component of the previous pixel, two bytes per
/// component, and the sum carries across the two bytes.
#[test]
fn tiff_predictor_16_bit_three_colours() {
    prove("tiff 16 bits, 3 colours", 3, 16, "/Predictor 2", &tiff_encode_16(3));
}
