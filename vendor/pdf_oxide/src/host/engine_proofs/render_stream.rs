//! The chunked render wire carries the pixels the whole-buffer encoder
//! writes.
//!
//! The whole-buffer path (`rendering::render_page`, which the `image` crate
//! encodes) stays for library users; the host's render op never calls it. A
//! Dart battery sees only the bytes the host writes, so only here can they be
//! compared with the encoder they replaced.

use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::host::binary_codec::Request;
use crate::host::render_stream::{
    render_pages_streamed, RenderFormat, RenderSize, DATA_FRAME_BYTES,
};
use crate::rendering::{render_page, RenderOptions};
use std::io::Write;

/// Two pages of paths, text, a clip and partial coverage on 200 x 200 points.
fn pdf() -> Vec<u8> {
    let first = "0.9 0.2 0.2 rg 10.3 10.7 60.5 40.2 re f\n\
        0.2 0.4 0.9 rg 80 20 m 130 20 l 105 70 l f\n\
        0.1 0.7 0.3 rg 20 80 m 60 140 100 140 140 80 c 100 100 60 100 20 80 c f\n\
        q 100 20 m 150 20 180 50 180 100 c 180 150 150 180 100 180 c 50 180 20 150 20 100 c \
        20 50 50 20 100 20 c h W n 0 0 1 rg 0 90 200 20 re f Q\n\
        BT /F1 24 Tf 10 170 Td 0 g (Streamed Rows) Tj ET\n\
        BT /F1 36 Tf 0.866 0.5 -0.5 0.866 40 40 Tm 0.1 0.2 0.7 rg (Rot) Tj ET\n";
    let second = "0 0.3 0.6 rg 99.5 0 1 200 re f 0.3 g 0 120.5 200 0.5 re f\n\
        BT /F1 12 Tf 10 100 Td 0 g (Second page) Tj ET\n";
    let page = |id: u32, contents: u32| {
        format!(
            "{id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents {contents} 0 R \
             /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>\nendobj\n"
        )
        .into_bytes()
    };
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>\nendobj\n".to_vec(),
        page(3, 4),
        stream_object(4, "", first.as_bytes()),
        page(5, 6),
        stream_object(6, "", second.as_bytes()),
    ])
}

/// One page as the wire delivered it.
struct WirePage {
    page: i32,
    width: i32,
    height: i32,
    format: String,
    /// Payload length of each data frame, in order.
    frames: Vec<usize>,
    bytes: Vec<u8>,
}

