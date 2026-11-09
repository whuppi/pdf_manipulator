//! Image rows as readers: what a page render draws an image from.
//!
//! This file is fork-only code (NOT upstream).
//!
//! The renderer paints a page in bands, and an image reaches into every band
//! it overlaps. An [`ImageCursor`] holds one image's open decode across the
//! bands of a page, so that each stored byte is read about once and the
//! image never exists whole:
//!
//! - A raw-sample image (no image codec in its filters) is read through the
//!   stream's filter readers, a strip of rows at a time. A strip is turned
//!   into pixels by the same extractor a whole image goes through, with the
//!   strip standing in as a small image of its own, so every colour space,
//!   palette, `/Decode` and bit depth means exactly what it means there.
//! - A codec image (DCT, JPX, CCITT, JBIG2) is decoded by its codec, whose
//!   decoder returns a raster: the JPEG at the largest inverse-DCT reduction
//!   that still covers the footprint, the JPEG 2000 codestream at the
//!   resolution level that covers it, the fax and JBIG2 bitmaps at one bit a
//!   pixel. The cursor keeps that raster and reads rows from it.
//! - `/Mask` and `/SMask` images are cursors of their own, read in step with
//!   the rows they mask.
//!
//! Rows are asked for in order. [`ImageCursor::scaled_rows`] yields the rows
//! of the image resampled to a smaller size, through [`RowResampler`];
//! [`ImageCursor::region`] yields a rectangle of the full-size rows. A
//! request that lies behind the cursor starts the decode again from the first
//! row (a flipped image meets every band from the bottom).

use super::resample::RowResampler;
use crate::decoders::{CcittParams, ccitt::FaxRowEngine};
use crate::decoders::ccitt::BitFeed;
use crate::error::{Error, Result};
use crate::extractors::images::{
    decode_jbig2_bits, row_geometry, ImageData, PdfImage,
};
use crate::object::{Object, ObjectRef};
use image::DynamicImage;
use std::collections::{HashMap, VecDeque};
use std::io::Read;

/// Rows a strip of a raw-sample image holds, to about this many bytes of
/// pixels.
const STRIP_BYTES: usize = 256 * 1024;

/// Bytes of finished rows an image cursor keeps behind its position, for the
/// margin rows of the next band's region.
const TAIL_BYTES: usize = 256 * 1024;

/// What a row holds per pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Output {
    /// Straight RGBA8.
    Rgba,
    /// One luma byte, for a mask.
    Luma,
}

impl Output {
    fn bytes(self) -> usize {
        match self {
            Output::Rgba => 4,
            Output::Luma => 1,
        }
    }
}

/// Where an image's rows are held while it is read.
enum Kind {
    /// A raw-sample stream read through its filters, a strip at a time.
    Strips(Strips),
    /// The raster a codec decoded, kept as the image crate holds it.
    Raster(DynamicImage),
    /// A bilevel bitmap, packed MSB first, rows padded to a byte. `one` is the
    /// gray value (0 or 255) a set bit stands for; a clear bit is the other.
    Bits { packed: Vec<u8>, row_bytes: usize, one: u8 },
    /// A fax image's rows, decoded a row at a time from the stream: two
    /// transition lines and one row are all the decode holds.
    Fax(Fax),
    /// A stencil's sample bits, read off the stream's filter readers one
    /// row at a time (the window the filters hold plus the row).
    Stencil(Stencil),
}

struct Strips {
    /// The stream's dictionary without its filters, which the strips'
    /// readers apply.
    dict: HashMap<String, Object>,
    /// The stream's dictionary as stored, to open its readers from.
    filtered: HashMap<String, Object>,
    body: crate::object::StreamBody,
    origin: Option<crate::decoders::stream::Origin>,
    color_spaces: HashMap<String, Object>,
    row_bytes: usize,
    strip_rows: u32,
    reader: Box<dyn Read>,
    /// Pixels of the rows `first..first + count`.
    pixels: Vec<u8>,
    first: u32,
    count: u32,
    /// A colour-key `/Mask`: per component sample ranges that are clear.
    colour_key: Option<Vec<(u32, u32)>>,
}

impl Strips {
    fn open_reader(
        dict: &HashMap<String, Object>,
        body: &crate::object::StreamBody,
        origin: Option<crate::decoders::stream::Origin>,
    ) -> Result<Box<dyn Read>> {
        let body = body.clone();
        crate::decoders::stream::reader_in(dict, origin, move || body.owned_reader())
    }
}

/// A fax image as the gray raster the whole-image extraction stores (an inked
/// pixel 0, a white one 255), `width` x `height`, read a row at a time from
/// the stream's reader: the codestream is never held. A row past the stream,
/// and the pixels of a row past `width`, are white.
#[cfg(feature = "rendering")]
pub(crate) fn fax_gray(
    xobject: &Object,
    params: CcittParams,
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let Object::Stream { dict, body } = xobject else {
        return Err(Error::Image("image XObject is not a stream".to_string()));
    };
    let columns = params.columns;
    let served = params.rows.map_or(height, |rows| rows.min(height));
    let mut fax = Fax::open(dict, body, params, served)?;
    let total = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| Error::Image("fax raster size overflows".to_string()))?;
    let mut gray = Vec::new();
    gray.try_reserve_exact(total)
        .map_err(|_| Error::Image(format!("Unable to allocate {total} fax bytes")))?;
    let mut row = vec![0u8; columns as usize];
    for y in 0..height {
        crate::cancel::poll()?;
        if y < served {
            fax.read(&mut row, columns, Output::Luma)?;
            let kept = (width as usize).min(row.len());
            gray.extend_from_slice(&row[..kept]);
            gray.resize(gray.len() + (width as usize - kept), 255);
        } else {
            gray.resize(gray.len() + width as usize, 255);
        }
    }
    Ok(gray)
}

/// A fax image's rows, decoded a row at a time from the stream's reader.
struct Fax {
    /// The stream's dictionary as stored, to open its reader again from.
    dict: HashMap<String, Object>,
    body: crate::object::StreamBody,
    /// The decode params, `/Decode`'s inversion already folded into
    /// `black_is_1`: a decoded row's `1` bit is the sample the whole decode
    /// reports (1 = ink).
    params: CcittParams,
    /// The row count the reader will ask for: the smaller of `/Rows` and
    /// `/Height`, which the engine serves white past the stream's end.
    height: u32,
    row_bytes: usize,
    engine: FaxRowEngine,
    feed: BitFeed,
    /// The row last decoded, reused across reads. White once the stream has
    /// failed and every remaining row is the blank stand-in.
    row: Vec<u8>,
    failed: bool,
    /// Set when the stencil's `/Decode` `[1 0]` puts the sample the mapping
    /// paints where the engine's bit is clear: the sample a row carries is
    /// then one flip from the decoded bit. Stencil readers only; the image
    /// and mask rows keep the engine's own polarity.
    reversed: bool,}

impl Fax {
    /// Opens the fax rows of the stream `dict` names: the reader its filters
    /// leave and the CCITT params `params` holds (the caller pulled them from
    /// the dictionary and its colour-space map exactly as the whole decode
    /// does). `height` is the image's `/Height`.
    fn open(
        dict: &HashMap<String, Object>,
        body: &crate::object::StreamBody,
        params: CcittParams,
        height: u32,
    ) -> Result<Self> {
        // The cursor's engine reads a u16 width; a wider fax image is an
        // error, and the caller skips that image with a warning.
        let width = params.columns;
        if width > u16::MAX as u32 {
            return Err(Error::Image(format!(
                "CCITT image /Width {width} exceeds the row cursor's limit {}",
                u16::MAX
            )));
        }
        let reader = Strips::open_reader(dict, body, None)?;
        Ok(Self {
            dict: dict.clone(),
            body: body.clone(),
            height,
            row_bytes: (width as usize).div_ceil(8),
            engine: FaxRowEngine::new(&params)?,
            feed: BitFeed::new(reader),
            params,
            row: Vec::new(),
            failed: false,
            reversed: false,
        })
    }

    /// Decodes the next row and expands it into `out`: `width` pixels of
    /// `output`, a set bit inked, exactly as the `Bits` kind reads one.
    /// A stream the engine gives up on leaves the rows white, the blank
    /// stand-in the whole decode makes of such a stream. The reader stops at
    /// `min(/Rows, /Height)`; the rows past that are white as well.
    fn read(&mut self, out: &mut [u8], width: u32, output: Output) -> Result<()> {
        if self.row.len() != self.row_bytes {
            self.row.clear();
            self.row.resize(self.row_bytes, 0);
        }
        let mut failed_now = false;
        if !self.failed {
            match self.engine.pull_row(&mut self.feed, Some(self.height), &mut self.row) {
                Ok(_) => {},
                Err(e) => {
                    log::warn!(
                        "CCITT row cursor gave up ({}x{}, K={}): {e}",
                        self.params.columns,
                        self.height,
                        self.params.k
                    );
                    self.failed = true;
                    failed_now = true;
                },
            }
        }
        if self.failed || failed_now {
            self.row.fill(0);
        }
        // The params' polarity read (and `/Decode` fold, if any) applied: a
        // set bit is ink, as the whole decode's rows carry.
        if self.params.black_is_1 {
            for b in &mut self.row {
                *b = !*b;
            }
        }
        bilevel_row(&self.row, width, 0, output, out);
        Ok(())
    }

