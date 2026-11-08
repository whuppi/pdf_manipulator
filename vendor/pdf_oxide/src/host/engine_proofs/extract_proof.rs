//! What extractImages writes: the descriptor first, a JPEG or JPEG 2000
//! stream byte for byte, every other image as a PNG whose pixels are the ones
//! the extractor it replaced decoded.
//!
//! A Dart battery sees the bytes the host writes but not the decoder they
//! replace, so only here can the two be compared.

use super::band_proof::{bilevel, detailed_rgb, group4, jbig2, packed_bits};
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::host::binary_codec::Request;
use crate::host::chunked::DATA_FRAME_BYTES;
use crate::host::extract_stream::extract_images_streamed;
use crate::object::Object;
use std::io::Write;

/// One image of the fixture and what the wire must say of it.
struct Case {
    name: &'static str,
    dict: String,
    data: Vec<u8>,
    width: u32,
    height: u32,
    encoding: &'static str,
    color: &'static str,
    bits: i32,
    /// The soft mask this image names, its object written after it.
    smask: Option<(String, Vec<u8>)>,
}

fn flate(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(data).expect("compresses");
    z.finish().expect("finishes")
}

fn cases() -> Vec<Case> {
    let case = |name, dict: &str, data: Vec<u8>, w, h, encoding, color, bits| Case {
        name,
        dict: dict.to_string(),
        data,
        width: w,
        height: h,
        encoding,
        color,
        bits,
        smask: None,
    };
    let gray = |w: u32, h: u32| -> Vec<u8> { (0..w * h).map(|i| (i * 7 % 251) as u8).collect() };
    let picture = bilevel(64, 48);
    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, 85)
        .encode(&detailed_rgb(24, 24), 24, 24, jpeg_encoder::ColorType::Rgb)
        .expect("encodes");
    let jp2: &[u8] = include_bytes!("../../../tests/fixtures/jpx/sample_gray.jp2");
    let decoded = crate::decoders::jpx::decode_jpx_at(jp2, None).expect("decodes");
    let palette: String = (0..16u8)
        .flat_map(|i| [i * 16, 255 - i * 16, i * 8])
        .map(|b| format!("{b:02x}"))
        .collect();
    let run_length = {
        let row = gray(16, 8);
        let mut out = vec![127u8];
        out.extend_from_slice(&row);
        out.push(128);
        out
    };
    let mut smasked = case(
        "smask",
        "/Width 16 /Height 16 /ColorSpace /DeviceRGB /BitsPerComponent 8",
        detailed_rgb(16, 16),
        16,
        16,
        "raw",
        "rgb",
        8,
    );
    smasked.smask = Some((
        "/Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 8".to_string(),
        (0..64u8).map(|i| i * 4).collect(),
    ));
    vec![
        case("rgb", "/Width 16 /Height 12 /ColorSpace /DeviceRGB /BitsPerComponent 8", detailed_rgb(16, 12), 16, 12, "raw", "rgb", 8),
        case("flate gray", "/Width 20 /Height 10 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", flate(&gray(20, 10)), 20, 10, "raw", "gray", 8),
        case("run length", "/Width 16 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /RunLengthDecode", run_length, 16, 8, "raw", "gray", 8),
        case("ascii hex", "/Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /ASCIIHexDecode", gray(8, 8).iter().map(|b| format!("{b:02x}")).collect::<String>().into_bytes(), 8, 8, "raw", "gray", 8),
        case("two bit gray", "/Width 16 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 2", (0..32u8).map(|i| i.wrapping_mul(37)).collect(), 16, 8, "raw", "gray", 2),
        case("indexed", &format!("/Width 16 /Height 8 /ColorSpace [/Indexed /DeviceRGB 15 <{palette}>] /BitsPerComponent 4"), (0..64u8).map(|i| i.wrapping_mul(53)).collect(), 16, 8, "raw", "rgb", 4),
        case("sixteen bit gray", "/Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 16", (0..128u8).map(|i| i.wrapping_mul(29)).collect(), 8, 8, "raw", "gray", 16),
        case("cmyk", "/Width 8 /Height 8 /ColorSpace /DeviceCMYK /BitsPerComponent 8", (0..256u32).map(|i| (i * 11 % 256) as u8).collect(), 8, 8, "raw", "cmyk", 8),
        case("ccitt", "/Width 64 /Height 48 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /CCITTFaxDecode /DecodeParms << /K -1 /Columns 64 /Rows 48 >>", group4(&picture), 64, 48, "ccitt", "bilevel", 1),
        case("jbig2", "/Width 64 /Height 48 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /JBIG2Decode", jbig2(&picture), 64, 48, "jbig2", "bilevel", 1),
        case("stencil", "/Width 64 /Height 48 /ImageMask true /BitsPerComponent 1", packed_bits(&picture, false), 64, 48, "raw", "bilevel", 1),
        case("dct", "/Width 24 /Height 24 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode", jpeg, 24, 24, "jpeg", "rgb", 8),
        case("jpx", &format!("/Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /JPXDecode", decoded.width, decoded.height), jp2.to_vec(), decoded.width, decoded.height, "jpx", "gray", 8),
        smasked,
        case("colour key", "/Width 16 /Height 16 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Mask [0 80 0 255 0 255]", detailed_rgb(16, 16), 16, 16, "raw", "rgb", 8),
    ]
}

