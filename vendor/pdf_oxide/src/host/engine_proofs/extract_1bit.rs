//! Sub-byte gray images under a Flate predictor read every sample, through
//! the whole-image extractor and through the row cursor alike.
//!
//! Images of 1, 2 and 4 bits a sample, widths that are not whole bytes,
//! PNG-predicted (`/Predictor 15`, rows tagged None, Sub, Up and Average in
//! turn) and TIFF-predicted (`/Predictor 2`), are read by
//! `PdfDocument::extract_images` and by `ImageRows`; both give the pattern
//! the image was built from.
//!
//! No Dart battery can observe this: the batteries see the PNG the
//! extractor writes for a small image, and a short sample buffer is padded
//! or cropped on its way there, so only the sample count and values the
//! extractor holds can show a predictor that read the wrong row geometry.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::extractors::ImageData;
use crate::host::images::stream::{ImageRows, Output};
use crate::object::ObjectRef;
use std::collections::HashMap;
use std::io::Write;

const HEIGHT: usize = 13;

fn sample(x: usize, y: usize, bpc: usize) -> u8 {
    ((x * 3 + y * 5 + x * y) % (1 << bpc)) as u8
}

/// A row of samples, packed MSB first and padded to a whole byte with 0.
fn pack(values: &[u8], bpc: usize) -> Vec<u8> {
    let mut row = vec![0u8; (values.len() * bpc).div_ceil(8)];
    for (x, &v) in values.iter().enumerate() {
        let bit = x * bpc;
        row[bit / 8] |= v << (8 - bpc - bit % 8);
    }
    row
}

fn flate(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

#[derive(Clone, Copy, Debug)]
enum Predictor {
    Png,
    Tiff,
}

/// The stream's dictionary words and its data for the pattern.
fn encoded(width: usize, bpc: usize, predictor: Predictor) -> (String, Vec<u8>) {
    let rows: Vec<Vec<u8>> = (0..HEIGHT)
        .map(|y| pack(&(0..width).map(|x| sample(x, y, bpc)).collect::<Vec<_>>(), bpc))
        .collect();
    let (number, data) = match predictor {
        Predictor::Png => {
            // bpp is ceil(bpc / 8) = 1 for every depth here.
            let mut out = Vec::new();
            for (y, row) in rows.iter().enumerate() {
                let tag = (y % 4) as u8;
                out.push(tag);
                for (i, &b) in row.iter().enumerate() {
                    let left = if i > 0 { row[i - 1] } else { 0 };
                    let up = if y > 0 { rows[y - 1][i] } else { 0 };
                    out.push(match tag {
                        0 => b,
                        1 => b.wrapping_sub(left),
                        2 => b.wrapping_sub(up),
                        _ => b.wrapping_sub(((left as u16 + up as u16) / 2) as u8),
                    });
                }
            }
            (15, out)
        },
        Predictor::Tiff => {
            // Each sample is the difference from the one before it, mod 2^bpc.
            let mask = ((1u16 << bpc) - 1) as u8;
            let mut out = Vec::new();
            for y in 0..HEIGHT {
                let diffs: Vec<u8> = (0..width)
                    .map(|x| {
                        let prev = if x > 0 { sample(x - 1, y, bpc) } else { 0 };
                        sample(x, y, bpc).wrapping_sub(prev) & mask
                    })
                    .collect();
                out.extend(pack(&diffs, bpc));
            }
            (2, out)
        },
    };
    (
        format!(
            "/Type /XObject /Subtype /Image /Width {width} /Height {HEIGHT} \
             /ColorSpace /DeviceGray /BitsPerComponent {bpc} /Filter /FlateDecode \
             /DecodeParms << /Predictor {number} /Colors 1 /BitsPerComponent {bpc} \
             /Columns {width} >>"
        ),
        flate(&data),
    )
}

fn pdf(dict: &str, data: &[u8]) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] \
          /Resources << /XObject << /Im 5 0 R >> >> /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", b"q 100 0 0 100 0 0 cm /Im Do Q\n"),
        stream_object(5, dict, data),
    ])
}

/// Both readers give `width * HEIGHT` samples equal to the pattern scaled to
/// 0..255.
fn prove(width: usize, bpc: usize, predictor: Predictor) {
    let (words, data) = encoded(width, bpc, predictor);
    let doc = PdfDocument::from_bytes(pdf(&words, &data)).expect("fixture parses");
    let scale = |v: u8| (v as u32 * 255 / ((1 << bpc) - 1)) as u8;
    let expected: Vec<u8> = (0..HEIGHT)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| scale(sample(x, y, bpc)))
        .collect();
    let what = format!("width {width}, {bpc} bit, {predictor:?}");

    let images = doc.extract_images(0).expect("extracts");
    assert_eq!(images.len(), 1, "{what}: one image");
    match images[0].data() {
        ImageData::Raw { pixels, .. } => {
            assert_eq!(pixels.len(), expected.len(), "{what}: the extractor's sample count");
            assert_eq!(pixels, &expected, "{what}: the extractor's samples");
        },
        ImageData::Jpeg(_) => panic!("{what}: a Flate image is raw samples"),
    }

    let xobject = doc.load_object(ObjectRef::new(5, 0)).expect("the image object");
    let mut rows = ImageRows::open(&doc, &xobject, &HashMap::new(), None, Output::Luma, None)
        .expect("the cursor opens");
    assert_eq!(rows.size(), (width as u32, HEIGHT as u32), "{what}: the cursor's size");
    for y in 0..HEIGHT {
        let mut row = vec![0u8; width];
        rows.read(&doc, &mut row).expect("a row");
        assert_eq!(row, expected[y * width..(y + 1) * width], "{what}: cursor row {y}");
    }
}

/// A 1-bit gray image whose width is not a whole number of bytes, Flate
/// with a PNG predictor, is read in full by the extractor and the cursor.
/// The extractor's sample buffer is internal: a battery sees only the PNG
/// written from it.
#[test]
fn a_one_bit_png_predicted_image_reads_every_sample() {
    for width in [9, 17, 70] {
        prove(width, 1, Predictor::Png);
    }
}

/// The same images under the TIFF predictor, which works on samples, not
/// bytes: at one bit it differences neighbouring bits. Internal sample
/// buffers again, out of a battery's sight.
#[test]
fn a_one_bit_tiff_predicted_image_reads_every_sample() {
    for width in [9, 17, 70] {
        prove(width, 1, Predictor::Tiff);
    }
}

/// Two and four bits a sample follow the same row rule under the PNG
/// predictor; one case each, read through both readers.
#[test]
fn two_and_four_bit_png_predicted_images_read_every_sample() {
    prove(9, 2, Predictor::Png);
    prove(9, 4, Predictor::Png);
}