    /// Decodes `rows` rows past the cursor without producing them.
    fn skip(&mut self, rows: u32) -> Result<()> {
        if self.row.len() != self.row_bytes {
            self.row.clear();
            self.row.resize(self.row_bytes, 0);
        }
        for _ in 0..rows {
            crate::cancel::poll()?;
            if self.failed {
                continue; // the rows past the failure are white already
            }
            match self.engine.pull_row(&mut self.feed, Some(self.height), &mut self.row) {
                Ok(_) => {},
                Err(e) => {
                    log::warn!(
                        "CCITT row cursor gave up skipping ({}x{}, K={}): {e}",
                        self.params.columns,
                        self.height,
                        self.params.k
                    );
                    self.failed = true;
                },
            }
        }
        Ok(())
    }

    /// Bytes this source holds in memory: the row and its window, plus the
    /// engine's two transition lines, one column each.
    fn retained(&self) -> usize {
        self.row.capacity()
            + self.feed.buf.capacity()
            + 2 * self.params.columns as usize * 2
    }

    /// Opens the stream's reader and the row decode again from the first row.
    fn restart(&mut self) -> Result<()> {
        let reader = Strips::open_reader(&self.dict, &self.body, None)?;
        self.feed = BitFeed::new(reader);
        self.engine = FaxRowEngine::new(&self.params)?;
        self.row.clear();
        self.failed = false;
        Ok(())
    }

    /// Reads the next row into `out` (`row_bytes` bytes, MSB first) as the
    /// stencil's own sample bits: a set bit is the sample the stencil's
    /// `/Decode` puts where the mapping paints. A row the engine cannot
    /// serve is the engine's white stand-in carried into the sample space,
    /// and so is every row after. A sample read and the luma read drive the
    /// same buffer through two different flips: one cursor serves one of
    /// them.
    fn read_samples(&mut self, out: &mut [u8]) -> Result<()> {
        if self.row.len() != self.row_bytes {
            self.row.clear();
            self.row.resize(self.row_bytes, 0);
        }
        let mut failed_now = false;
        let mut carried = false;
        if !self.failed {
            let decoded_before = self.engine.decoded_rows();
            match self.engine.pull_row(&mut self.feed, Some(self.height), &mut self.row) {
                Ok(_) => {},
                Err(e) => {
                    log::warn!(
                        "CCITT stencil sample rows gave up ({}x{}, K={}): {e}",
                        self.params.columns,
                        self.height,
                        self.params.k
                    );
                    self.failed = true;
                    failed_now = true;
                },
            }
            // A row the engine served without a real decode behind it (the
            // stream that gave out before it, the block that ended early)
            // is the white stand-in, not content: nothing more of this
            // stencil exists to read, and the rest is the sample no paint
            // under this mask's own /Decode reads.
            carried = self.engine.decoded_rows() == decoded_before;
            if carried && !failed_now {
                self.failed = true;
            }
        }
        if self.failed || failed_now || carried {
            // A row the cursor could not decode, or the engine's white
            // stand-in, wears the sample the mapping reads as nothing to
            // paint (the page as it would look with the stencil absent);
            // the flip below never applies to it.
            self.row.fill(if self.reversed { 0x00 } else { 0xFF });
        } else if self.reversed != self.params.black_is_1 {
            // The engine's bit to the sample the stencil's `/Decode` mapping
            // paints: inverted with the folded `black_is_1` and again with
            // the stencil's own, so the two folds cancel when both say the
            // same.
            for b in &mut self.row {
                *b = !*b;
            }
        }
        out.copy_from_slice(&self.row);
        Ok(())
    }
}

/// A raw or Flate-family stencil's sample bits, read off the stream's filter
/// readers a row at a time. A stencil has no colour space (`/ImageMask
/// true`), so the strip reader an image takes is not open: the filtered
/// bytes are the samples, `row_bytes` a row.
struct Stencil {
    /// The stream's dictionary as stored, to open its reader again from.
    dict: HashMap<String, Object>,
    body: crate::object::StreamBody,
    row_bytes: usize,
    /// The sample a row's padding carries on the luma read: the bit the
    /// mapping reads as the base image visible, which is what a `/Mask`
    /// stencil's missing samples stand for. A sample read pads with
    /// [`Stencil::no_paint`] instead.
    white_pad: u8,
    /// The gray value (0 or 255) a set sample bit stands for in `read`.
    one: u8,
    reader: Box<dyn Read>,
    /// The row last read from the reader, reused across reads. Its bytes
    /// past what `fill` returned are the caller's padding.
    row: Vec<u8>,
    failed: bool,
}

impl Stencil {
    /// Reads the next row's bytes from the reader into the row and returns
    /// how many the stream gave: `row_bytes` for a full row, fewer where the
    /// stream ends short or has failed. The bytes past that count are not
    /// the stream's, so the caller pads them.
    fn fill(&mut self) -> usize {
        if self.row.len() != self.row_bytes {
            self.row.clear();
            self.row.resize(self.row_bytes, 0);
        }
        if self.failed {
            return 0;
        }
        let mut filled = 0;
        while filled < self.row_bytes {
            match self.reader.read(&mut self.row[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {},
                Err(e) => {
                    log::warn!("stencil sample rows stopped early: {e}");
                    self.failed = true;
                    break;
                },
            }
        }
        filled
    }

    /// The sample bit under which no paint lands: the opposite of the bit
    /// the mapping paints, so a padded row shows nothing of the stencil.
    fn no_paint(&self) -> u8 {
        if self.one == 255 { 0x00 } else { 0xFF }
    }

    fn restart(&mut self) -> Result<()> {
        self.reader = Strips::open_reader(&self.dict, &self.body, None)?;
        self.row.clear();
        self.failed = false;
        Ok(())
    }

    /// Bytes this source holds: the row window and the body's bytes when
    /// they are the stream's own.
    fn retained(&self) -> usize {
        self.row.capacity() + self.body.inline().map_or(0, |bytes| bytes.len())
    }
}

/// One image's rows, read from the first to the last.
pub(crate) struct ImageRows {
    width: u32,
    height: u32,
    output: Output,
    /// The next row to read.
    next: u32,
    kind: Kind,
}

/// The `DynamicImage` in the form rows are cut from: the variants whose rows
/// convert without a whole-image pass are kept, anything else is converted
/// once the way the whole-image path converts it.
fn normalised(img: DynamicImage, output: Output) -> DynamicImage {
    match output {
        Output::Luma => match img {
            DynamicImage::ImageLuma8(_) => img,
            other => DynamicImage::ImageLuma8(other.to_luma8()),
        },
        Output::Rgba => match img {
            DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_) => img,
            other => DynamicImage::ImageRgba8(other.to_rgba8()),
        },
    }
}

/// Row `y` of `img` as `output` pixels, exactly as the image crate's
/// `to_rgba8` / `to_luma8` give it.
fn dynamic_row(img: &DynamicImage, y: u32, output: Output, out: &mut [u8]) {
    let w = img.width() as usize;
    let y = y as usize;
    match (img, output) {
        (DynamicImage::ImageLuma8(b), Output::Luma) => {
            out.copy_from_slice(&b.as_raw()[y * w..(y + 1) * w]);
        },
        (DynamicImage::ImageLuma8(b), Output::Rgba) => {
            for (g, px) in b.as_raw()[y * w..(y + 1) * w].iter().zip(out.chunks_exact_mut(4)) {
                px.copy_from_slice(&[*g, *g, *g, 255]);
            }
        },
        (DynamicImage::ImageLumaA8(b), Output::Rgba) => {
            for (ga, px) in b.as_raw()[y * w * 2..(y + 1) * w * 2]
                .chunks_exact(2)
                .zip(out.chunks_exact_mut(4))
            {
                px.copy_from_slice(&[ga[0], ga[0], ga[0], ga[1]]);
            }
        },
        (DynamicImage::ImageRgb8(b), Output::Rgba) => {
            for (rgb, px) in b.as_raw()[y * w * 3..(y + 1) * w * 3]
                .chunks_exact(3)
                .zip(out.chunks_exact_mut(4))
            {
                px.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        },
        (DynamicImage::ImageRgba8(b), Output::Rgba) => {
            out.copy_from_slice(&b.as_raw()[y * w * 4..(y + 1) * w * 4]);
        },
        // `normalised` leaves no other pair.
        _ => out.fill(0),
    }
}

/// One packed bilevel row as `output` pixels: `width` samples, MSB first, `one`
/// the gray value a set bit stands for, `255 - one` a clear bit. What the
/// `Bits` kind reads a row of its whole bitmap as, and what `Fax::read` reads
/// a decoded row as.
fn bilevel_row(row: &[u8], width: u32, one: u8, output: Output, out: &mut [u8]) {
    for (x, px) in out.chunks_exact_mut(output.bytes()).enumerate().take(width as usize) {
        let bit = (row[x / 8] >> (7 - (x % 8))) & 1;
        let gray = if bit == 1 { one } else { 255 - one };
        if output == Output::Rgba {
            px.copy_from_slice(&[gray, gray, gray, 255]);
        } else {
            px[0] = gray;
        }
    }
}

/// Zeroes the alpha of every pixel of `rgba` whose raw component samples all
/// fall in the colour-key `ranges` (ISO 32000-1 §8.9.6.4). `image` holds the
/// samples of the same rows.
///
/// Colour-key masking is defined against the raw pre-Decode samples. Those are
/// only recoverable from an 8-bit `ImageData::Raw` buffer whose per-pixel byte
/// count matches `ranges.len()`. For anything else (JPEG, non-8-bit depths, or
/// a palette-expanded Indexed image whose original indices are lost) the
/// ranges cannot be mapped onto the decoded pixels, so masking is skipped
/// rather than applied incorrectly.
fn apply_colour_key(image: &PdfImage, ranges: &[(u32, u32)], rgba: &mut [u8]) {
    let ncomp = ranges.len();
    let ImageData::Raw { pixels, format } = image.data() else {
        log::debug!("color-key /Mask: non-raw (e.g. JPEG) image, skipping");
        return;
    };
    if image.bits_per_component() != 8 || format.bytes_per_pixel() != ncomp {
        log::debug!(
            "color-key /Mask: unsupported layout (bpc={}, bpp={}, ncomp={}), skipping",
            image.bits_per_component(),
            format.bytes_per_pixel(),
            ncomp
        );
        return;
    }
    if pixels.len() < rgba.len() / 4 * ncomp {
        log::debug!("color-key /Mask: sample buffer too small, skipping");
        return;
    }
    for (samples, px) in pixels.chunks_exact(ncomp).zip(rgba.chunks_exact_mut(4)) {
        if crate::rendering::page_renderer::color_key_pixel_masked(samples, ranges) {
            px[3] = 0;
        }
    }
}

/// Straight RGBA8 to premultiplied, in place. `tiny_skia::Pixmap` stores
/// premultiplied alpha and `Pixmap::from_vec` converts nothing, so every
/// buffer handed to it must already satisfy `r,g,b <= a`; resampling is also
/// only right on premultiplied pixels, since straight alpha bleeds colour
/// across an alpha edge from pixels that contribute none.
pub(crate) fn premultiply_in_place(data: &mut [u8]) {
    let (pixels, _remainder) = data.as_chunks_mut::<4>();
    for px in pixels {
        let a = u32::from(px[3]);
        if a == 255 {
            continue;
        }
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            continue;
        }
        // Rounded, so a round trip through premultiply/unpremultiply does not
        // drift downward on every pass.
        px[0] = ((u32::from(px[0]) * a + 127) / 255) as u8;
        px[1] = ((u32::from(px[1]) * a + 127) / 255) as u8;
        px[2] = ((u32::from(px[2]) * a + 127) / 255) as u8;
    }
}