/// Parses what `render_pages_streamed` wrote: pages of header, data frames
/// and an end frame, then the zero-length terminator.
fn read_wire(wire: &[u8]) -> Vec<WirePage> {
    let mut pages: Vec<WirePage> = Vec::new();
    let mut open = false;
    let mut at = 0;
    loop {
        let len = u32::from_le_bytes(wire[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        if len == 0 {
            assert_eq!(at, wire.len(), "nothing follows the terminator");
            assert!(!open, "the terminator came inside a page");
            return pages;
        }
        // A response is a status byte, then fields laid out as a request's
        // after its (here empty) operation name.
        let frame = &wire[at..at + len];
        at += len;
        assert_eq!(frame[0], 1, "an ok response");
        let mut as_request = vec![0u8];
        as_request.extend_from_slice(&frame[1..]);
        let fields = Request::parse(&as_request).expect("a frame parses");
        match fields.get_str("kind").expect("a frame has a kind") {
            "header" => {
                assert!(!open, "a header inside a page");
                open = true;
                pages.push(WirePage {
                    page: fields.get_i32("page").unwrap(),
                    width: fields.get_i32("width").unwrap(),
                    height: fields.get_i32("height").unwrap(),
                    format: fields.get_str("format").unwrap().to_string(),
                    frames: Vec::new(),
                    bytes: Vec::new(),
                });
            },
            "data" => {
                assert!(open, "data outside a page");
                let data = fields.get_bytes("data").unwrap();
                let page = pages.last_mut().unwrap();
                page.frames.push(data.len());
                page.bytes.extend_from_slice(data);
            },
            "end" => {
                assert!(open, "an end outside a page");
                open = false;
            },
            other => panic!("unknown frame kind {other}"),
        }
    }
}

fn wire(doc: &PdfDocument, size: RenderSize, format: RenderFormat) -> Vec<WirePage> {
    let mut out = Vec::new();
    render_pages_streamed(doc, &[0, 1], size, format, &mut out, &CancelCheck::none())
        .expect("renders");
    read_wire(&out)
}

/// Width, height, colour type and raw bytes of a PNG.
fn decode(png: &[u8]) -> (u32, u32, png::ColorType, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .expect("a PNG");
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).expect("a PNG frame");
    buf.truncate(info.buffer_size());
    (info.width, info.height, info.color_type, buf)
}

/// The whole-buffer encoder's PNG of `page`, the bytes the host used to send.
fn whole(doc: &PdfDocument, page: usize, dpi: u32) -> Vec<u8> {
    render_page(doc, page, &RenderOptions::with_dpi(dpi))
        .expect("renders")
        .data
}

/// A Dart battery sees the bytes the host writes but not the whole-buffer
/// encoder they replace, so only here can the two be compared.
#[test]
fn the_png_rows_decode_to_the_pixels_of_the_whole_buffer_encoder() {
    let doc = PdfDocument::from_bytes(pdf()).expect("parses");
    for dpi in [36, 72, 150, 300] {
        let streamed = wire(&doc, RenderSize::Dpi(dpi), RenderFormat::Png);
        assert_eq!(streamed.len(), 2);
        for (index, page) in streamed.iter().enumerate() {
            assert_eq!(page.page, index as i32);
            assert_eq!(page.format, "png");
            let (w, h, color, pixels) = decode(&page.bytes);
            let (rw, rh, rcolor, reference) = decode(&whole(&doc, index, dpi));
            assert_eq!((w, h), (rw, rh), "{dpi} dpi page {index}: size");
            assert_eq!((page.width as u32, page.height as u32), (w, h));
            assert_eq!(color, rcolor);
            assert!(
                pixels == reference,
                "{dpi} dpi page {index}: the streamed PNG decodes to other pixels"
            );
            // The pixels are the contract; whether the two files are also
            // byte for byte the same is reported, not asserted.
            println!(
                "{dpi} dpi page {index}: {} bytes streamed, {} whole, identical bytes: {}",
                page.bytes.len(),
                whole(&doc, index, dpi).len(),
                page.bytes == whole(&doc, index, dpi)
            );
        }
    }
}

/// The raw format is a wire shape only the host writes; the PNG it is
/// compared with is a different frame sequence, not observable from Dart's
/// whole-buffer side.
#[test]
fn the_rgba_rows_are_the_png_pixels_and_the_header_names_the_format() {
    let doc = PdfDocument::from_bytes(pdf()).expect("parses");
    let png = wire(&doc, RenderSize::Dpi(150), RenderFormat::Png);
    let rgba = wire(&doc, RenderSize::Dpi(150), RenderFormat::Rgba);
    for (png, rgba) in png.iter().zip(&rgba) {
        assert_eq!(rgba.format, "rgba");
        assert_eq!((rgba.width, rgba.height), (png.width, png.height));
        let (_, _, _, pixels) = decode(&png.bytes);
        assert_eq!(rgba.bytes.len(), rgba.width as usize * rgba.height as usize * 4);
        assert!(rgba.bytes == pixels, "rgba rows differ from the PNG's pixels");
    }
}

/// The frame cut is the host's own; a Dart sink sees chunks, not frames.
#[test]
fn data_frames_are_at_most_256_kib_and_only_the_last_is_short() {
    let doc = PdfDocument::from_bytes(pdf()).expect("parses");
    let pages = wire(&doc, RenderSize::Dpi(300), RenderFormat::Rgba);
    for page in &pages {
        assert!(page.frames.len() > 1, "a 300 dpi raster spans several frames");
        let (last, full) = page.frames.split_last().unwrap();
        assert!(full.iter().all(|&len| len == DATA_FRAME_BYTES));
        assert!((1..=DATA_FRAME_BYTES).contains(last));
        assert_eq!(page.frames.iter().sum::<usize>(), page.bytes.len());
    }
}

/// Fit and thumbnail are computed in the engine from the page box.
#[test]
fn fit_and_thumbnail_size_the_page_in_the_engine() {
    let doc = PdfDocument::from_bytes(pdf()).expect("parses");
    let fit = wire(
        &doc,
        RenderSize::Fit {
            max_width: 100,
            max_height: 50,
        },
        RenderFormat::Rgba,
    );
    assert!(fit.iter().all(|p| (p.width, p.height) == (50, 50)));
    let thumb = wire(&doc, RenderSize::Thumbnail(80), RenderFormat::Rgba);
    assert!(thumb.iter().all(|p| (p.width, p.height) == (80, 80)));
}

/// A writer that fails mid-page is a Dart sink that threw: the render must
/// end with the writer's error and leave no end frame behind.
#[test]
fn a_failing_writer_ends_the_render_with_an_io_error() {
    struct Fails(usize);
    impl Write for Fails {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            if self.0 < data.len() {
                return Err(std::io::Error::other("sink refused"));
            }
            self.0 -= data.len();
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let doc = PdfDocument::from_bytes(pdf()).expect("parses");
    let err = render_pages_streamed(
        &doc,
        &[0, 1],
        RenderSize::Dpi(150),
        RenderFormat::Png,
        &mut Fails(2_000),
        &CancelCheck::none(),
    )
    .expect_err("the writer fails");
    assert!(matches!(err, crate::error::Error::Io(_)), "{err:?}");
}

/// A size of zero never reaches the rasteriser.
#[test]
fn a_zero_dpi_is_refused() {
    let doc = PdfDocument::from_bytes(pdf()).expect("parses");
    let err = render_pages_streamed(
        &doc,
        &[0],
        RenderSize::Dpi(0),
        RenderFormat::Png,
        &mut Vec::new(),
        &CancelCheck::none(),
    )
    .expect_err("refused");
    assert!(matches!(err, crate::error::Error::InvalidOperation(_)), "{err:?}");
}
