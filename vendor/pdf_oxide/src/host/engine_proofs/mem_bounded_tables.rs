//! Tables the spec sizes are read to that size, whatever the stream holds past it.

use super::filter_readers::thread_growth;
use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::extractors::{extract_image_from_xobject, ImageData};
use crate::fonts::font_dict::{CIDToGIDMap, FontInfo};
use crate::object::{Object, ObjectRef};
use std::collections::HashMap;
use std::io::Write;

/// Bytes past the declared end of each table.
const STRAY: usize = 48 * 1024 * 1024;
/// What reading a table to its declared end may grow a thread by; a whole read
/// of the stream is `STRAY` and more.
const BOUND: usize = 8 * 1024 * 1024;

/// `head`, then `STRAY` zero bytes, Flate-coded block by block as generated.
fn flate_with_stray_tail(head: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(head).unwrap();
    let block = vec![0u8; 64 * 1024];
    for _ in 0..STRAY / block.len() {
        encoder.write_all(&block).unwrap();
    }
    encoder.finish().unwrap()
}

/// A catalog, one page and `extra` as objects 4 and up.
fn document(extra: Vec<Vec<u8>>) -> PdfDocument {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
    ];
    objects.extend(extra);
    PdfDocument::from_bytes(pdf_from_objects(&objects)).unwrap()
}

const PALETTE: [u8; 12] = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];

/// Four index samples over a four-entry RGB palette; the lookup is object 5.
fn indexed_image_document(lookup: &[u8], filter: &str) -> PdfDocument {
    document(vec![
        stream_object(
            4,
            "/Type /XObject /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 8 \
             /ColorSpace [/Indexed /DeviceRGB 3 5 0 R]",
            &[0, 1, 2, 3],
        ),
        stream_object(5, filter, lookup),
    ])
}

fn pixels_of(doc: &PdfDocument) -> Vec<u8> {
    let image = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    let image = extract_image_from_xobject(Some(doc), &image, None, None).unwrap();
    match image.data() {
        ImageData::Raw { pixels, .. } => pixels.clone(),
        ImageData::Jpeg(_) => panic!("an Indexed image of raw samples is raw"),
    }
}

/// An image whose lookup stream holds 48 MiB past its `(hival + 1) * N` bytes
/// gets the pixels the exact lookup gives, and the palette is not read past
/// its end. No Dart battery can observe this: the picture is the same either
/// way, and what differs is a buffer held for the length of the call.
#[test]
fn an_indexed_image_palette_is_read_to_its_declared_length() {
    let exact = pixels_of(&indexed_image_document(&PALETTE, ""));
    assert_eq!(exact, PALETTE);
    let doc = indexed_image_document(&flate_with_stray_tail(&PALETTE), "/Filter /FlateDecode");
    let (pixels, growth) = thread_growth(|| pixels_of(&doc));
    assert_eq!(pixels, exact);
    assert!(growth < BOUND, "reading a palette of 12 bytes grew the thread by {growth} bytes");
}

#[cfg(feature = "rendering")]
/// A colour painted in an Indexed space whose lookup stream holds 48 MiB past
/// its declared end is the entry the exact lookup gives, with the thread
/// growing by a table's length, not the stream's. No Dart battery can observe
/// this: the colour is the same either way, and what differs is a buffer held
/// for the length of the call.
#[test]
fn an_indexed_paint_palette_is_read_to_its_declared_length() {
    use crate::rendering::resolution::color::ColorResolver;
    use crate::rendering::resolution::{LogicalColor, ResolutionContext, ResolvedColor};
    let space = Object::Array(vec![
        Object::Name("Indexed".into()),
        Object::Name("DeviceRGB".into()),
        Object::Integer(3),
        Object::Reference(ObjectRef::new(4, 0)),
    ]);
    let paint = |doc: &PdfDocument| {
        let color_spaces = HashMap::new();
        let ctx = ResolutionContext::new(doc, &color_spaces);
        let color = LogicalColor::Spaced { space: &space, components: smallvec::smallvec![2.0] };
        match ColorResolver.resolve(&color, &ctx, 1.0).unwrap() {
            ResolvedColor::Rgba { r, g, b, .. } => [r, g, b],
            other => panic!("an RGB palette entry paints as RGB, got {other:?}"),
        }
    };
    let exact = paint(&document(vec![stream_object(4, "", &PALETTE)]));
    assert_eq!(exact, [70.0 / 255.0, 80.0 / 255.0, 90.0 / 255.0]);
    let doc = document(vec![stream_object(4, "/Filter /FlateDecode", &flate_with_stray_tail(&PALETTE))]);
    let (color, growth) = thread_growth(|| paint(&doc));
    assert_eq!(color, exact);
    assert!(growth < BOUND, "reading a palette of 12 bytes grew the thread by {growth} bytes");
}

