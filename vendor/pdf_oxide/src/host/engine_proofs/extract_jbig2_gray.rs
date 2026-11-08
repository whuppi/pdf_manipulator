//! A JBIG2 image extracted whole is the 8-bit gray raster of its bitmap.

use super::band_proof::{bilevel, jbig2};
use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::extractors::{extract_image_from_xobject, ImageData, PixelFormat};
use crate::object::ObjectRef;

fn extracted(extra: &str, picture: &[Vec<bool>]) -> (Vec<u8>, bool, u8) {
    let doc = PdfDocument::from_bytes(pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        stream_object(
            4,
            &format!(
                "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray \
                 /BitsPerComponent 1 /Filter /JBIG2Decode {extra}",
                picture[0].len(),
                picture.len()
            ),
            &jbig2(picture),
        ),
    ]))
    .unwrap();
    let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    let image = extract_image_from_xobject(Some(&doc), &object, None, None).unwrap();
    let ImageData::Raw { pixels, format } = image.data() else { panic!("JBIG2 decodes to raw samples") };
    assert_eq!(*format, PixelFormat::Grayscale);
    (pixels.clone(), image.samples_are_raw(), image.bits_per_component())
}

/// The whole-image path gives one byte a pixel, black 0 and white 255, the
/// other way about under `/Decode [1 0]`, and says its samples are not the
/// stream's own bits. No Dart battery can observe this: Dart extracts the
/// image through the row cursor, and only the library's whole-image call
/// hands out this raster.
#[test]
fn a_jbig2_image_extracted_whole_is_its_bitmap_as_8_bit_gray() {
    let picture = bilevel(64, 48);
    let expected: Vec<u8> = picture.iter().flatten().map(|&black| if black { 0 } else { 255 }).collect();
    let (plain, raw, bits) = extracted("", &picture);
    assert!(plain == expected, "the bitmap as gray bytes");
    assert!(!raw);
    assert_eq!(bits, 8);
    let (inverted, _, _) = extracted("/Decode [1 0]", &picture);
    let negative: Vec<u8> = expected.iter().map(|b| !b).collect();
    assert!(inverted == negative, "the bitmap as gray bytes under /Decode [1 0]");
}