/// The PDF: one page that draws every case, in order.
fn pdf(cases: &[Case]) -> Vec<u8> {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
    ];
    let mut content = String::new();
    let mut names = String::new();
    let mut extra = Vec::new();
    let mut number = 5;
    for (i, case) in cases.iter().enumerate() {
        content.push_str(&format!("q 10 0 0 10 {} {} cm /Im{i} Do Q\n", 10 + i % 8 * 20, 10 + i / 8 * 20));
        names.push_str(&format!("/Im{i} {number} 0 R "));
        let smask = case.smask.as_ref().map(|_| format!("/SMask {} 0 R", number + 1)).unwrap_or_default();
        extra.push(stream_object(
            number,
            &format!("/Type /XObject /Subtype /Image {} {smask}", case.dict),
            &case.data,
        ));
        number += 1;
        if let Some((dict, data)) = &case.smask {
            extra.push(stream_object(number, &format!("/Type /XObject /Subtype /Image {dict}"), data));
            number += 1;
        }
    }
    objects.push(
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R \
             /Resources << /XObject << {names}>> >> >>\nendobj\n"
        )
        .into_bytes(),
    );
    objects.push(stream_object(4, "", content.as_bytes()));
    objects.extend(extra);
    pdf_from_objects(&objects)
}

/// One image as the wire delivered it.
struct Item {
    page: i32,
    index: i32,
    width: i32,
    height: i32,
    bits: i32,
    encoding: String,
    color: String,
    frames: Vec<usize>,
    bytes: Vec<u8>,
}