fn is_filter(filters: &[String], names: &[&str]) -> bool {
    filters
        .iter()
        .any(|f| names.iter().any(|n| f.eq_ignore_ascii_case(n)))
}

impl ImageRows {
    /// Opens the image stream `xobject` for reading as `output` rows.
    /// `target` is the size in device pixels it will be painted at: a codec
    /// that decodes in reduced form stops at the reduction that covers it.
    pub(crate) fn open(
        doc: &crate::document::PdfDocument,
        xobject: &Object,
        color_spaces: &HashMap<String, Object>,
        target: Option<(u32, u32)>,
        output: Output,
        colour_key: Option<&[Object]>,
    ) -> Result<Self> {
        use crate::extractors::images::extract_image_from_xobject_at;

        let Object::Stream { dict, body } = xobject else {
            return Err(Error::Image("XObject is not a stream".to_string()));
        };
        let filters = dict
            .get("Filter")
            .map(crate::object::extract_filter_names)
            .unwrap_or_default();

        if is_filter(&filters, &["JBIG2Decode"]) {
            let geometry = row_geometry(Some(doc), dict, Some(color_spaces))?;
            let packed =
                decode_jbig2_bits(xobject, dict, Some(doc), geometry.width, geometry.height, true)?;
            return Ok(Self {
                width: geometry.width,
                height: geometry.height,
                output,
                next: 0,
                kind: Kind::Bits {
                    packed,
                    row_bytes: (geometry.width as usize).div_ceil(8),
                    one: 255,
                },
            });
        }

        let is_codec = is_filter(&filters, &["JPXDecode", "CCITTFaxDecode", "DCTDecode"]);
        if !is_codec {
            let geometry = row_geometry(Some(doc), dict, Some(color_spaces))?;
            let filtered = dict.clone();
            let mut dict = dict.clone();
            // The strips are read through the filters, so the extractor sees
            // plain samples.
            dict.remove("Filter");
            dict.remove("DecodeParms");
            let origin = body.origin();
            let reader = Strips::open_reader(&filtered, body, origin.clone())?;
            let strip_rows = u32::try_from((STRIP_BYTES / (geometry.width as usize * 4).max(1)).max(1))
                .unwrap_or(1)
                .min(geometry.height);
            return Ok(Self {
                width: geometry.width,
                height: geometry.height,
                output,
                next: 0,
                kind: Kind::Strips(Strips {
                    dict,
                    filtered,
                    body: body.clone(),
                    origin,
                    color_spaces: color_spaces.clone(),
                    row_bytes: geometry.row_bytes,
                    strip_rows,
                    reader,
                    pixels: Vec::new(),
                    first: 0,
                    count: 0,
                    colour_key: colour_key.and_then(|ranges| {
                        let parsed = crate::rendering::page_renderer::parse_color_key_mask(
                            ranges,
                            geometry.components,
                        );
                        if parsed.is_none() {
                            log::debug!(
                                "Ignoring malformed color-key /Mask array (ncomp={})",
                                geometry.components
                            );
                        }
                        parsed
                    }),
                }),
            });
        }

        // A fax image is decoded a row at a time; the packed whole bitmap the
        // decode used to produce never exists.
        if is_filter(&filters, &["CCITTFaxDecode"]) {
            let geometry = row_geometry(Some(doc), dict, Some(color_spaces))?;
            let Some(params) = crate::extractors::images::fax_params(
                Some(doc),
                dict,
                Some(color_spaces),
                geometry.width,
                geometry.height,
            ) else {
                return Err(Error::Image(
                    "CCITT image is not 1-bit DeviceGray with readable /DecodeParms".to_string(),
                ));
            };
            // `/Columns` is the row the cursor packs and paints;
            // `/Rows` and `/Height` bound the rows it serves, at the
            // smaller.
            let width = params.columns;
            let height = params.rows.map_or(geometry.height, |rows| rows.min(geometry.height));
            let kind = Kind::Fax(Fax::open(dict, body, params, height)?);
            return Ok(Self { width, height, output, next: 0, kind });
        }

        let pdf_image = extract_image_from_xobject_at(Some(doc), xobject, None, Some(color_spaces), target)?;
        let mut raster = pdf_image.to_dynamic_image_covering(target)?;
        // A colour-key /Mask is tested against the raw samples, which a JPEG
        // 2000 decode still holds beside the picture. Where a sample is in
        // range the pixel is made clear in the raster itself, so that the
        // raw samples need not outlive the open.
        if is_filter(&filters, &["JPXDecode"]) && pdf_image.samples_are_raw() {
            let ranges = colour_key.and_then(|key| {
                crate::rendering::page_renderer::parse_color_key_mask(
                    key,
                    pdf_image.color_space().components(),
                )
            });
            if let Some(ranges) = ranges {
                let mut rgba = raster.to_rgba8();
                apply_colour_key(&pdf_image, &ranges, &mut rgba);
                raster = DynamicImage::ImageRgba8(rgba);
            }
        }
        let raster = normalised(raster, output);
        Ok(Self {
            width: raster.width(),
            height: raster.height(),
            output,
            next: 0,
            kind: Kind::Raster(raster),
        })
    }

