//! extractImages: the images of some pages, each written to the wire as it
//! is read, never decoded whole.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! The images of a page come from `PdfDocument::page_image_handles`, which
//! reads the page's content and the images' dictionaries and decodes nothing.
//! Each one is delivered by what it is stored as:
//!
//! - **A JPEG (`DCTDecode`) or JPEG 2000 (`JPXDecode`) stream** goes through
//!   byte for byte: the stored codestream, decrypted and with any filter
//!   before the codec undone, never decoded. Its soft mask, `/Mask` and
//!   `/Decode` are the caller's to read from the file; they are not applied.
//! - **Every other image** is written as a PNG file, its rows read from the
//!   image cursor (`host::images::stream`) and encoded as they come: a raw
//!   or Flate, LZW or run-length stream, a CCITT stream by line, a JBIG2
//!   bitmap kept at one bit a pixel. The PNG is 1-bit gray for a stencil mask
//!   and for a 1-bit gray image, 8-bit gray for gray, 8-bit RGB for RGB, CMYK
//!   and every other colour space (converted as the renderer converts), and
//!   8-bit RGBA when the image has an `/SMask` or a `/Mask` stream, which is
//!   applied as the alpha channel: a picture extracted without it would show
//!   pixels the page never shows, and a caller cannot get the mask back
//!   from a file that has no room for it. A colour-key `/Mask` array is
//!   applied as alpha too. A 16-bit image is written at 8 bits.
//!
//! The wire is the chunked framing of `host::chunked`, one item per image,
//! whose header is `{ kind = "header", page, index, width, height,
//! bitsPerComponent, encoding, color }`: the descriptor, before any byte.
//!
//! An image narrower or shorter than 8 pixels (a glyph fragment) is not
//! listed, nor is one whose filter the engine does not know: they never
//! were.

use std::io::{Read, Write};

use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::error::{Error, Result};
use crate::extractors::images::{ColorSpace, PdfFilter, PdfImageHandle};
use crate::host::chunked::{png_error, Encoded, Framer, PNG_CHUNK_BYTES};
use crate::host::images::stream::{ImageCursor, ImageRows};
use crate::object::Object;

/// Images narrower or shorter than this are not extracted.
const MIN_SIDE: u32 = 8;

/// How an image is stored, by the name the wire carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Raw,
    Jpeg,
    Jpx,
    Ccitt,
    Jbig2,
}

impl Encoding {
    fn name(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Jpeg => "jpeg",
            Self::Jpx => "jpx",
            Self::Ccitt => "ccitt",
            Self::Jbig2 => "jbig2",
        }
    }
}

/// What an image's samples mean, by the name the wire carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Bilevel,
    Gray,
    Rgb,
    Cmyk,
    Other,
}

impl Color {
    fn name(self) -> &'static str {
        match self {
            Self::Bilevel => "bilevel",
            Self::Gray => "gray",
            Self::Rgb => "rgb",
            Self::Cmyk => "cmyk",
            Self::Other => "other",
        }
    }

    fn of(space: ColorSpace, indexed_base: Option<ColorSpace>) -> Self {
        match space {
            ColorSpace::DeviceGray | ColorSpace::CalGray => Self::Gray,
            ColorSpace::DeviceRGB | ColorSpace::CalRGB => Self::Rgb,
            ColorSpace::DeviceCMYK => Self::Cmyk,
            ColorSpace::ICCBased(1) => Self::Gray,
            ColorSpace::ICCBased(3) => Self::Rgb,
            ColorSpace::ICCBased(4) => Self::Cmyk,
            ColorSpace::Indexed => indexed_base.map_or(Self::Other, |base| Self::of(base, None)),
            _ => Self::Other,
        }
    }
}

/// An image on its way out: what the caller is told first, and the stream
/// the bytes come from.
struct Found {
    width: u32,
    height: u32,
    bits: u8,
    encoding: Encoding,
    color: Color,
    stencil: bool,
}