/// Parses what `extract_images_streamed` wrote: items of header, data frames
/// and an end frame, then the zero-length terminator.
fn read_wire(wire: &[u8]) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let mut open = false;
    let mut at = 0;
    loop {
        let len = u32::from_le_bytes(wire[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        if len == 0 {
            assert_eq!(at, wire.len(), "nothing follows the terminator");
            assert!(!open, "the terminator came inside an image");
            return items;
        }
        let frame = &wire[at..at + len];
        at += len;
        assert_eq!(frame[0], 1, "an ok response");
        let mut as_request = vec![0u8];
        as_request.extend_from_slice(&frame[1..]);
        let fields = Request::parse(&as_request).expect("a frame parses");
        match fields.get_str("kind").expect("a frame has a kind") {
            "header" => {
                assert!(!open, "a header inside an image");
                open = true;
                items.push(Item {
                    page: fields.get_i32("page").unwrap(),
                    index: fields.get_i32("index").unwrap(),
                    width: fields.get_i32("width").unwrap(),
                    height: fields.get_i32("height").unwrap(),
                    bits: fields.get_i32("bitsPerComponent").unwrap(),
                    encoding: fields.get_str("encoding").unwrap().to_string(),
                    color: fields.get_str("color").unwrap().to_string(),
                    frames: Vec::new(),
                    bytes: Vec::new(),
                });
            },
            "data" => {
                assert!(open, "data outside an image");
                let data = fields.get_bytes("data").unwrap();
                let item = items.last_mut().unwrap();
                item.frames.push(data.len());
                item.bytes.extend_from_slice(data);
            },
            "end" => {
                assert!(open, "an end outside an image");
                open = false;
            },
            other => panic!("unknown frame kind {other}"),
        }
    }
}

fn extracted(doc: &PdfDocument) -> Vec<Item> {
    let mut out = Vec::new();
    extract_images_streamed(doc, &[0], &mut out, &CancelCheck::none()).expect("extracts");
    read_wire(&out)
}

/// Width, height, colour type and the 8-bit samples of a PNG.
fn decode(png: &[u8]) -> (u32, u32, png::ColorType, Vec<u8>) {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).expect("a PNG frame");
    buf.truncate(info.buffer_size());
    (info.width, info.height, info.color_type, buf)
}

fn rgba_of(color: png::ColorType, samples: &[u8]) -> Vec<u8> {
    match color {
        png::ColorType::Grayscale => samples.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Rgb => samples.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Rgba => samples.to_vec(),
        other => panic!("unexpected PNG colour type {other:?}"),
    }
}

/// Every image is described before its bytes, in the order drawn, with what
/// it is stored as and what its samples mean.
#[test]
fn every_image_is_described_in_the_order_it_is_drawn() {
    let cases = cases();
    let doc = PdfDocument::from_bytes(pdf(&cases)).expect("fixture parses");
    let items = extracted(&doc);
    assert_eq!(items.len(), cases.len());
    for (i, (item, case)) in items.iter().zip(&cases).enumerate() {
        assert_eq!((item.page, item.index), (0, i as i32), "{}", case.name);
        assert_eq!((item.width, item.height), (case.width as i32, case.height as i32), "{}", case.name);
        assert_eq!(item.encoding, case.encoding, "{}", case.name);
        assert_eq!(item.color, case.color, "{}", case.name);
        assert_eq!(item.bits, case.bits, "{}", case.name);
        assert!(!item.bytes.is_empty(), "{} has bytes", case.name);
    }
}

/// A JPEG and a JPEG 2000 stream arrive as the file stores them.
#[test]
fn a_jpeg_and_a_jpx_stream_arrive_byte_for_byte() {
    let cases = cases();
    let doc = PdfDocument::from_bytes(pdf(&cases)).expect("fixture parses");
    let items = extracted(&doc);
    for (item, case) in items.iter().zip(&cases) {
        if case.encoding == "jpeg" || case.encoding == "jpx" {
            assert_eq!(item.bytes, case.data, "{} is the stored stream", case.name);
        }
    }
}

