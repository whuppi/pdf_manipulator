//! A JPEG painted smaller than itself is decoded from its stream, not from a copy of its file.

use super::filter_readers::thread_growth;
use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::extractors::images::{decode_cmyk_jpeg_to_raw_cmyk, extract_image_from_xobject, extract_image_from_xobject_at};
use crate::extractors::{ColorSpace, ImageData, PdfImage};
use crate::host::images::classify::classify_stored;
use crate::host::images::execute::{Shape, Source};
use crate::host::images::stream::{ImageCursor, ImageRows, Output};
use crate::object::ObjectRef;
use std::collections::HashMap;

fn noise_jpeg(color: jpeg_encoder::ColorType, width: u16, height: u16, quality: u8) -> Vec<u8> {
    let bytes = match color {
        jpeg_encoder::ColorType::Luma => 1,
        jpeg_encoder::ColorType::Cmyk | jpeg_encoder::ColorType::Ycck => 4,
        _ => 3,
    };
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let pixels: Vec<u8> = (0..width as usize * height as usize * bytes)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect();
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, quality).encode(&pixels, width, height, color).unwrap();
    out
}

fn document(space: &str, width: u16, height: u16, jpeg: &[u8]) -> PdfDocument {
    PdfDocument::from_bytes(pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        stream_object(
            4,
            &format!(
                "/Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace {space} \
                 /BitsPerComponent 8 /Filter /DCTDecode"
            ),
            jpeg,
        ),
    ]))
    .unwrap()
}

/// What the whole bytes of the file give at `target`: the decode this change
/// moved onto the stream.
fn from_bytes(space: ColorSpace, width: u16, height: u16, jpeg: &[u8], target: (u32, u32)) -> image::DynamicImage {
    PdfImage::new(u32::from(width), u32::from(height), space, 8, ImageData::Jpeg(jpeg.to_vec()))
        .to_dynamic_image_covering(Some(target))
        .unwrap()
}

fn from_stream(doc: &PdfDocument, target: Option<(u32, u32)>) -> (image::DynamicImage, bool) {
    let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    let image = extract_image_from_xobject_at(Some(doc), &object, None, None, target).unwrap();
    let held = matches!(image.data(), ImageData::Jpeg(_));
    (image.to_dynamic_image_covering(target).unwrap(), held)
}

/// A gray and an RGB JPEG painted at an eighth, a quarter, and one size that
/// asks for no reduction decode to the pixels the file's bytes give, and
/// the extraction keeps the file only where the decode needs it whole. No
/// Dart battery can observe this: the picture is the same either way.
#[test]
fn a_jpeg_decodes_at_its_target_to_the_pixels_of_its_bytes() {
    for (name, color, space, doc_space) in [
        ("gray", jpeg_encoder::ColorType::Luma, ColorSpace::DeviceGray, "/DeviceGray"),
        ("rgb", jpeg_encoder::ColorType::Rgb, ColorSpace::DeviceRGB, "/DeviceRGB"),
    ] {
        let jpeg = noise_jpeg(color, 192, 128, 90);
        let doc = document(doc_space, 192, 128, &jpeg);
        for target in [(24, 16), (48, 32), (96, 64), (192, 128), (400, 300)] {
            let (got, held) = from_stream(&doc, Some(target));
            let want = from_bytes(space, 192, 128, &jpeg, target);
            // At its size the stream goes through jpeg-decoder and the file
            // through zune-jpeg, which round the inverse DCT apart by up to
            // 3; a reduction is jpeg-decoder on both sides.
            let tolerance = if target.0 >= 192 { 3 } else { 0 };
            assert!(within(got.as_bytes(), want.as_bytes(), tolerance), "{name} at {target:?}: the pixels");
            assert_eq!((got.width(), got.height()), (want.width(), want.height()), "{name} at {target:?}");
            assert!(!held, "{name} at {target:?}: the file is not the image");
        }
        let (whole, held) = from_stream(&doc, None);
        assert!(!held, "{name}: a decode with no target is a decode");
        assert!(within(whole.as_bytes(), from_bytes(space, 192, 128, &jpeg, (192, 128)).as_bytes(), 3));
        let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
        let library = extract_image_from_xobject(Some(&doc), &object, None, None).unwrap();
        assert!(
            matches!(library.data(), ImageData::Jpeg(bytes) if *bytes == jpeg),
            "{name}: the library's entry returns the file"
        );
    }
}