/// A `CIDToGIDMap` stream that holds 48 MiB past its 65,536 entries maps every
/// CID as the exact stream does, and the font is read with the thread growing
/// by the map, not the stream. No Dart battery can observe this: text shows
/// the same glyphs either way, and what differs is a buffer held for the
/// length of the call.
#[test]
fn a_cid_to_gid_map_is_read_to_65536_entries() {
    let map: Vec<u8> = (0..=u16::MAX).flat_map(|cid| cid.wrapping_mul(7).to_be_bytes()).collect();
    assert_eq!(map.len(), 131_072);
    let font = |filter: &str, data: &[u8]| {
        let doc = document(vec![
            b"4 0 obj\n<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Probe \
              /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
              /CIDToGIDMap 5 0 R >>\nendobj\n"
                .to_vec(),
            stream_object(5, filter, data),
        ]);
        let type0 = Object::Dictionary(HashMap::from([
            ("Type".to_string(), Object::Name("Font".into())),
            ("Subtype".to_string(), Object::Name("Type0".into())),
            ("BaseFont".to_string(), Object::Name("Probe".into())),
            ("Encoding".to_string(), Object::Name("Identity-H".into())),
            ("DescendantFonts".to_string(), Object::Array(vec![Object::Reference(ObjectRef::new(4, 0))])),
        ]));
        thread_growth(|| FontInfo::from_dict(&type0, &doc).unwrap().cid_to_gid_map)
    };
    let (exact, _) = font("", &map);
    let Some(CIDToGIDMap::Explicit(exact)) = exact else { panic!("an explicit map, got {exact:?}") };
    assert_eq!(exact.len(), 65_536);
    let (stray, growth) = font("/Filter /FlateDecode", &flate_with_stray_tail(&map));
    let Some(CIDToGIDMap::Explicit(stray)) = stray else { panic!("an explicit map, got {stray:?}") };
    assert!(growth < BOUND, "reading a map of 65,536 entries grew the thread by {growth} bytes");
    assert_eq!(stray.len(), exact.len(), "entries past CID 65535");
    assert!(stray == exact, "the entries of the exact stream");
}

/// A raw-sample image whose stream holds 48 MiB past the `/Height` rows it
/// declares gives the pixels the exact stream gives, 8 and 16 bits a sample,
/// with the thread growing by the image, not by the stream. No Dart battery
/// can observe this: the picture is the same either way, and what differs is
/// a buffer held for the length of the call.
#[test]
fn a_raw_image_is_read_to_its_declared_rows() {
    for (depth, row) in [(8, 4usize * 3), (16, 4 * 3 * 2), (1, 1)] {
        let samples: Vec<u8> = (0..row * 4).map(|i| (i * 37 % 251) as u8).collect();
        let space = if depth == 1 { "/DeviceGray" } else { "/DeviceRGB" };
        let image = |filter: &str, data: &[u8]| {
            document(vec![stream_object(
                4,
                &format!(
                    "/Type /XObject /Subtype /Image /Width 4 /Height 4 /BitsPerComponent {depth} \
                     /ColorSpace {space} {filter}"
                ),
                data,
            )])
        };
        let picture = |doc: &PdfDocument| {
            let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
            let image = extract_image_from_xobject(Some(doc), &object, None, None).unwrap();
            (image.to_dynamic_image().unwrap().to_rgba8().into_raw(), pixels_of(doc))
        };
        let (exact_picture, exact) = picture(&image("", &samples));
        let doc = image("/Filter /FlateDecode", &flate_with_stray_tail(&samples));
        let ((stray_picture, pixels), growth) = thread_growth(|| picture(&doc));
        assert!(stray_picture == exact_picture, "{depth} bits: the picture");
        assert!(growth < BOUND, "{depth} bits: reading a {} byte image grew the thread by {growth} bytes", samples.len());
        assert_eq!(pixels.len(), exact.len(), "{depth} bits: the samples held");
        assert!(pixels == exact, "{depth} bits: the samples held");
    }
}