/// Every other image is a PNG, and its pixels are those the extractor that
/// decoded each image whole gives: `PdfImageHandle::decode`.
#[test]
fn a_png_has_the_pixels_the_whole_image_decoder_gave() {
    let cases = cases();
    let doc = PdfDocument::from_bytes(pdf(&cases)).expect("fixture parses");
    let items = extracted(&doc);
    let handles = doc.page_image_handles(0).expect("handles");
    for ((item, case), handle) in items.iter().zip(&cases).zip(&handles) {
        if case.encoding == "jpeg" || case.encoding == "jpx" {
            continue;
        }
        let (w, h, color, samples) = decode(&item.bytes);
        assert_eq!((w, h), (case.width, case.height), "{}", case.name);
        let rgba = rgba_of(color, &samples);
        if case.name == "stencil" {
            // The old extractor has no path for a stencil: its samples are
            // read here. A 0 sample marks the page, which is black.
            assert_eq!(color, png::ColorType::Grayscale, "a stencil is 1-bit gray");
            let packed = packed_bits(&bilevel(64, 48), false);
            for y in 0..48usize {
                for x in 0..64usize {
                    let bit = (packed[y * 8 + x / 8] >> (7 - x % 8)) & 1;
                    assert_eq!(rgba[(y * 64 + x) * 4], if bit == 0 { 0 } else { 255 }, "stencil ({x},{y})");
                }
            }
            continue;
        }
        let old = handle.decode().expect("the whole-image decoder").to_dynamic_image().expect("pixels").to_rgba8();
        assert_eq!(old.dimensions(), (w, h), "{}", case.name);
        for (i, (new, old)) in rgba.chunks_exact(4).zip(old.chunks_exact(4)).enumerate() {
            assert_eq!(&new[..3], &old[..3], "{}: pixel {i}", case.name);
        }
        // A mask arrives as the alpha channel; nothing else has one.
        match case.name {
            "smask" => {
                assert_eq!(color, png::ColorType::Rgba);
                assert!(rgba.chunks_exact(4).any(|p| p[3] == 0) && rgba.chunks_exact(4).any(|p| p[3] > 200));
            },
            "colour key" => {
                assert_eq!(color, png::ColorType::Rgba);
                assert!(rgba.chunks_exact(4).any(|p| p[3] == 0));
            },
            _ => assert_ne!(color, png::ColorType::Rgba, "{} has no mask", case.name),
        }
    }
}

/// The data frames carry at most 256 KiB and only the last is short.
#[test]
fn data_frames_are_at_most_256_kib_and_only_the_last_is_short() {
    let width = 1024u32;
    let height = 640u32;
    let mut noise = 0x1234_5678u32;
    let data: Vec<u8> = (0..width * height * 3)
        .map(|_| {
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            noise as u8
        })
        .collect();
    let case = Case {
        name: "noise",
        dict: format!("/Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8"),
        data,
        width,
        height,
        encoding: "raw",
        color: "rgb",
        bits: 8,
        smask: None,
    };
    let doc = PdfDocument::from_bytes(pdf(std::slice::from_ref(&case))).expect("fixture parses");
    let items = extracted(&doc);
    let frames = &items[0].frames;
    assert!(frames.len() > 2, "noise does not compress into one frame");
    assert!(frames[..frames.len() - 1].iter().all(|&n| n == DATA_FRAME_BYTES));
    assert!(*frames.last().unwrap() <= DATA_FRAME_BYTES);
}

/// A writer that fails ends the extraction with its error.
#[test]
fn a_failing_writer_ends_the_extraction_with_an_io_error() {
    struct Failing;
    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("the sink refused the bytes"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let doc = PdfDocument::from_bytes(pdf(&cases())).expect("fixture parses");
    let result = extract_images_streamed(&doc, &[0], &mut Failing, &CancelCheck::none());
    assert!(matches!(result, Err(crate::error::Error::Io(_))), "{result:?}");
}

/// The page itself is not an image: the stream objects are untouched by it.
#[test]
fn the_source_streams_are_the_ones_the_page_names() {
    let cases = cases();
    let doc = PdfDocument::from_bytes(pdf(&cases)).expect("fixture parses");
    let handles = doc.page_image_handles(0).expect("handles");
    assert_eq!(handles.len(), cases.len());
    let dct = handles[11].object_ref().expect("an XObject");
    let Object::Stream { body, .. } = doc.load_object(dct).expect("loads") else {
        panic!("a stream");
    };
    assert_eq!(body.read_whole_raw().expect("reads"), cases[11].data);
}