/// A JPEG of 14 MiB painted at an eighth of its size is decoded with the
/// thread growing by the raster and the decoder's rows, not by the file. No
/// Dart battery can observe this: the picture is the same either way, and
/// what differs is a copy of the file held for the length of the call.
#[test]
fn a_jpeg_painted_smaller_is_not_held_as_a_file() {
    let jpeg = noise_jpeg(jpeg_encoder::ColorType::Rgb, 2048, 1536, 100);
    assert!(jpeg.len() > 12 * 1024 * 1024, "the fixture is {} bytes", jpeg.len());
    let doc = document("/DeviceRGB", 2048, 1536, &jpeg);
    let ((_, held), growth) = thread_growth(|| from_stream(&doc, Some((256, 192))));
    assert!(!held);
    assert!(growth < 4 * 1024 * 1024, "decoding a {} byte JPEG at an eighth grew the thread by {growth} bytes", jpeg.len());
}

/// The file with its Adobe APP14 segment taken out: a four-component JPEG
/// that names no transform.
fn without_app14(jpeg: &[u8]) -> Vec<u8> {
    let mut out = jpeg[..2].to_vec();
    let mut at = 2;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xFF && jpeg[at + 1] != 0xDA {
        let end = at + 2 + usize::from(u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]));
        if jpeg[at + 1] != 0xEE {
            out.extend_from_slice(&jpeg[at..end]);
        }
        at = end;
    }
    out.extend_from_slice(&jpeg[at..]);
    assert!(out.len() < jpeg.len(), "the fixture has an APP14 segment");
    out
}

fn open_rows(doc: &PdfDocument, target: Option<(u32, u32)>) -> ImageRows {
    let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    ImageRows::open(doc, &object, &HashMap::new(), target, Output::Rgba, None).unwrap()
}

fn open_cmyk_source(doc: &PdfDocument) -> Source {
    let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    let Some(crate::object::Object::Stream { dict, .. }) = Some(&object) else { unreachable!() };
    let kind = classify_stored(doc, dict, 0, &[]);
    Source::open(doc, &object, &kind, Shape::Cmyk, &HashMap::new(), None).unwrap()
}

/// Whether two rasters differ by at most `tolerance` in any sample.
fn within(a: &[u8], b: &[u8], tolerance: u8) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= tolerance)
}

/// The rows of a source, end to end.
fn rows_of(mut read: impl FnMut(&mut [u8]), height: u32, row_bytes: usize) -> Vec<u8> {
    let mut all = Vec::with_capacity(height as usize * row_bytes);
    let mut row = vec![0u8; row_bytes];
    for _ in 0..height {
        read(&mut row);
        all.extend_from_slice(&row);
    }
    all
}

/// Each path a lane reads a JPEG by gives the pixels the file's bytes give:
/// the extractor at a target and without one, the row reader `render` and
/// `reduceImages` open, and the CMYK rows `reduceImages` reads. RGB at its own
/// size and reduced, CMYK with no Adobe marker, with transform 0 and with
/// transform 2. No Dart battery can observe this: it pins the decoder the
/// stream is read by to the decode the file's bytes got.
#[test]
fn a_jpeg_on_a_lane_decodes_to_the_pixels_of_its_bytes_in_every_path() {
    let rgb = noise_jpeg(jpeg_encoder::ColorType::Rgb, 192, 128, 90);
    // A file at its own size is decoded by `image`'s decoder, a stream by
    // `jpeg-decoder`; the two round the colour conversion differently, by 3 at
    // most. A reduced decode is `jpeg-decoder`'s on both sides.
    for (name, target, tolerance) in [("rgb at its size", (192, 128), 3), ("rgb reduced", (48, 32), 0)] {
        let doc = document("/DeviceRGB", 192, 128, &rgb);
        let want = from_bytes(ColorSpace::DeviceRGB, 192, 128, &rgb, target);
        let (got, held) = from_stream(&doc, Some(target));
        assert!(!held && within(got.as_bytes(), want.as_bytes(), tolerance), "{name}: the extractor");
        let mut rows = open_rows(&doc, Some(target));
        assert_eq!(rows.size(), (want.width(), want.height()), "{name}: the size");
        let read = rows_of(|row| rows.read(&doc, row).unwrap(), want.height(), want.width() as usize * 4);
        assert!(within(&read, &want.to_rgba8().into_raw(), tolerance), "{name}: the rows");
    }

    let cmyk = noise_jpeg(jpeg_encoder::ColorType::Cmyk, 96, 64, 90);
    let ycck = noise_jpeg(jpeg_encoder::ColorType::Ycck, 96, 64, 90);
    for (name, jpeg) in [("cmyk, no marker", without_app14(&cmyk)), ("cmyk, transform 0", cmyk), ("cmyk, transform 2", ycck)] {
        let doc = document("/DeviceCMYK", 96, 64, &jpeg);
        let want = from_bytes(ColorSpace::DeviceCMYK, 96, 64, &jpeg, (96, 64));
        let (got, held) = from_stream(&doc, Some((96, 64)));
        assert!(!held && got.as_bytes() == want.as_bytes(), "{name}: the extractor");
        let mut rows = open_rows(&doc, Some((96, 64)));
        let read = rows_of(|row| rows.read(&doc, row).unwrap(), 64, 96 * 4);
        assert!(read == want.to_rgba8().into_raw(), "{name}: the rows");

        let mut source = open_cmyk_source(&doc);
        assert_eq!(source.size(), (96, 64), "{name}: the CMYK size");
        let read = rows_of(|row| source.read(&doc, row).unwrap(), 64, 96 * 4);
        assert!(read == decode_cmyk_jpeg_to_raw_cmyk(&jpeg).unwrap(), "{name}: the CMYK rows");
    }
}