/// Writes the images drawn on `pages` to `writer` as chunked items, then the
/// terminator. Returns the number of images written.
pub fn extract_images_streamed<W: Write>(
    doc: &PdfDocument,
    pages: &[usize],
    writer: &mut W,
    cancel: &CancelCheck,
) -> Result<usize> {
    let mut framer = Framer::new(writer);
    let mut delivered = 0;
    cancel.each(pages.iter().copied(), |page| {
        let mut index = 0;
        for handle in doc.page_image_handles(page)? {
            cancel.check()?;
            let Some(found) = describe(&handle)? else {
                continue;
            };
            write_image(doc, &handle, &found, page, index, &mut framer)?;
            index += 1;
            delivered += 1;
        }
        Ok(())
    })?;
    framer.terminate()?;
    Ok(delivered)
}

/// What `handle` is, or `None` for an image that is not listed.
fn describe(handle: &PdfImageHandle<'_>) -> Result<Option<Found>> {
    if handle.width < MIN_SIDE || handle.height < MIN_SIDE {
        return Ok(None);
    }
    let encoding = match handle.filter_chain.last() {
        None
        | Some(
            PdfFilter::FlateDecode
            | PdfFilter::LZWDecode
            | PdfFilter::RunLengthDecode
            | PdfFilter::ASCIIHexDecode
            | PdfFilter::ASCII85Decode
            | PdfFilter::Crypt,
        ) => Encoding::Raw,
        Some(PdfFilter::DCTDecode) => Encoding::Jpeg,
        Some(PdfFilter::JPXDecode) => Encoding::Jpx,
        Some(PdfFilter::CCITTFaxDecode) => Encoding::Ccitt,
        Some(PdfFilter::JBIG2Decode) => Encoding::Jbig2,
        Some(PdfFilter::Other(_)) => return Ok(None),
    };
    let (object, _, _) = handle.stream_parts()?;
    let Object::Stream { dict, .. } = &object else {
        return Ok(None);
    };
    let stencil = matches!(dict.get("ImageMask"), Some(Object::Boolean(true)));
    let indexed = handle.color_space == ColorSpace::Indexed;
    let color = if stencil {
        Color::Bilevel
    } else {
        match Color::of(handle.color_space, handle.indexed_base()) {
            Color::Gray if handle.bits_per_component == 1 && !indexed => Color::Bilevel,
            other => other,
        }
    };
    Ok(Some(Found {
        width: handle.width,
        height: handle.height,
        bits: if stencil { 1 } else { handle.bits_per_component },
        encoding,
        color,
        stencil,
    }))
}

fn write_image<W: Write>(
    doc: &PdfDocument,
    handle: &PdfImageHandle<'_>,
    found: &Found,
    page: usize,
    index: usize,
    framer: &mut Framer<'_, W>,
) -> Result<()> {
    framer
        .header(|frame| {
            frame.put_i32("page", page as i32);
            frame.put_i32("index", index as i32);
            frame.put_i32("width", found.width as i32);
            frame.put_i32("height", found.height as i32);
            frame.put_i32("bitsPerComponent", i32::from(found.bits));
            frame.put_str("encoding", found.encoding.name());
            frame.put_str("color", found.color.name());
        })
        .map_err(Error::Io)?;
    let (object, _, color_spaces) = handle.stream_parts()?;
    match found.encoding {
        Encoding::Jpeg | Encoding::Jpx => pass_through(&object, framer)?,
        _ => write_png(doc, &object, color_spaces, found, framer)?,
    }
    framer.end().map_err(Error::Io)
}

/// The stored codec stream, byte for byte: the stream's filters before the
/// codec are undone, the codec's own are not.
fn pass_through<W: Write>(object: &Object, framer: &mut Framer<'_, W>) -> Result<()> {
    let Object::Stream { dict, body } = object else {
        return Err(Error::Image("image XObject is not a stream".to_string()));
    };
    let owned = body.clone();
    let mut reader =
        crate::decoders::stream::reader_in(dict, body.origin(), move || owned.owned_reader())?;
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(crate::encryption::stream::from_io(e)),
        };
        framer.push(&chunk[..n]).map_err(Error::Io)?;
    }
}