    /// A stencil mask (`/ImageMask true`) that has no colour space, as luma
    /// rows: 255 where the base image shows through, 0 where it is masked out
    /// (ISO 32000-1 §8.9.6.2: under the default `/Decode`, sample 0 marks the
    /// page, which for an explicit mask means the base image shows).
    pub(crate) fn open_stencil(
        doc: &crate::document::PdfDocument,
        xobject: &Object,
    ) -> Result<Self> {
        let Object::Stream { dict, body } = xobject else {
            return Err(Error::Image("mask is not a stream".to_string()));
        };
        let is_image_mask = matches!(dict.get("ImageMask"), Some(Object::Boolean(true)));
        if !is_image_mask {
            return Err(Error::Image("mask is not an image mask".to_string()));
        }
        // `as u32` turns a negative /Width into a near-u32::MAX stride.
        let dimension = |key: &str| {
            dict.get(key)
                .and_then(|o| o.as_integer())
                .and_then(|v| u32::try_from(v).ok())
                .filter(|v| *v > 0)
                .ok_or_else(|| Error::Image(format!("mask /{key} must be positive")))
        };
        let (mw, mh) = (dimension("Width")?, dimension("Height")?);
        let row_bytes = (mw as usize).div_ceil(8);
        let filters = dict
            .get("Filter")
            .map(crate::object::extract_filter_names)
            .unwrap_or_default();
        let decode_reverses = dict
            .get("Decode")
            .and_then(|o| o.as_array())
            .and_then(|a| a.first())
            .map(|o| o.as_real().or_else(|| o.as_integer().map(|i| i as f64)).unwrap_or(0.0) >= 0.5)
            .unwrap_or(false);

        // The sample a row the source cannot serve stands for: the mapping
        // below reads the base image visible where no sample exists (the
        // padding is the bit that does, ISO 32000-1 §8.9.6.2).
        let white_pad = if decode_reverses { 0xFF } else { 0x00 };
        let kind = if is_filter(&filters, &["JBIG2Decode"]) {
            // The exception (d-122): the format composes its page bitmap
            // before any row could leave it, so its packed rows are held
            // whole. A mask the codec could not decode has no sample to
            // test: it reads as the base image visible, the same result an
            // absent /Mask gives.
            let packed =
                crate::extractors::images::decode_jbig2_stencil(xobject, dict, Some(doc), mw, mh)
                    .unwrap_or_default();
            let mut packed = packed;
            if packed.len() < row_bytes * mh as usize {
                packed.resize(row_bytes * mh as usize, white_pad);
            }
            Kind::Bits {
                packed,
                row_bytes,
                one: if decode_reverses { 255 } else { 0 },
            }
        } else if is_filter(&filters, &["CCITTFaxDecode"]) {
            // The stencil's codestream is decoded a row at a time off the
            // stream's reader, never held whole. The cursor's rows leave
            // with its `black_is_1` flip applied; the sample the stencil's
            // mapping reads sits at most one flip from the decoded bit, and
            // the read folds that flip in.
            let decode_parms = crate::extractors::images::resolved_decode_parms(
                Some(doc),
                dict.get("DecodeParms"),
            );
            let mut params =
                crate::object::extract_ccitt_params_with_width(decode_parms.as_ref(), Some(mw))
                    .unwrap_or(crate::decoders::CcittParams {
                        // The filter's default: Group 3, pure 1-D.
                        k: 0,
                        columns: mw,
                        ..Default::default()
                    });
            params.columns = mw;
            params.rows = Some(mh);
            // The read's flip lands a decoded bit on the sample the luma
            // mapping reads (a set sample bit is `one`, a clear one the
            // other): inverted with the engine's own `/BlackIs1` and with
            // the sample the stencil's `/Decode` puts where the mapping
            // paints, so the two folds cancel when both say the same.
            params.black_is_1 = decode_reverses == params.black_is_1;
            let mut fax = Fax::open(dict, body, params, mh)?;
            fax.reversed = decode_reverses;
            Kind::Fax(fax)
        } else {
            // Raw and Flate-family filters, through the stream's filter
            // readers (decryption on, predictor included) a row at a time.
            Kind::Stencil(Stencil {
                dict: dict.clone(),
                body: body.clone(),
                row_bytes,
                // Past the end of the stream, a /Mask read shows the base
                // image there (the luma pad); a sample read paints nothing
                // there (`Stencil::no_paint`).
                white_pad,
                one: if decode_reverses { 255 } else { 0 },
                reader: Strips::open_reader(dict, body, None)?,
                row: Vec::new(),
                failed: false,
            })
        };
        Ok(Self { width: mw, height: mh, output: Output::Luma, next: 0, kind })
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Bytes this source holds in memory.
    fn retained_bytes(&self) -> usize {
        match &self.kind {
            Kind::Strips(s) => {
                s.pixels.capacity() + s.body.inline().map_or(0, |bytes| bytes.len())
            },
            Kind::Raster(img) => img.as_bytes().len(),
            Kind::Bits { packed, .. } => packed.len(),
            Kind::Fax(fax) => fax.retained(),
            Kind::Stencil(stencil) => stencil.retained(),
        }
    }

    /// Whether the source has failed: a `Fax` or `Stencil` kind that cannot
    /// read a row from here on serves the padding instead.
    pub(crate) fn failed(&self) -> bool {
        match &self.kind {
            Kind::Fax(fax) => fax.failed,
            Kind::Stencil(stencil) => stencil.failed,
            _ => false,
        }
    }

    /// Starts again from the first row.
    fn restart(&mut self) -> Result<()> {
        self.next = 0;
        if let Kind::Strips(s) = &mut self.kind {
            s.reader = Strips::open_reader(&s.filtered, &s.body, s.origin.clone())?;
            s.pixels.clear();
            s.first = 0;
            s.count = 0;
        }
        if let Kind::Fax(fax) = &mut self.kind {
            fax.restart()?;
        }
        if let Kind::Stencil(stencil) = &mut self.kind {
            stencil.restart()?;
        }
        Ok(())
    }

    /// Passes over `rows` rows without producing them.
    fn skip(&mut self, rows: u32) -> Result<()> {
        let target = (self.next + rows).min(self.height);
        if let Kind::Strips(s) = &mut self.kind {
            let read_to = s.first + s.count;
            if target > read_to {
                let discard = u64::from(target - read_to) * s.row_bytes as u64;
                std::io::copy(&mut s.reader.by_ref().take(discard), &mut std::io::sink())
                    .map_err(crate::encryption::stream::from_io)?;
                s.pixels.clear();
                s.first = target;
                s.count = 0;
            }
        }
        if let Kind::Fax(fax) = &mut self.kind {
            fax.skip(target - self.next)?;
        }
        if let Kind::Stencil(stencil) = &mut self.kind {
            let rows = target - self.next;
            for _ in 0..rows {
                crate::cancel::poll()?;
                stencil.fill();
            }
        }
        self.next = target;
        Ok(())
    }

    /// Reads the next row into `out`, `width * output.bytes()` bytes.
    pub(crate) fn read(&mut self, doc: &crate::document::PdfDocument, out: &mut [u8]) -> Result<()> {
        let y = self.next;
        if y >= self.height {
            return Err(Error::Image("image has no more rows".to_string()));
        }
        let (width, output) = (self.width, self.output);
        match &mut self.kind {
            Kind::Raster(img) => dynamic_row(img, y, output, out),
            Kind::Bits { packed, row_bytes, one } => {
                let row = &packed[y as usize * *row_bytes..(y as usize + 1) * *row_bytes];
                bilevel_row(row, width, *one, output, out);
            },
            Kind::Fax(fax) => {
                // `Fax::read` decodes and expands one row, `black_is_1`
                // folded into the params it opens with.
                fax.read(out, width, output)?;
            },
            Kind::Stencil(stencil) => {
                let filled = stencil.fill();
                stencil.row[filled..].fill(stencil.white_pad);
                bilevel_row(&stencil.row, width, stencil.one, output, out);
            },
            Kind::Strips(s) => {
                if y >= s.first + s.count {
                    Self::fill(s, doc, y, width, self.height, output)?;
                }
                let stride = width as usize * output.bytes();
                let at = (y - s.first) as usize * stride;
                out.copy_from_slice(&s.pixels[at..at + stride]);
            },
        }
        self.next += 1;
        Ok(())
    }

    /// The sample bits of the padding a stencil's unreadable rows carry:
    /// none of them paints under the mask's own `/Decode`. `one` is 255
    /// exactly when `/Decode` `[1 0]` puts the painted sample where the bit
    /// is set, so the padding is the opposite bit.
    fn no_paint_sample_byte(&self) -> u8 {
        match &self.kind {
            Kind::Bits { one, .. } => {
                if *one == 255 { 0x00 } else { 0xFF }
            },
            Kind::Stencil(stencil) => stencil.no_paint(),
            Kind::Fax(fax) => {
                if fax.reversed { 0x00 } else { 0xFF }
            },
            _ => 0xFF,
        }
    }

    /// Reads the next row into `out` (`row_bytes` bytes, MSB first) as the
    /// stencil's own sample bits: a set bit is the sample the stencil's
    /// `/Decode` puts where the mapping paints. One cursor is read through
    /// `read` or through this, never both: the two carry the kind's
    /// personal buffer through different spaces.
    /// A `Kind::Bits` row is read as the samples it holds; a raw or
    /// Flate-family stencil's row is what its reader yields; a fax stencil
    /// carries the decoded bit to the sample space the `/Decode` names.
    pub(crate) fn read_samples(&mut self, out: &mut [u8]) -> Result<()> {
        let y = self.next;
        if y >= self.height {
            return Err(Error::Image("image has no more rows".to_string()));
        }
        match &mut self.kind {
            Kind::Bits { packed, row_bytes, .. } => {
                out.copy_from_slice(&packed[y as usize * *row_bytes..(y as usize + 1) * *row_bytes]);
            },
            Kind::Fax(fax) => fax.read_samples(out)?,
            Kind::Stencil(stencil) => {
                let pad = stencil.no_paint();
                let filled = stencil.fill();
                stencil.row[filled..].fill(pad);
                out.copy_from_slice(&stencil.row);
            },
            _ => return Err(Error::Image("not a stencil row source".to_string())),
        }
        self.next += 1;
        Ok(())
    }

    /// Decodes the strip that starts at row `first`.
    fn fill(
        s: &mut Strips,
        doc: &crate::document::PdfDocument,
        first: u32,
        width: u32,
        height: u32,
        output: Output,
    ) -> Result<()> {
        use crate::extractors::images::extract_image_from_xobject_at;

        let rows = s.strip_rows.min(height - first);
        let mut data = vec![0u8; rows as usize * s.row_bytes];
        // A stream shorter than its declared size is padded with zero
        // samples, the way the whole-image path pads it.
        let mut filled = 0;
        while filled < data.len() {
            match s.reader.read(&mut data[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {},
                Err(e) => return Err(crate::encryption::stream::from_io(e)),
            }
        }
        let mut dict = s.dict.clone();
        dict.insert("Height".to_string(), Object::Integer(i64::from(rows)));
        let strip = Object::stream(dict, data);
        let pdf_image =
            extract_image_from_xobject_at(Some(doc), &strip, None, Some(&s.color_spaces), None)?;
        let dynamic = pdf_image.to_dynamic_image()?;
        if dynamic.width() != width || dynamic.height() != rows {
            return Err(Error::Image(format!(
                "image strip is {}x{}, expected {}x{}",
                dynamic.width(),
                dynamic.height(),
                width,
                rows
            )));
        }
        let dynamic = normalised(dynamic, output);
        let stride = width as usize * output.bytes();
        s.pixels.clear();
        s.pixels.resize(rows as usize * stride, 0);
        for r in 0..rows {
            crate::cancel::poll()?;
            dynamic_row(&dynamic, r, output, &mut s.pixels[r as usize * stride..(r as usize + 1) * stride]);
        }
        if let (Some(ranges), Output::Rgba) = (&s.colour_key, output) {
            if pdf_image.samples_are_raw() {
                apply_colour_key(&pdf_image, ranges, &mut s.pixels);
            } else {
                // The extractor mapped a non-default /Decode into the stored
                // samples, so they are no longer in the space the /Mask
                // ranges are expressed in. Masking them here would hide the
                // wrong pixels.
                log::warn!(
                    "Skipping colour-key /Mask: the stored samples are no longer in the \
                     raw sample space the mask ranges are expressed in"
                );
            }
        }
        s.first = first;
        s.count = rows;
        Ok(())
    }
}

/// Premultiplied RGBA8 of the columns `x0..x1` of a packed 1-bit stencil's
/// rows, `rows` holding the mask's sample bits for exactly those rows (row
/// `r` is stencil row `y0 + r`, `row_bytes` bytes each) and nothing else.
/// `colour` where the stencil paints and clear elsewhere: a sample paints
/// where it is 0, or where it is 1 when `invert`. The row-range form of
/// [`stencil_region`], for the band a cursor serves instead of the mask's
/// whole bitmap.
pub(crate) fn stencil_region_rows(
    rows: &[u8],
    row_bytes: usize,
    invert: bool,
    colour: [u8; 4],
    x0: u32,
    x1: u32,
) -> Vec<u8> {
    let width = (x1 - x0) as usize;
    let height = rows.len() / row_bytes.max(1);
    let mut out = vec![0u8; width * height * 4];
    for (band_y, row) in rows.chunks_exact(row_bytes).enumerate() {
        for x in x0..x1 {
            let byte = row.get(x as usize / 8).copied().unwrap_or(0);
            let bit = (byte >> (7 - (x % 8))) & 1 == 1;
            if bit == invert {
                let at = (band_y * width + (x - x0) as usize) * 4;
                out[at..at + 4].copy_from_slice(&colour);
            }
        }
    }
    out
}

/// A stencil's rows reduced to its device footprint, read a row at a time
/// through the resampler the image path reduces its rows with.
struct StencilReduce {
    resampler: RowResampler,
    /// The destination row the next `coverage` call asks for first.
    next_dst: u32,
    /// The source row's sample bits, and its coverage as RGBA8.
    bits: Vec<u8>,
    cov: Vec<u8>,
}

/// The rows of one stencil band, as the band paints them.
pub(crate) enum StencilBand {
    /// Sample bits, `row_bytes` a row. A sample paints where its bit is
    /// `invert`.
    Bits { rows: Vec<u8>, row_bytes: usize, invert: bool },
    /// Premultiplied RGBA8 rows of a reduced stencil, `width` pixels each;
    /// the alpha is the coverage.
    Coverage { rgba: Vec<u8>, width: usize },
}

impl StencilBand {
    /// The band's pixels in `colour` where the stencil paints, for the
    /// columns `x0..x1`.
    pub(crate) fn paint(&self, colour: [u8; 4], x0: u32, x1: u32) -> Vec<u8> {
        match self {
            StencilBand::Bits { rows, row_bytes, invert } => {
                stencil_region_rows(rows, *row_bytes, *invert, colour, x0, x1)
            },
            StencilBand::Coverage { rgba, width } => {
                coverage_region_rows(rgba, *width, colour, x0, x1)
            },
        }
    }
}

/// The columns `x0..x1` of reduced stencil rows, `colour` scaled by each
/// pixel's coverage (its alpha).
fn coverage_region_rows(rgba: &[u8], width: usize, colour: [u8; 4], x0: u32, x1: u32) -> Vec<u8> {
    let out_w = (x1 - x0) as usize;
    let height = rgba.len() / (width * 4);
    let mut out = vec![0u8; out_w * height * 4];
    for (y, row) in rgba.chunks_exact(width * 4).enumerate() {
        for (x, px) in row[x0 as usize * 4..x1 as usize * 4].chunks_exact(4).enumerate() {
            let alpha = u32::from(px[3]);
            let at = (y * out_w + x) * 4;
            for (out_c, c) in out[at..at + 4].iter_mut().zip(colour) {
                *out_c = ((u32::from(c) * alpha + 127) / 255) as u8;
            }
        }
    }
    out
}

/// One stencil mask open across the bands of a page: rows read as packed
/// sample bits a band paints, never the mask's whole bitmap. A stencil larger
/// than its device footprint is read as coverage at that footprint.
pub(crate) struct StencilCursor {
    rows: ImageRows,
    /// Set when a row the source could not read has been met: from then on
    /// every row is the padding that no paint under this mask's `/Decode`
    /// reads, the page as it would look with the stencil absent.
    failed: bool,
    reduce: Option<StencilReduce>,
}

impl StencilCursor {
    /// Opens the stencil `xobject` (`/ImageMask true`) for a device footprint
    /// of `footprint` pixels: a stencil larger than that in a dimension is
    /// reduced to it in that dimension.
    pub(crate) fn open(
        doc: &crate::document::PdfDocument,
        xobject: &Object,
        footprint: (u32, u32),
    ) -> Result<Self> {
        let rows = ImageRows::open_stencil(doc, xobject)?;
        let (width, height) = rows.size();
        let reduce = (width > footprint.0 || height > footprint.1).then(|| {
            let (dst_w, dst_h) = (width.min(footprint.0), height.min(footprint.1));
            StencilReduce {
                resampler: RowResampler::new(width, height, dst_w, dst_h),
                next_dst: 0,
                bits: vec![0; (width as usize).div_ceil(8)],
                cov: vec![0; width as usize * 4],
            }
        });
        Ok(Self { rows, failed: false, reduce })
    }

    /// The size the stencil is painted at: its own, or its footprint when it
    /// was reduced.
    pub(crate) fn size(&self) -> (u32, u32) {
        match &self.reduce {
            Some(reduce) => reduce.resampler.dst_size(),
            None => self.rows.size(),
        }
    }

    /// The rows `y0..y1` of the stencil as a band paints them.
    pub(crate) fn band(&mut self, y0: u32, y1: u32, invert: bool) -> Result<StencilBand> {
        if self.reduce.is_some() {
            let (width, _) = self.size();
            let rgba = self.coverage(y0, y1, invert)?;
            return Ok(StencilBand::Coverage { rgba, width: width as usize });
        }
        let row_bytes = self.row_bytes();
        Ok(StencilBand::Bits { rows: self.region(y0, y1)?, row_bytes, invert })
    }

    /// The reduced rows `y0..y1` as premultiplied RGBA8, the whole reduced
    /// width a row, the alpha the stencil's coverage of each pixel. Rows are
    /// asked for in order: a request behind the last row produced starts the
    /// decode over.
    fn coverage(&mut self, y0: u32, y1: u32, invert: bool) -> Result<Vec<u8>> {
        let Some(reduce) = self.reduce.as_mut() else {
            return Err(Error::Image("stencil has no reduction to read".to_string()));
        };
        let (dst_w, dst_h) = reduce.resampler.dst_size();
        let y1 = y1.min(dst_h);
        if y0 >= y1 {
            return Ok(Vec::new());
        }
        if y0 < reduce.next_dst {
            if self.rows.restart().is_err() {
                self.failed = true;
            }
            reduce.resampler.reset();
            reduce.next_dst = 0;
        }
        let mut out = vec![0u8; (y1 - y0) as usize * dst_w as usize * 4];
        for dst in y0..y1 {
            let (start, stop) = reduce.resampler.source_rows_of(dst);
            while reduce.resampler.next_source_row() < start {
                self.rows.skip(1)?;
                reduce.resampler.skip();
            }
            while reduce.resampler.next_source_row() < stop {
                if self.failed {
                    reduce.bits.fill(self.rows.no_paint_sample_byte());
                } else if self.rows.read_samples(&mut reduce.bits).is_err() || self.rows.failed() {
                    self.failed = true;
                    reduce.bits.fill(self.rows.no_paint_sample_byte());
                }
                for (x, px) in reduce.cov.chunks_exact_mut(4).enumerate() {
                    let bit = (reduce.bits[x / 8] >> (7 - x % 8)) & 1 == 1;
                    px.fill(if bit == invert { 255 } else { 0 });
                }
                reduce.resampler.feed(&reduce.cov);
            }
            let at = (dst - y0) as usize * dst_w as usize * 4;
            reduce.resampler.take(dst, &mut out[at..at + dst_w as usize * 4]);
            reduce.next_dst = dst + 1;
        }
        Ok(out)
    }

    /// The stencil's sample bits for the rows `y0..y1`, each `row_bytes`
    /// bytes, bottom left in the band's order (topmost first). A request
    /// behind the last row produced starts the decode over. `rows.next`
    /// advances, so the same rows are read once across the bands.
    pub(crate) fn region(&mut self, y0: u32, y1: u32) -> Result<Vec<u8>> {
        let (_, height) = self.rows.size();
        let y1 = y1.min(height);
        if y0 >= y1 {
            return Ok(Vec::new());
        }
        if y0 < self.rows.next {
            if self.rows.restart().is_err() {
                self.failed = true;
            }
        }
        if self.rows.next < y0 {
            self.rows.skip(y0 - self.rows.next)?;
        }
        let row_bytes = self.row_bytes();
        let mut out = vec![0u8; row_bytes * (y1 - y0) as usize];
        for row in out.chunks_exact_mut(row_bytes) {
            if self.failed {
                row.fill(self.no_paint_byte());
            } else if self.rows.read_samples(row).is_err() || self.rows.failed() {
                self.failed = true;
                row.fill(self.no_paint_byte());
            }
        }
        Ok(out)
    }

    /// Bytes a stencil row spans, which every row of a region holds.
    fn row_bytes(&self) -> usize {
        let (width, _) = self.rows.size();
        (width as usize).div_ceil(8)
    }

    /// The sample bits of the padding when the stencil's rows stop: none of
    /// them paints under this mask's own `/Decode` (which the row source
    /// knows through its `one` and its folded fax polarity).
    fn no_paint_byte(&self) -> u8 {
        self.rows.no_paint_sample_byte()
    }

    /// Bytes this cursor holds in memory: the row source's (its reader
    /// windows, an exception's whole bitmap) and little of its own.
    fn retained_bytes(&self) -> usize {
        let reduced = self.reduce.as_ref().map_or(0, |reduce| {
            reduce.resampler.retained_bytes() + reduce.bits.capacity() + reduce.cov.capacity()
        });
        self.rows.retained_bytes() + reduced + std::mem::size_of::<StencilCursor>()
    }
}
/// Rows of an image in the sample terms the stream stores, for a redaction's
/// copy: post-filter, post-decrypt, before `/Decode` maps them to colour, so
/// the untouched pixels are the source's own bytes and the covered ones are
/// written once at the source's depth.
pub(crate) struct SampleRows {
    height: u32,
    /// Bytes one row spans: the samples packed as the copy re-encodes.
    row_bytes: usize,
    next: u32,
    kind: SampleKind,
}

enum SampleKind {
    /// A raw-sample stream read through its filters a row at a time; the
    /// padding of a stream too short for its declared rows is zeroes.
    Reader { reader: Box<dyn Read>, row: Vec<u8> },
    /// The packed rows a JBIG2 decode keeps whole (the ruled exception:
    /// d-122 composes the page bitmap before any row could leave it). Bits
    /// are the samples, MSB first, padded to a byte.
    Packed { packed: Vec<u8>, at: u32 },
    /// A stencil or an explicit mask's sample bits a row at a time, off the
    /// stencil row cursor (its raw, CCITT and JBIG2 kinds alike); one row
    /// `width.div_ceil(8)` bytes.
    Stencil { rows: ImageRows },
    /// A codec raster or a fax image read as luma rows of the colour space's
    /// own brightness, packed one bit a pixel into the copy's samples.
    Luma { rows: ImageRows, gray: Vec<u8> },
}

impl SampleRows {
    /// Opens the stored-sample rows of the image `xobject`; `color_spaces`
    /// resolves the named colour spaces the dictionary refers to, exactly as
    /// the row cursor the renderer opens does.
    pub(crate) fn open(
        doc: &crate::document::PdfDocument,
        xobject: &Object,
        color_spaces: &HashMap<String, Object>,
    ) -> Result<Self> {
        let Object::Stream { dict, body } = xobject else {
            return Err(Error::Image("XObject is not a stream".to_string()));
        };
        let filters = dict
            .get("Filter")
            .map(crate::object::extract_filter_names)
            .unwrap_or_default();
        let height_row = |h: u32, row_bytes: usize, kind: SampleKind| {
            Ok(Self { height: h, row_bytes, next: 0, kind })
        };
        if matches!(dict.get("ImageMask"), Some(Object::Boolean(true))) {
            let rows = ImageRows::open_stencil(doc, xobject)?;
            let (w, h) = rows.size();
            return height_row(h, (w as usize).div_ceil(8), SampleKind::Stencil { rows });
        }
        let filters_now = filters.as_slice();
        if is_filter(filters_now, &["JBIG2Decode"]) {
            let geometry = row_geometry(Some(doc), dict, Some(color_spaces))?;
            let packed = crate::extractors::images::decode_jbig2_bits(
                xobject,
                dict,
                Some(doc),
                geometry.width,
                geometry.height,
                true,
            )?;
            return height_row(
                geometry.height,
                (geometry.width as usize).div_ceil(8),
                SampleKind::Packed { packed, at: 0 },
            );
        }
        if is_filter(filters_now, &["CCITTFaxDecode"]) {
            let rows = ImageRows::open(doc, xobject, color_spaces, None, Output::Luma, None)?;
            let (w, h) = rows.size();
            return height_row(
                h,
                (w as usize).div_ceil(8),
                SampleKind::Luma { rows, gray: vec![0; w as usize] },
            );
        }
        if is_filter(filters_now, &["DCTDecode", "JPXDecode"]) {
            return Err(Error::Image(
                "a DCT or JPX image's redaction copy is piped from its raster, never held rows"
                    .to_string(),
            ));
        }
        // Raw-sample: the filters (predictor included) are the rows.
        let geometry = row_geometry(Some(doc), dict, Some(color_spaces))?;
        height_row(
            geometry.height,
            geometry.row_bytes,
            SampleKind::Reader {
                reader: Strips::open_reader(dict, body, None)?,
                row: Vec::new(),
            },
        )
    }

    /// Bytes one row spans.
    pub(crate) fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    /// The next row into `out`, `row_bytes` bytes of packed samples: the
    /// source's own stored samples for a raw, JBIG2 or stencil source
    /// (before its `/Decode` maps them to colour), and the colour space's
    /// brightness at one bit for a codec or fax source.
    pub(crate) fn read(
        &mut self,
        doc: &crate::document::PdfDocument,
        out: &mut [u8],
    ) -> Result<()> {
        let y = self.next;
        if y >= self.height {
            return Err(Error::Image("image has no more rows".to_string()));
        }
        match &mut self.kind {
            SampleKind::Reader { reader, row } => {
                if row.len() != self.row_bytes {
                    row.clear();
                    row.resize(self.row_bytes, 0);
                }
                let mut filled = 0;
                while filled < self.row_bytes {
                    match reader.read(&mut row[filled..]) {
                        Ok(0) => break,
                        Ok(n) => filled += n,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {},
                        Err(e) => return Err(crate::encryption::stream::from_io(e)),
                    }
                }
                row[filled..].fill(0);
                out.copy_from_slice(row);
            },
            SampleKind::Packed { packed, at } => {
                let start = *at as usize * self.row_bytes;
                let end = start + self.row_bytes;
                if end > packed.len() {
                    return Err(Error::Image("packed rows end early".to_string()));
                }
                out.copy_from_slice(&packed[start..end]);
                *at += 1;
            },
            SampleKind::Stencil { rows } => rows.read_samples(out)?,
            SampleKind::Luma { rows, gray } => {
                rows.read(doc, gray)?;
                // One luma byte a pixel, packed MSB-first one bit a sample.
                for (x, &luma) in gray.iter().enumerate() {
                    let mask = 1u8 << (7 - (x % 8));
                    let Some(b) = out.get_mut(x / 8) else { break };
                    if luma >= 128 {
                        *b |= mask;
                    } else {
                        *b &= !mask;
                    }
                }
            },
        }
        self.next += 1;
        Ok(())
    }
}

/// nearest-neighbour onto the base image's grid (§8.9.6.3 maps both to the
/// same unit square, so they need not share a resolution).
struct MaskRows {
    rows: ImageRows,
    /// The mask row held and its samples.
    held: Option<(u32, Vec<u8>)>,
    /// Set when the mask could not be read: it masks nothing from then on.
    /// A mask that cannot be read has no sample to test, so the base image
    /// shows through, the result an absent mask gives; resolving it the other
    /// way would erase the picture the mask only meant to refine.
    failed: bool,
}

impl MaskRows {
    fn new(rows: ImageRows) -> Self {
        Self { rows, held: None, failed: false }
    }

    fn restart(&mut self) {
        self.held = None;
        if self.rows.restart().is_err() {
            self.failed = true;
        }
    }

    /// Multiplies the alpha of base row `y` (of `base_h` rows, `row` the RGBA8
    /// of `base_w` pixels) by the mask sample under each pixel.
    fn apply(
        &mut self,
        doc: &crate::document::PdfDocument,
        y: u32,
        base_w: u32,
        base_h: u32,
        row: &mut [u8],
    ) -> Result<()> {
        let (mw, mh) = self.rows.size();
        let my = ((u64::from(y) * u64::from(mh) / u64::from(base_h)) as u32).min(mh - 1);
        if self.held.as_ref().map(|(index, _)| *index) != Some(my) {
            if my < self.rows.next {
                self.rows.restart()?;
            }
            self.rows.skip(my - self.rows.next)?;
            let mut samples = self.held.take().map(|(_, v)| v).unwrap_or_default();
            samples.clear();
            samples.resize(mw as usize, 0);
            self.rows.read(doc, &mut samples)?;
            self.held = Some((my, samples));
        }
        let Some((_, samples)) = &self.held else {
            return Ok(());
        };
        for x in 0..base_w {
            let mx = ((u64::from(x) * u64::from(mw) / u64::from(base_w)) as u32).min(mw - 1);
            let px = &mut row[x as usize * 4..x as usize * 4 + 4];
            px[3] = ((u32::from(px[3]) * u32::from(samples[mx as usize])) / 255) as u8;
        }
        Ok(())
    }
}

/// An image open across the bands of a page.
pub(crate) struct ImageCursor {
    base: ImageRows,
    masks: Vec<MaskRows>,
    resampler: Option<RowResampler>,
    /// The next resampled row to produce.
    next_scaled: u32,
    /// The last full-size rows read, premultiplied: rows
    /// `tail_first..base.next`.
    tail: VecDeque<Vec<u8>>,
    tail_first: u32,
}

impl ImageCursor {
    /// Opens the image `xobject`; `mask` is its `/Mask` entry (a reference to
    /// a mask image, or a colour-key array) and `smask` its `/SMask` entry.
    pub(crate) fn open(
        doc: &crate::document::PdfDocument,
        xobject: &Object,
        color_spaces: &HashMap<String, Object>,
        smask: Option<&Object>,
        mask: Option<&Object>,
        target: Option<(u32, u32)>,
    ) -> Result<Self> {
        let colour_key = match mask {
            Some(Object::Array(ranges)) => Some(ranges.as_slice()),
            _ => None,
        };
        let base = ImageRows::open(doc, xobject, color_spaces, target, Output::Rgba, colour_key)?;
        let mut masks = Vec::new();

        // /Mask: an image mask, where sample 0 shows the base image under the
        // default /Decode (§8.9.6.3).
        if let Some(reference) = mask.and_then(|m| m.as_reference()) {
            if let Ok(stream) = doc.load_object(reference) {
                match ImageRows::open(doc, &stream, color_spaces, target, Output::Luma, None) {
                    Ok(rows) => masks.push(MaskRows::new(rows)),
                    Err(_) => {
                        if let Ok(rows) = ImageRows::open_stencil(doc, &stream) {
                            masks.push(MaskRows::new(rows));
                        }
                    },
                }
            }
        }
        if let Some(smask) = smask {
            if let Ok(resolved) = doc.resolve_object(smask) {
                if let Ok(rows) =
                    ImageRows::open(doc, &resolved, color_spaces, target, Output::Luma, None)
                {
                    masks.push(MaskRows::new(rows));
                }
            }
        }
        Ok(Self {
            base,
            masks,
            resampler: None,
            next_scaled: 0,
            tail: VecDeque::new(),
            tail_first: 0,
        })
    }

    /// The image's size in samples, which a codec may have reduced.
    pub(crate) fn size(&self) -> (u32, u32) {
        self.base.size()
    }

    /// Bytes this cursor holds in memory: the decoded raster or strip, the
    /// masks' the same, the rows kept behind the position and the resampler's
    /// rows, with 16 KiB for what the cursor is itself (its dictionaries, the
    /// colour spaces it was opened with and the state of its filter readers).
    fn retained_bytes(&self) -> usize {
        let masks: usize = self
            .masks
            .iter()
            .map(|m| m.rows.retained_bytes() + m.held.as_ref().map_or(0, |(_, v)| v.capacity()))
            .sum();
        let tail: usize = self.tail.iter().map(|row| row.capacity()).sum();
        let resampler = self.resampler.as_ref().map_or(0, |r| r.retained_bytes());
        16 * 1024 + self.base.retained_bytes() + masks + tail + resampler
    }

    /// Resamples to `dst_w x dst_h`; fixed for the cursor's life.
    pub(crate) fn scale_to(&mut self, dst_w: u32, dst_h: u32) {
        if self.resampler.is_none() {
            let (w, h) = self.base.size();
            self.resampler = Some(RowResampler::new(w, h, dst_w, dst_h));
        }
    }

    /// Reads the next full-size row as straight RGBA8, its masks applied to
    /// the alpha.
    pub(crate) fn read_straight(
        &mut self,
        doc: &crate::document::PdfDocument,
        out: &mut [u8],
    ) -> Result<()> {
        let y = self.base.next;
        let (w, h) = self.base.size();
        self.base.read(doc, out)?;
        for mask in self.masks.iter_mut().filter(|m| !m.failed) {
            if let Err(e) = mask.apply(doc, y, w, h, out) {
                log::debug!("image mask could not be read, so it masks nothing: {e}");
                mask.failed = true;
            }
        }
        Ok(())
    }

    /// Whether the image has a mask that is read with it.
    pub(crate) fn has_masks(&self) -> bool {
        !self.masks.is_empty()
    }

    /// Reads the next full-size row, masks applied and premultiplied.
    fn pull(&mut self, doc: &crate::document::PdfDocument, out: &mut [u8]) -> Result<()> {
        self.read_straight(doc, out)?;
        premultiply_in_place(out);
        Ok(())
    }

    fn restart(&mut self) -> Result<()> {
        self.base.restart()?;
        for mask in &mut self.masks {
            mask.restart();
        }
        if let Some(resampler) = &mut self.resampler {
            resampler.reset();
        }
        self.next_scaled = 0;
        self.tail.clear();
        self.tail_first = 0;
        Ok(())
    }

    /// Resampled rows `first..end` as premultiplied RGBA8, each
    /// `dst_w * 4` bytes. A request behind the last row produced starts the
    /// decode over.
    pub(crate) fn scaled_rows(
        &mut self,
        doc: &crate::document::PdfDocument,
        first: u32,
        end: u32,
    ) -> Result<Vec<u8>> {
        let Some(resampler) = self.resampler.as_ref() else {
            return Err(Error::Image("image cursor has no scale".to_string()));
        };
        let (dst_w, dst_h) = resampler.dst_size();
        let end = end.min(dst_h);
        if first >= end {
            return Ok(Vec::new());
        }
        if first < self.next_scaled {
            self.restart()?;
        }
        let (w, _) = self.base.size();
        let mut row = vec![0u8; w as usize * 4];
        let mut out = vec![0u8; (end - first) as usize * dst_w as usize * 4];
        for d in first..end {
            let (start, stop) = self.resampler.as_ref().map_or((0, 0), |r| r.source_rows_of(d));
            while self.resampler.as_ref().is_some_and(|r| r.next_source_row() < start) {
                self.base.skip(1)?;
                if let Some(r) = self.resampler.as_mut() {
                    r.skip();
                }
            }
            while self.resampler.as_ref().is_some_and(|r| r.next_source_row() < stop) {
                self.pull(doc, &mut row)?;
                if let Some(r) = self.resampler.as_mut() {
                    r.feed(&row);
                }
            }
            let at = (d - first) as usize * dst_w as usize * 4;
            if let Some(r) = self.resampler.as_mut() {
                r.take(d, &mut out[at..at + dst_w as usize * 4]);
            }
            self.next_scaled = d + 1;
        }
        Ok(out)
    }

    /// The rectangle columns `x0..x1`, rows `y0..y1` of the full-size image as
    /// premultiplied RGBA8. Rows are asked for from the top down; the last
    /// few are kept so that the next request may begin a little above where
    /// this one ended, and one that begins above those starts the decode over.
    pub(crate) fn region(
        &mut self,
        doc: &crate::document::PdfDocument,
        x0: u32,
        x1: u32,
        y0: u32,
        y1: u32,
    ) -> Result<Vec<u8>> {
        let (w, h) = self.base.size();
        let (x1, y1) = (x1.min(w), y1.min(h));
        if x0 >= x1 || y0 >= y1 {
            return Ok(Vec::new());
        }
        if y0 < self.tail_first {
            self.restart()?;
        }
        if y0 > self.base.next {
            let skip = y0 - self.base.next;
            self.base.skip(skip)?;
            self.tail.clear();
            self.tail_first = self.base.next;
        }
        let width = (x1 - x0) as usize;
        let mut out = vec![0u8; width * (y1 - y0) as usize * 4];
        for y in y0..y1 {
            if y >= self.base.next {
                let mut row = vec![0u8; w as usize * 4];
                self.pull(doc, &mut row)?;
                self.tail.push_back(row);
            }
            let row = &self.tail[(y - self.tail_first) as usize];
            let at = (y - y0) as usize * width * 4;
            out[at..at + width * 4].copy_from_slice(&row[x0 as usize * 4..x1 as usize * 4]);
        }
        let keep = (TAIL_BYTES / (w as usize * 4)).max(1);
        while self.tail.len() > keep {
            self.tail.pop_front();
            self.tail_first += 1;
        }
        Ok(out)
    }
}

/// What a page's open images are keyed by: the stream, or for an inline image
/// the hash of its data, and the transform the image is painted under.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CursorKey {
    source: Source,
    transform: [u32; 6],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Source {
    Reference(ObjectRef),
    Stream(u64),
}

impl CursorKey {
    /// An inline image's key hashes all of its data, polling the job's check
    /// every [`STRIDE`](crate::cancel::STRIDE) bytes.
    pub(crate) fn new(
        obj_ref: Option<ObjectRef>,
        xobject: &Object,
        transform: [f32; 6],
    ) -> crate::error::Result<Self> {
        use std::hash::{Hash, Hasher};
        let source = match (obj_ref, xobject) {
            (Some(r), _) => Source::Reference(r),
            (None, Object::Stream { body, .. }) => {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                match body.inline() {
                    Some(bytes) => {
                        bytes.len().hash(&mut hasher);
                        for chunk in bytes.chunks(crate::cancel::STRIDE as usize) {
                            crate::cancel::poll()?;
                            hasher.write(chunk);
                        }
                    },
                    None => format!("{body:?}").hash(&mut hasher),
                }
                Source::Stream(hasher.finish())
            },
            (None, _) => Source::Stream(0),
        };
        Ok(Self { source, transform: transform.map(f32::to_bits) })
    }
}

/// Bytes of open images a page keeps beside the one it is drawing. An image
/// kept past this is read again from its first row when it is next drawn.
const TABLE_BYTES: usize = 4 * 1024 * 1024;

/// A page's open images. One that is not drawn in the next band is of no use
/// to a page that paints thousands of inline images, so the table holds a
/// byte budget of them and puts the least recently drawn out.
/// What a let-over cursor says of the bytes it holds, which the table's
/// budget works with.
pub(crate) trait CursorBytes {
    fn held_bytes(&self) -> usize;
}

pub(crate) struct CursorTable<T> {
    open: HashMap<CursorKey, (u64, usize, T)>,
    bytes: usize,
    clock: u64,
}

impl<T> Default for CursorTable<T> {
    fn default() -> Self {
        Self { open: HashMap::new(), bytes: 0, clock: 0 }
    }
}

impl CursorTable<ImageCursor> {
    /// Bytes the open images hold.
    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
}

impl<T: CursorBytes> CursorTable<T> {
    pub(crate) fn clear(&mut self) {
        self.open.clear();
        self.bytes = 0;
    }

    /// The open cursor for `key`, which leaves the table while it is drawn.
    pub(crate) fn take(&mut self, key: &CursorKey) -> Option<T> {
        let (_, bytes, cursor) = self.open.remove(key)?;
        self.bytes -= bytes;
        Some(cursor)
    }

    /// Keeps `cursor` as the most recently drawn, and puts out older ones
    /// while the table holds more than its budget. The cursor just drawn is
    /// kept whatever it holds: a raster that is one image's alone is the
    /// decode, and dropping it would repeat the decode in the next band.
    pub(crate) fn put(&mut self, key: CursorKey, cursor: T) {
        self.clock += 1;
        let bytes = cursor.held_bytes();
        self.bytes += bytes;
        self.open.insert(key.clone(), (self.clock, bytes, cursor));
        while self.bytes > TABLE_BYTES {
            let oldest = self
                .open
                .iter()
                .filter(|(k, _)| **k != key)
                .min_by_key(|(_, (stamp, _, _))| *stamp)
                .map(|(k, _)| k.clone());
            let Some(oldest) = oldest else {
                break;
            };
            if let Some((_, bytes, _)) = self.open.remove(&oldest) {
                self.bytes -= bytes;
            }
        }
    }
}

impl CursorBytes for ImageCursor {
    fn held_bytes(&self) -> usize {
        self.retained_bytes()
    }
}

impl CursorBytes for StencilCursor {
    fn held_bytes(&self) -> usize {
        self.retained_bytes()
    }
}

#[cfg(test)]
mod fax_arm {
    use crate::decoders::CcittParams;
    use crate::error::Error;
    use crate::extractors::ccitt_bilevel::{bilevel_to_grayscale, decompress_ccitt};
    use crate::extractors::images::extract_image_from_xobject_at;
    use crate::extractors::{ImageData, PixelFormat};
    use crate::object::Object;
    use std::collections::HashMap;

    const WIDTH: u32 = 37;
    const HEIGHT: u32 = 11;

    /// A picture whose runs differ from row to row and whose width is not a
    /// multiple of eight.
    fn picture() -> Vec<Vec<bool>> {
        (0..HEIGHT as usize)
            .map(|y| (0..WIDTH as usize).map(|x| (x * 7 + y * 13) % 5 < 2 || x == y).collect())
            .collect()
    }

    fn group4(picture: &[Vec<bool>]) -> Vec<u8> {
        let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
        for row in picture {
            let pels = row
                .iter()
                .map(|&black| if black { fax::Color::Black } else { fax::Color::White });
            encoder.encode_line(pels, WIDTH).expect("encodes");
        }
        encoder.finish().expect("finishes").finish()
    }

    fn parms(columns: u32, rows: u32, black_is_1: bool) -> Object {
        Object::Dictionary(HashMap::from([
            ("K".to_string(), Object::Integer(-1)),
            ("Columns".to_string(), Object::Integer(i64::from(columns))),
            ("Rows".to_string(), Object::Integer(i64::from(rows))),
            ("BlackIs1".to_string(), Object::Boolean(black_is_1)),
        ]))
    }

    fn fax_image(code: Vec<u8>, extra: &[(&str, Object)]) -> Object {
        let mut dict = HashMap::from([
            ("Subtype".to_string(), Object::Name("Image".to_string())),
            ("Width".to_string(), Object::Integer(i64::from(WIDTH))),
            ("Height".to_string(), Object::Integer(i64::from(HEIGHT))),
            ("ColorSpace".to_string(), Object::Name("DeviceGray".to_string())),
            ("BitsPerComponent".to_string(), Object::Integer(1)),
            ("Filter".to_string(), Object::Name("CCITTFaxDecode".to_string())),
            ("DecodeParms".to_string(), parms(WIDTH, HEIGHT, false)),
        ]);
        for (key, value) in extra {
            dict.insert((*key).to_string(), value.clone());
        }
        Object::stream(dict, code)
    }

    /// The whole decode as it was: the packed bitmap of `decompress_ccitt`,
    /// one gray byte a pixel.
    fn oracle(code: &[u8], black_is_1: bool) -> Vec<u8> {
        let params = CcittParams {
            k: -1,
            columns: WIDTH,
            rows: Some(HEIGHT),
            black_is_1,
            ..Default::default()
        };
        bilevel_to_grayscale(&decompress_ccitt(code, &params).expect("the oracle decodes"), WIDTH, HEIGHT)
    }

    /// A fax image's pixels are what the line decoder gives for each polarity
    /// the dictionary can write, and are held as the gray raster, not as the
    /// stream.
    #[test]
    fn a_fax_image_is_the_gray_raster_of_its_whole_decode() {
        let code = group4(&picture());
        let decode = ("Decode", Object::Array(vec![Object::Integer(1), Object::Integer(0)]));
        let black_is_1 = ("DecodeParms", parms(WIDTH, HEIGHT, true));
        // `/BlackIs1` and `/Decode [1 0]` each invert, and together cancel.
        for (name, extra, inverted) in [
            ("plain", vec![], false),
            ("BlackIs1", vec![black_is_1.clone()], true),
            ("Decode [1 0]", vec![decode.clone()], true),
            ("both", vec![black_is_1, decode], false),
        ] {
            let image = extract_image_from_xobject_at(None, &fax_image(code.clone(), &extra), None, None, None)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let want = oracle(&code, inverted);
            let ImageData::Raw { pixels, format } = image.data() else {
                panic!("{name}: a fax image is raw samples");
            };
            assert_eq!(*format, PixelFormat::Grayscale, "{name}");
            assert_eq!(pixels.len(), (WIDTH * HEIGHT) as usize, "{name}: the raster, not the stream");
            assert_eq!(pixels, &want, "{name}: the gray bytes");
            assert_eq!(image.bits_per_component(), 8, "{name}");
            assert_eq!(
                image.to_dynamic_image().expect("converts").as_bytes(),
                want.as_slice(),
                "{name}: the picture"
            );
            assert!(image.ccitt_params().is_some(), "{name}: still classed as fax");
        }
    }

    /// A stream that ends before `/Height` rows leaves the rows after it
    /// white, as the cursor serves them.
    #[test]
    fn rows_past_the_stream_are_white() {
        let code = group4(&picture()[..4]);
        let image = extract_image_from_xobject_at(None, &fax_image(code, &[]), None, None, None)
            .expect("extracts");
        let ImageData::Raw { pixels, .. } = image.data() else { panic!("raw samples") };
        assert_eq!(pixels.len(), (WIDTH * HEIGHT) as usize);
        assert!(pixels[(4 * WIDTH) as usize..].iter().all(|&p| p == 255));
        assert!(pixels[..(4 * WIDTH) as usize].contains(&0));
    }

    /// A fax image the line decoder cannot take (a palette colour space, a
    /// depth other than one) is declined, never read whole.
    #[test]
    fn a_fax_image_the_cursor_cannot_take_is_declined() {
        let code = group4(&picture());
        let palette = Object::Array(vec![
            Object::Name("Indexed".to_string()),
            Object::Name("DeviceRGB".to_string()),
            Object::Integer(1),
            Object::String(vec![0, 0, 0, 255, 255, 255]),
        ]);
        for (name, extra) in [
            ("palette", vec![("ColorSpace", palette)]),
            ("eight bits", vec![("BitsPerComponent", Object::Integer(8))]),
        ] {
            let got = extract_image_from_xobject_at(None, &fax_image(code.clone(), &extra), None, None, None);
            assert!(matches!(got, Err(Error::Image(_))), "{name}: {got:?}");
        }
    }
}