/// A DCT stream with no readable header is an error at the extractor, not the
/// file handed on, with a target and without one.
#[test]
fn a_jpeg_the_stream_decoder_cannot_name_is_an_error_not_the_file() {
    let doc = document("/DeviceRGB", 64, 64, b"these bytes are not a JPEG, they are text");
    let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    for target in [None, Some((64, 64)), Some((8, 8))] {
        let got = extract_image_from_xobject_at(Some(&doc), &object, None, None, target);
        assert!(matches!(got, Err(crate::error::Error::Image(_))), "target {target:?}: {got:?}");
    }
}

/// A 2048 x 1536 noise JPEG of 12 MiB, whose file is larger than its raster,
/// painted at its own size, is decoded with the thread growing by what the
/// decoder holds (its component planes beside the raster it returns) and not
/// by the file as well. No Dart battery can observe this: the picture
/// is the same either way.
#[test]
fn a_jpeg_painted_at_its_size_is_not_held_as_a_file() {
    let jpeg = noise_jpeg(jpeg_encoder::ColorType::Rgb, 2048, 1536, 100);
    let raster = 2048 * 1536 * 3;
    assert!(jpeg.len() > raster, "the file is {} bytes, the raster {raster}", jpeg.len());
    let doc = document("/DeviceRGB", 2048, 1536, &jpeg);
    for target in [(2048, 1536), (3000, 2000)] {
        let ((_, held), growth) = thread_growth(|| from_stream(&doc, Some(target)));
        assert!(!held, "{target:?}");
        assert!(
            growth < 2 * raster + 4 * 1024 * 1024,
            "decoding a {} byte JPEG at {target:?} grew the thread by {growth} bytes, the raster is {raster}",
            jpeg.len()
        );
    }
}

/// A 2048 x 1536 CMYK noise JPEG on a lane, through the rows `reduceImages`
/// reads and the ones `render` reads, is not held as a file beside its
/// raster. No Dart battery can observe this: the picture is the same either
/// way.
#[test]
fn a_cmyk_jpeg_on_a_lane_is_not_held_as_a_file() {
    let jpeg = noise_jpeg(jpeg_encoder::ColorType::Cmyk, 2048, 1536, 100);
    let raster = 2048 * 1536 * 4;
    assert!(jpeg.len() > raster, "the file is {} bytes, the raster {raster}", jpeg.len());
    let doc = document("/DeviceCMYK", 2048, 1536, &jpeg);
    let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    // The decoder's component planes are held beside the raster it returns.
    let bound = 2 * raster + 4 * 1024 * 1024;

    let (source, growth) = thread_growth(|| open_cmyk_source(&doc));
    assert_eq!(source.size(), (2048, 1536));
    assert!(growth < bound, "CmykRows::open grew the thread by {growth} bytes, the raster is {raster}");
    drop(source);

    let (rows, growth) = thread_growth(|| open_rows(&doc, Some((2048, 1536))));
    assert_eq!(rows.size(), (2048, 1536));
    assert!(growth < bound, "ImageRows::open grew the thread by {growth} bytes, the raster is {raster}");
    drop(rows);

    let (cursor, growth) = thread_growth(|| {
        ImageCursor::open(&doc, &object, &HashMap::new(), None, None, Some((2048, 1536))).unwrap()
    });
    drop(cursor);
    assert!(growth < bound, "ImageCursor::open grew the thread by {growth} bytes, the raster is {raster}");
}