/// The image as a PNG file, its rows encoded as they are read.
fn write_png<W: Write>(
    doc: &PdfDocument,
    object: &Object,
    color_spaces: &std::collections::HashMap<String, Object>,
    found: &Found,
    framer: &mut Framer<'_, W>,
) -> Result<()> {
    let Object::Stream { dict, .. } = object else {
        return Err(Error::Image("image XObject is not a stream".to_string()));
    };
    // How a row is read and what it becomes: bilevel images (a stencil
    // included) are read as luma and written at one bit a pixel.
    enum Source {
        Luma(ImageRows),
        Cursor(ImageCursor),
    }
    let mut source = if found.stencil {
        Source::Luma(ImageRows::open_stencil(doc, object)?)
    } else {
        Source::Cursor(ImageCursor::open(
            doc,
            object,
            color_spaces,
            dict.get("SMask"),
            dict.get("Mask"),
            None,
        )?)
    };
    let (width, height) = match &source {
        Source::Luma(rows) => rows.size(),
        Source::Cursor(cursor) => cursor.size(),
    };
    // A mask stream is read in step with the image; a colour-key array
    // clears pixels of the rows themselves. Either is the alpha channel.
    let alpha = matches!(&source, Source::Cursor(c) if c.has_masks())
        || matches!(dict.get("Mask"), Some(Object::Array(_)));
    let (color, depth, samples) = if found.stencil {
        (png::ColorType::Grayscale, png::BitDepth::One, 1)
    } else if alpha {
        (png::ColorType::Rgba, png::BitDepth::Eight, 4)
    } else if found.color == Color::Bilevel {
        (png::ColorType::Grayscale, png::BitDepth::One, 1)
    } else if found.color == Color::Gray {
        (png::ColorType::Grayscale, png::BitDepth::Eight, 1)
    } else {
        (png::ColorType::Rgb, png::BitDepth::Eight, 3)
    };

    let encoded = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut encoder = png::Encoder::new(Encoded(std::rc::Rc::clone(&encoded)), width, height);
    encoder.set_color(color);
    encoder.set_depth(depth);
    encoder.set_compression(png::Compression::Balanced);
    encoder.set_filter(png::Filter::Adaptive);
    let mut png = encoder
        .write_header()
        .map_err(png_error)?
        .into_stream_writer_with_size(PNG_CHUNK_BYTES)
        .map_err(png_error)?;
    let drain = |framer: &mut Framer<'_, W>| -> Result<()> {
        let mut bytes = encoded.borrow_mut();
        let result = framer.push(&bytes).map_err(Error::Io);
        bytes.clear();
        result
    };
    drain(framer)?;

    let mut read = vec![0u8; width as usize * if matches!(source, Source::Luma(_)) { 1 } else { 4 }];
    let mut row = Vec::with_capacity(width as usize * samples);
    for _ in 0..height {
        crate::cancel::poll()?;
        row.clear();
        match &mut source {
            Source::Luma(rows) => {
                rows.read(doc, &mut read)?;
                // A stencil's paint is the page's ink: black.
                pack_bits(read.iter().map(|&g| g < 128), &mut row);
            },
            Source::Cursor(cursor) => {
                cursor.read_straight(doc, &mut read)?;
                match (color, depth) {
                    (_, png::BitDepth::One) => {
                        pack_bits(read.chunks_exact(4).map(|px| px[0] >= 128), &mut row)
                    },
                    (png::ColorType::Rgba, _) => row.extend_from_slice(&read),
                    (png::ColorType::Grayscale, _) => row.extend(read.chunks_exact(4).map(|px| px[0])),
                    _ => {
                        for px in read.chunks_exact(4) {
                            row.extend_from_slice(&px[..3]);
                        }
                    },
                }
            },
        }
        png.write_all(&row).map_err(Error::Io)?;
        drain(framer)?;
    }
    png.finish().map_err(png_error)?;
    drain(framer)
}

/// Packs one bit a pixel, the first pixel in the high bit, a row padded to a
/// whole byte with zeros.
fn pack_bits(bits: impl Iterator<Item = bool>, out: &mut Vec<u8>) {
    let mut byte = 0u8;
    let mut filled = 0u8;
    for bit in bits {
        byte = (byte << 1) | u8::from(bit);
        filled += 1;
        if filled == 8 {
            out.push(byte);
            byte = 0;
            filled = 0;
        }
    }
    if filled > 0 {
        out.push(byte << (8 - filled));
    }
}
