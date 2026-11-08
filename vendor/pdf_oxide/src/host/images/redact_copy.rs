//! A redaction's copy of an image: the covered pixels destroyed in the
//! image's own sample terms, everything left the source's bytes.
//!
//! `RedactPass` (redaction/transform) decides per placement; the save queues
//! the copy and its page walk writes it here, after the page's content,
//! through one streaming writer. What is held is one row, the strip a raw
//! source reads through, the few rows a resampler needs, and the encoded
//! output of the one image being written.
//!
//! By a source whose samples the stream stores raw, the untouched pixels are
//! the source's own bytes: the rows come straight from the stream's filters,
//! the covered bits are overwritten once, and the rows are re-encoded with
//! the PNG predictor at the source's bit depth. A codec image (DCT, JPX,
//! JBIG2, CCITT) decodes to the raster the row cursor produces and
//! re-encodes in the terms that raster reports — d-122's rulings, each
//! codec's whole raster inside its declared exception, and DCT re-encoded
//! as JPEG at the source's quality.

use std::collections::HashMap;
use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::XrefEntries;
use crate::error::{Error, Result};
use crate::extractors::images::{parse_color_space, ColorSpace};
use crate::host::positioned_write::PositionedWrite;
use crate::object::{Object, ObjectRef};
use crate::writer::ObjectSerializer;

use super::classify::{filter_names, Encoding};
use super::dct;
use super::encode::{best_filtered_row, FlateRows};
use super::execute::{Pipe, Shape, Source};
use super::stream::SampleRows;

/// What the covered pixels take.
pub(crate) enum CopySemantics {
    /// The overlay colour, converted to the image's colour space.
    Overlay,
    /// An explicit stencil `/Mask`: the sample under which the base image
    /// shows through (ISO 32000-1 §8.9.6.3). Decoded 0.0, which the default
    /// `/Decode [0 1]` maps to stored sample 0.
    Base,
    /// A soft mask's fully opaque sample: decoded 1.0 per component.
    Opaque,
}

/// `(rect, samples)`: the /Decode-mapped colour a covered pixel takes, one
/// stored sample per component, in units of the source's bit depth.
struct CoverFill {
    rect: [f32; 4],
    samples: Vec<u32>,
}

/// The overlay colour as the copy's samples, per the colour space the copy
/// stores its components in. Red covers a gray plane through its luma and a
/// CMYK plane through `1 - component`; a colour space whose components are
/// no device model takes sample zero: the overlay draws the mark's colour
/// above the box, and this only has to be a well-formed opaque fill.
fn fill_samples(rgb: [f32; 3], space: &ColorSpace, out: &mut Vec<f32>) {
    let clamp01 = |v: f32| v.clamp(0.0, 1.0);
    out.clear();
    match space {
        ColorSpace::DeviceRGB | ColorSpace::CalRGB => {
            for c in rgb {
                out.push(clamp01(c));
            }
        },
        ColorSpace::DeviceGray | ColorSpace::CalGray => {
            // Rec 601, as the image crate's to_luma8 computes it, so a
            // decoded pixel and a filled one agree on their luma.
            let luma = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
            out.push(clamp01(luma));
        },
        ColorSpace::DeviceCMYK => {
            let (r, g, b) = (clamp01(1.0 - rgb[0]), clamp01(1.0 - rgb[1]), clamp01(1.0 - rgb[2]));
            let k = r.min(g).min(b);
            let minus = |v: f32| (v - k).max(0.0);
            out.push(minus(r));
            out.push(minus(g));
            out.push(minus(b));
            out.push(k);
        },
        _ => out.resize(space.components(), 0.0),
    }
}

/// The `/Decode` pair of component `at`, or `[0 1]` where the array has no
/// entry for it.
fn decode_pair(decode: Option<&Object>, at: usize) -> (f64, f64) {
    let entries = decode.and_then(|d| d.as_array());
    let value = |i: usize, default: f64| -> f64 {
        match entries.and_then(|a| a.get(i)) {
            Some(Object::Real(v)) => *v,
            Some(Object::Integer(v)) => (*v) as f64,
            _ => default,
        }
    };
    (value(2 * at, 0.0), value(2 * at + 1, 1.0))
}

/// The covered pixels' fill, one per region, in stored sample units.
///
/// `/Decode` maps a stored fraction `t` to the decoded value
/// `a + t * (b - a)` (ISO 32000-1 §8.9.3.2), so the fill's decoded colour
/// goes back through `t`. A colour-key `/Mask` range the fill lands in is
/// nudged out of it, so the fill shows instead of being masked clear.
fn cover_fills(
    fills: &[([f32; 4], [f32; 3])],
    semantics: &CopySemantics,
    space: Option<&ColorSpace>,
    decode: Option<&Object>,
    bpc: u8,
    key: Option<Vec<(u32, u32)>>,
) -> Result<Vec<CoverFill>> {
    let space = space.map_or_else(
        || {
            parse_color_space(&Object::Name("DeviceGray".to_string()))
                .expect("a name the colour-space parser reads")
                .clone()
        },
        |s| s.clone(),
    );
    let ncomp = space.components().max(1);
    let max: u32 = match bpc {
        1 => 1,
        bpc => (1u32 << bpc.min(15)) - 1,
    };
    let mut out = Vec::with_capacity(fills.len());
    for (rect, rgb) in fills {
        let mut wanted = Vec::with_capacity(ncomp);
        match semantics {
            CopySemantics::Base => wanted.resize(ncomp, 0.0),
            CopySemantics::Opaque => wanted.resize(ncomp, 1.0),
            CopySemantics::Overlay => fill_samples(*rgb, &space, &mut wanted),
        }
        let mut samples = Vec::with_capacity(ncomp);
        for (i, &w) in wanted.iter().enumerate() {
            let (a, b) = decode_pair(decode, i);
            let t = if (b - a).abs() <= f64::EPSILON {
                0.0
            } else {
                ((f64::from(w) - a) / (b - a)).clamp(0.0, 1.0)
            };
            let v = (t * f64::from(max)).round() as u32;
            let v = match key.as_ref().and_then(|ranges| ranges.get(i)) {
                Some(&(lo, hi)) if v >= lo && v <= hi => {
                    if lo > 0 {
                        lo - 1
                    } else if hi < max {
                        hi + 1
                    } else {
                        v
                    }
                },
                _ => v,
            };
            samples.push(v);
        }
        out.push(CoverFill {
            rect: *rect,
            samples,
        });
    }
    Ok(out)
}

/// The covered pixel span of a `u`/`v` rect over a `w × h` image: columns
/// `x0..x1`, rows `y0..y1` counted from the top (the first row of stored
/// samples): row `y` holds `v ∈ [1-(y+1)/h, 1-y/h]`.
pub(crate) fn covered_span(
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    w: u32,
    h: u32,
) -> (usize, usize, usize, usize) {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let span = |lo: f64, hi: f64, size: f64| -> (usize, usize) {
        (
            lo.floor().clamp(0.0, size) as usize,
            hi.ceil().clamp(0.0, size) as usize,
        )
    };
    let (x0, x1) = span(
        f64::from(u0.min(u1)) * wf,
        f64::from(u1.max(u0)) * wf,
        wf,
    );
    let (y0, y1) = span(
        (1.0 - f64::from(v1.max(v0))) * hf,
        (1.0 - f64::from(v0.min(v1))) * hf,
        hf,
    );
    (x0, x1, y0, y1)
}
/// Writes one packed sample into `row`: big-endian value `v`, `bpc` bits
/// starting at `bit_at` from the left.
fn put_sample(row: &mut [u8], bit_at: usize, bpc: u8, v: u32) {
    for k in 0..bpc {
        let bit = bit_at + k as usize;
        let Some(b) = row.get_mut(bit / 8) else { break };
        let mask = 1u8 << (7 - (bit % 8));
        if v & (1 << (bpc as i32 - 1 - k as i32)) != 0 {
            *b |= mask;
        } else {
            *b &= !mask;
        }
    }
}

/// The fill a covered pixel takes: the last of `fills` whose rect contains the
/// pixel's centre. The list is in drawing order, so the last region is the one
/// drawn on top, the same choice the DCT block path makes.
fn pick_fill(
    x: usize,
    row: usize,
    w: usize,
    h: usize,
    fills: &[CoverFill],
) -> Option<&CoverFill> {
    let u = (x as f64 + 0.5) / w as f64;
    let v = 1.0 - (row as f64 + 0.5) / h as f64;
    fills.iter().rev().find(|f| {
        let [fu0, fv0, fu1, fv1] = f.rect;
        f64::from(fu0) <= u && u < f64::from(fu1) && f64::from(fv0) <= v && v < f64::from(fv1)
    })
}

/// The `/DecodeParms` of a PNG-predicted Flate stream: one filter tag per
/// row, `colors` components at `bpc` bits, `columns` wide.
fn predicted_parms(colors: usize, bpc: u8, columns: u32) -> Object {
    let mut d = HashMap::new();
    d.insert("Predictor".to_string(), Object::Integer(15));
    d.insert("Colors".to_string(), Object::Integer(colors.max(1) as i64));
    d.insert("BitsPerComponent".to_string(), Object::Integer(u64::from(bpc) as i64));
    d.insert("Columns".to_string(), Object::Integer(u64::from(columns) as i64));
    Object::Dictionary(d)
}

/// One row's PNG-predicted bytes into `out`: the filter tag byte first, the
/// filter the libpng heuristic takes, the previous row standing behind it.
fn filtered_row(
    row: &[u8],
    prior: &mut Vec<u8>,
    bpp: usize,
    filtered: &mut Vec<u8>,
    out: &mut dyn Write,
) -> Result<()> {
    filtered.clear();
    best_filtered_row(row, prior, bpp, filtered);
    prior.copy_from_slice(row);
    out.write_all(filtered)?;
    Ok(())
}

/// Rows packed with the PNG predictor into `out`, one at a time, as
/// `next_row` fills them. A row a source could not give is zero padding, as
/// the whole-image decode pads a stream short of its declared rows.
fn compress_rows(
    mut next_row: impl FnMut(&mut Vec<u8>, u32) -> Result<()>,
    row_bytes: usize,
    bpp: usize,
    height: u32,
    cancel: &CancelCheck,
    out: &mut dyn Write,
) -> Result<()> {
    let mut prior = vec![0u8; row_bytes];
    let mut row = vec![0u8; row_bytes];
    let mut filtered = Vec::with_capacity(row_bytes + 1);
    let mut zlib = ZlibEncoder::new(out, Compression::best());
    for y in 0..height {
        if y % 512 == 0 {
            cancel.check()?;
        }
        next_row(&mut row, y)?;
        filtered.clear();
        filtered_row(&row, &mut prior, bpp, &mut filtered, &mut zlib)?;
    }
    zlib.finish()?;
    Ok(())
}

/// The layout the row loop runs over.
struct RowLayout {
    width: u32,
    height: u32,
    /// Components per pixel.
    ncomp: usize,
    bpc: u8,
}

impl RowLayout {
    /// The predictor's byte group: the bytes one pixel spans, the filter's
    /// distance to the left neighbour that shares the same channel.
    fn bpp(&self) -> usize {
        (self.ncomp * usize::from(self.bpc)).div_ceil(8)
    }
}

/// The rows of a sample-space source, the covered columns of the covered
/// rows overwritten with their fill, packed and flate-coded into `out`.
/// The untouched pixels are the source's own bytes.
fn write_sample_rows(
    doc: &PdfDocument,
    rows: &mut SampleRows,
    layout: &RowLayout,
    fills: &[CoverFill],
    (x0, x1, y0, y1): (usize, usize, usize, usize),
    cancel: &CancelCheck,
    out: &mut dyn Write,
) -> Result<()> {
    let row_bytes = rows.row_bytes();
    compress_rows(
        |row, y| {
            rows.read(doc, row)?;
            if (y as usize) >= y0 && (y as usize) < y1 && !fills.is_empty() {
                for x in x0..x1 {
                    let Some(f) = pick_fill(x, y as usize, layout.width as usize, layout.height as usize, fills) else {
                        continue;
                    };
                    for (c, &v) in f.samples.iter().enumerate() {
                        put_sample(
                            row,
                            (x * layout.ncomp + c) * usize::from(layout.bpc),
                            layout.bpc,
                            v,
                        );
                    }
                }
            }
            Ok(())
        },
        row_bytes,
        layout.bpp(),
        layout.height,
        cancel,
        out,
    )
}

/// The next raster row of `pipe` into `row`, its covered pixels set to their
/// fill. `y` is the row's index from the top of the raster.
fn pull_covered_row(
    pipe: &mut Pipe,
    y: usize,
    fills: &[CoverFill],
    (x0, x1, y0, y1): (usize, usize, usize, usize),
    row: &mut [u8],
) -> Result<()> {
    pipe.pull(row)?;
    if y < y0 || y >= y1 {
        return Ok(());
    }
    let (w, h) = (pipe.width as usize, pipe.height as usize);
    let ncomp = row.len() / w;
    for x in x0..x1 {
        let Some(f) = pick_fill(x, y, w, h, fills) else {
            continue;
        };
        for (c, &v) in f.samples.iter().enumerate().take(ncomp) {
            row[x * ncomp + c] = v as u8;
        }
    }
    Ok(())
}

/// Re-encodes a JPEG 2000 raster in the raster's own colour model, its covered
/// pixels the fills, as Flate. The caller has set the dictionary's terms to match.
fn write_codec_rows(
    doc: &PdfDocument,
    object: &Object,
    kind: &super::classify::Kind,
    color_spaces: &HashMap<String, Object>,
    fills: &[CoverFill],
    span: (usize, usize, usize, usize),
    cancel: &CancelCheck,
    out: &mut dyn Write,
) -> Result<()> {
    let shape = match kind.color {
        super::classify::ColorModel::Bilevel | super::classify::ColorModel::Gray => Shape::Gray,
        super::classify::ColorModel::Rgb => Shape::Rgb,
        super::classify::ColorModel::Cmyk => Shape::Cmyk,
        // Lab, Separation, DeviceN, an indexed or unparsable base: the
        // raster's components are not a colour this encoding can write
        // reliably; refusing leaves the underlying pixels untouched.
        super::classify::ColorModel::Other => {
            return Err(Error::Unsupported(
                "the image under this redaction decodes to a colour space its re-encoded \
                 copy cannot carry; refusing to redact rather than write a wrong picture"
                    .to_string(),
            ));
        },
    };
    let source = Source::open(doc, object, kind, shape, color_spaces, None)?;
    let mut pipe = Pipe::new(doc, cancel, source, shape, None);
    let channels = shape.channels();
    let mut encoder = FlateRows::to_writer(out, pipe.width, channels)?;
    let mut row = vec![0u8; pipe.width as usize * usize::from(channels)];
    for y in 0..pipe.height as usize {
        pull_covered_row(&mut pipe, y, fills, span, &mut row)?;
        encoder.push(&row)?;
    }
    encoder.finish_writer()?;
    Ok(())
}

/// The first bytes a JPEG 2000 codestream's main header needs: the start
/// doubles from 64 KiB until the header is complete, up to the cap.
#[cfg(feature = "jpeg2000")]
const JPX_HEADER_START: usize = 64 * 1024;
#[cfg(feature = "jpeg2000")]
const JPX_HEADER_LIMIT: usize = 1024 * 1024;

/// Whether the codestream in `head` has its main header through the first
/// tile-part marker (ISO/IEC 15444-1 §A.4): the SOC, then marker segments
/// with lengths up to SOT.
#[cfg(feature = "jpeg2000")]
fn jpx_main_header_complete(head: &[u8]) -> bool {
    let Some(soc) = head.windows(4).position(|w| w == [0xFF, 0x4F, 0xFF, 0x51]) else {
        return false;
    };
    let mut at = soc + 2;
    while at + 4 <= head.len() {
        if head[at] != 0xFF {
            return false;
        }
        if matches!(head[at + 1], 0x90 | 0xD9) {
            return true;
        }
        let len = usize::from(u16::from_be_bytes([head[at + 2], head[at + 3]]));
        at += 2 + len;
    }
    false
}

/// The codestream's prefix through its complete main header, read through the
/// stream's own reader: 64 KiB, then doubled while the header is incomplete,
/// refused past 1 MiB. The stream is not read past the header's end.
#[cfg(feature = "jpeg2000")]
fn jpx_main_header(body: &crate::object::StreamBody) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut reader = body.raw_reader()?;
    let mut head = Vec::new();
    let mut target = JPX_HEADER_START;
    loop {
        let want = (target - head.len()) as u64;
        let got = (&mut reader)
            .take(want)
            .read_to_end(&mut head)
            .map_err(crate::encryption::stream::from_io)?;
        if (got as u64) < want || jpx_main_header_complete(&head) {
            return Ok(head);
        }
        if target >= JPX_HEADER_LIMIT {
            return Err(Error::Unsupported(
                "the JPEG 2000 image under this redaction has a main header longer than 1 MiB; \
                 refusing to redact rather than guess its size"
                    .to_string(),
            ));
        }
        target *= 2;
    }
}

/// Reads a codec stream's header and nothing after it: the codec opens and
/// states the size the dictionary declares. A JPEG is read by its marker walk
/// and proved to code in the baseline tables; neither holds a pixel.
fn check_codec_header(
    body: &crate::object::StreamBody,
    dict: &HashMap<String, Object>,
    encoding: Encoding,
) -> Result<()> {
    let refuse = |why: String| {
        Error::Unsupported(format!(
            "the codec image under this redaction has no readable header; refusing to redact \
             rather than write a copy that cannot be read back: {why}"
        ))
    };
    let declared = |key: &str| {
        dict.get(key)
            .and_then(|o| o.as_integer())
            .and_then(|v| u32::try_from(v).ok())
    };
    let (Some(width), Some(height)) = (declared("Width"), declared("Height")) else {
        return Err(refuse("the dictionary declares no /Width and /Height".to_string()));
    };
    match encoding {
        Encoding::Jpeg => {
            let jpeg = dct::Jpeg::read(body)?;
            let (fw, fh) = jpeg.size();
            if (fw, fh) != (width, height) {
                return Err(refuse(format!(
                    "the frame is {fw}x{fh}, the dictionary says {width}x{height}"
                )));
            }
            jpeg.check_codes(body, &CancelCheck::none())?;
        },
        Encoding::Jpx => {
            #[cfg(feature = "jpeg2000")]
            {
                let head = jpx_main_header(body)?;
                let settings = hayro_jpeg2000::DecodeSettings::default();
                let image = hayro_jpeg2000::Image::new(&head, &settings)
                    .map_err(|e| refuse(format!("{e:?}")))?;
                if (image.width(), image.height()) != (width, height) {
                    return Err(refuse(format!(
                        "the codestream is {}x{}, the dictionary says {width}x{height}",
                        image.width(),
                        image.height()
                    )));
                }
            }
            #[cfg(not(feature = "jpeg2000"))]
            {
                let _ = body;
                return Err(refuse("this build has no JPEG 2000 decoder".to_string()));
            }
        },
        _ => {},
    }
    Ok(())
}

/// Proves one queued copy's source can be written by the row writers,
/// without writing: the dictionary's codec and colour space are the same
/// classification the write runs, the row source opens. A copy that would
/// fail halfway raises [`Error::Unsupported`] here, at the dry run, so the
/// redaction is refused before anything is recorded.
pub(crate) fn validate_copy_source(
    doc: &PdfDocument,
    source: ObjectRef,
    color_spaces: &HashMap<String, Object>,
) -> Result<()> {
    let object = doc.load_object(source)?;
    let Object::Stream { dict, body } = &object else {
        return Err(Error::Image("a redaction copy's source is not a stream".to_string()));
    };
    let filter_names = filter_names(doc, dict);
    let filters: Vec<crate::extractors::images::PdfFilter> =
        filter_names.iter().map(|f| crate::extractors::images::PdfFilter::from_name(f)).collect();
    let encoding = match filters.last() {
        None
        | Some(crate::extractors::images::PdfFilter::FlateDecode)
        | Some(crate::extractors::images::PdfFilter::LZWDecode)
        | Some(crate::extractors::images::PdfFilter::RunLengthDecode)
        | Some(crate::extractors::images::PdfFilter::ASCIIHexDecode)
        | Some(crate::extractors::images::PdfFilter::ASCII85Decode)
        | Some(crate::extractors::images::PdfFilter::Crypt) => Encoding::Raw,
        Some(crate::extractors::images::PdfFilter::DCTDecode) => Encoding::Jpeg,
        Some(crate::extractors::images::PdfFilter::JPXDecode) => Encoding::Jpx,
        Some(crate::extractors::images::PdfFilter::CCITTFaxDecode) => Encoding::Ccitt,
        Some(crate::extractors::images::PdfFilter::JBIG2Decode) => Encoding::Jbig2,
        Some(crate::extractors::images::PdfFilter::Other(_)) => {
            return Err(Error::Unsupported(format!(
                "the image under this redaction is stored with a filter this engine \
                 does not decode; refusing to redact rather than leave its pixels \
                 recoverable"
            )));
        },
    };
    match encoding {
        // The stored-row sources are proved by opening them.
        Encoding::Raw | Encoding::Jbig2 | Encoding::Ccitt => {
            SampleRows::open(doc, &object, color_spaces)?;
        },
        // The codec sources are proved by their headers: the dry run never
        // decodes a pixel, the save decodes once.
        Encoding::Jpeg | Encoding::Jpx => {
            let stored = body.decrypted_len()?;
            let mut kind = super::classify::classify_stored(doc, dict, stored, &[]);
            if let Some(bit) = dict.get("BitsPerComponent").and_then(|o| o.as_integer()) {
                kind.bits = u8::try_from(bit).unwrap_or(8);
            }
            if matches!(kind.color, super::classify::ColorModel::Other) {
                return Err(Error::Unsupported(
                    "the image under this redaction decodes to a colour space its re-encoded \
                     copy cannot carry; refusing to redact rather than write a wrong picture"
                        .to_string(),
                ));
            }
            check_codec_header(body, dict, encoding)?;
        },
        Encoding::Unknown => unreachable!("an unknown filter raises before the branch"),
    }
    Ok(())
}

/// Writes one placement's copy of the image `source` as stream `out_id`, its
/// /Length the object `length_id`. The covered pixels take `semantics`' fill
/// inside the `u`/`v` rect; everything else is the source's own samples (or,
/// for a codec source whose samples the format does not store raw, the
/// raster's, re-encoded in the terms the codec's decode reports).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_copy(
    doc: &PdfDocument,
    source: ObjectRef,
    out_id: u32,
    length_id: u32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    fills: &[([f32; 4], [f32; 3])],
    semantics: &CopySemantics,
    color_spaces: &HashMap<String, Object>,
    dict_edits: &HashMap<String, Object>,
    cancel: &CancelCheck,
    writer: &mut impl PositionedWrite,
    serializer: &ObjectSerializer,
    handler: &Option<crate::encryption::EncryptionWriteHandler>,
    xref_entries: &mut XrefEntries,
) -> Result<()> {
    let object = doc.load_object(source)?;
    let Object::Stream { dict, body } = &object else {
        return Err(Error::Image(
            "a redaction copy's source is not a stream".to_string(),
        ));
    };
    let body = body.clone();
    let stored = body.decrypted_len()?;
    let filter_names = filter_names(doc, dict);
    let filters: Vec<crate::extractors::images::PdfFilter> =
        filter_names.iter().map(|f| crate::extractors::images::PdfFilter::from_name(f)).collect();
    let encoding = match filters.last() {
        None
        | Some(crate::extractors::images::PdfFilter::FlateDecode)
        | Some(crate::extractors::images::PdfFilter::LZWDecode)
        | Some(crate::extractors::images::PdfFilter::RunLengthDecode)
        | Some(crate::extractors::images::PdfFilter::ASCIIHexDecode)
        | Some(crate::extractors::images::PdfFilter::ASCII85Decode)
        | Some(crate::extractors::images::PdfFilter::Crypt) => Encoding::Raw,
        Some(crate::extractors::images::PdfFilter::DCTDecode) => Encoding::Jpeg,
        Some(crate::extractors::images::PdfFilter::JPXDecode) => Encoding::Jpx,
        Some(crate::extractors::images::PdfFilter::CCITTFaxDecode) => Encoding::Ccitt,
        Some(crate::extractors::images::PdfFilter::JBIG2Decode) => Encoding::Jbig2,
        Some(crate::extractors::images::PdfFilter::Other(_)) => {
            return Err(Error::Unsupported(format!(
                "the image under this redaction is stored with a filter this engine \
                 does not decode; refusing to redact rather than leave its pixels \
                 recoverable"
            )));
        },
    };
    if filters.len() > 1 {
        // Chains the codecs read of hide (e.g. a DCT under Crypt, or an
        // image codec above an ASCII layer) decode whole through the row
        // cursor; only the plain single-codec shapes the copy's encoders
        // name row by row.
        match encoding {
            Encoding::Raw => {},
            _ => {
                return Err(Error::Unsupported(format!(
                    "the image under this redaction chains a codec into {} filters; \
                     refusing to redact rather than decode it whole",
                    filter_names.len()
                )));
            },
        }
    }
    let jpeg = (encoding == Encoding::Jpeg)
        .then(|| dct::Jpeg::read(&body))
        .transpose()?;
    let image_mask = matches!(dict.get("ImageMask"), Some(Object::Boolean(true)));
    let width = dict
        .get("Width")
        .and_then(|o| o.as_integer())
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| Error::Image("a copy's source has no /Width".to_string()))?;
    let height = dict
        .get("Height")
        .and_then(|o| o.as_integer())
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| Error::Image("a copy's source has no /Height".to_string()))?;
    // A fax or JBIG2 image's rows are one bit a sample whatever its dictionary
    // says, so its fill and its packed rows agree on that depth.
    let bpc = if image_mask || matches!(encoding, Encoding::Ccitt | Encoding::Jbig2) {
        1u8
    } else {
        dict.get("BitsPerComponent")
            .and_then(|o| o.as_integer())
            .and_then(|b| u8::try_from(b).ok())
            .unwrap_or(8)
            .clamp(1, 16)
    };

    // The colour space the fill converts into. A colour-key /Mask is an
    // array in the source's own sample terms.
    let space = if image_mask {
        None
    } else {
        let resolved = crate::extractors::images::resolve_image_color_space(
            Some(doc),
            dict,
            Some(color_spaces),
        )?;
        Some(parse_color_space(&resolved)?)
    };
    let key = dict.get("Mask").and_then(|m| {
        doc.resolve_object(m)
            .ok()
            .and_then(|m| m.as_array().cloned())
            .and_then(|array| {
                crate::rendering::page_renderer::parse_color_key_mask(
                    &array,
                    space.as_ref().map_or(1, |s| s.components()),
                )
            })
    });
    // A fax copy's rows are already decoded brightness, so its /Decode does not
    // apply again; a codec copy's rows are 8 bits a sample whatever the source said.
    let fill_decode = if encoding == Encoding::Ccitt { None } else { dict.get("Decode") };
    let fill_bpc = if matches!(encoding, Encoding::Jpeg | Encoding::Jpx) { 8 } else { bpc };
    let fills = cover_fills(fills, semantics, space.as_ref(), fill_decode, fill_bpc, key)?;
    let (x0, x1, y0, y1) = covered_span(u0, v0, u1, v1, width, height);

    // The kind (raster colour model and bits) the codec branch pipes through;
    // the shape decision belongs in the dictionary too, so it is made once,
    // before the header.
    let kind = super::classify::classify_stored(doc, dict, stored, &[]);
    // The dictionary the copy carries.
    let mut out_dict = dict.clone();
    out_dict.remove("Length");
    out_dict.remove("Filter");
    out_dict.remove("DecodeParms");
    out_dict.remove("SMaskInData");
    for (key, value) in dict_edits {
        out_dict.insert(key.clone(), value.clone());
    }
    // A fax image's rows are its decoded brightness, one bit a pixel, so the
    // source's /Decode, which maps stored bits to brightness, must not apply again.
    if encoding == Encoding::Ccitt {
        out_dict.remove("Decode");
    }
    // The sample rows are written as PNG-predicted Flate, one component
    // count per pixel as the rows pack them.
    if matches!(encoding, Encoding::Raw | Encoding::Jbig2 | Encoding::Ccitt) {
        let colors = if image_mask || matches!(encoding, Encoding::Ccitt | Encoding::Jbig2) {
            1
        } else {
            crate::extractors::images::row_geometry(Some(doc), dict, Some(color_spaces))?.components
        };
        out_dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
        out_dict.insert("DecodeParms".to_string(), predicted_parms(colors, bpc, width));
    }
    // A JPEG copy is a baseline JPEG of the same components, so its dictionary
    // keeps the colour space and /Decode the source had.
    if encoding == Encoding::Jpeg {
        out_dict.insert("Filter".to_string(), Object::Name("DCTDecode".to_string()));
        out_dict.insert("BitsPerComponent".to_string(), Object::Integer(8));
    }
    // A JPEG 2000 copy's raster is re-encoded as Flate in the raster's own colour
    // model (physical components), the /Decode array gone with the colour space
    // named: this build has no colour-space keeping encoder for it.
    let codec_shape = match (encoding, image_mask) {
        (Encoding::Jpx, false) => match kind.color {
            super::classify::ColorModel::Bilevel | super::classify::ColorModel::Gray => Some(Shape::Gray),
            super::classify::ColorModel::Rgb => Some(Shape::Rgb),
            super::classify::ColorModel::Cmyk => Some(Shape::Cmyk),
            super::classify::ColorModel::Other => {
                return Err(Error::Unsupported(
                    "the image under this redaction decodes to a colour space its re-encoded \
                     copy cannot carry; refusing to redact rather than write a wrong picture"
                        .to_string(),
                ));
            },
        },
        _ => None,
    };
    if let Some(shape) = codec_shape {
        out_dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
        out_dict.insert("BitsPerComponent".to_string(), Object::Integer(8));
        out_dict.remove("Decode");
        let colors = match shape {
            Shape::Gray | Shape::Bilevel => 1,
            Shape::Rgb => 3,
            Shape::Cmyk => 4,
        };
        out_dict.insert("DecodeParms".to_string(), predicted_parms(colors, 8, width));
        out_dict.insert(
            "ColorSpace".to_string(),
            match shape {
                Shape::Cmyk => Object::Name("DeviceCMYK".to_string()),
                Shape::Rgb => Object::Name("DeviceRGB".to_string()),
                Shape::Gray | Shape::Bilevel => Object::Name("DeviceGray".to_string()),
            },
        );
    }

    let offset = writer.position();
    serializer.write_stream_open(&mut *writer, out_id, 0, &out_dict)?;
    let body_start = writer.position();

    let run = |out: &mut dyn Write| -> Result<()> {
        match encoding {
            Encoding::Raw => {
                let mut rows = SampleRows::open(doc, &object, color_spaces)?;
                // The stored-sample rows at the source's depth; a stencil's
                // no colour space, one packed row is `width.div_ceil(8)`
                // bytes.
                let ncomp = if image_mask {
                    1
                } else {
                    crate::extractors::images::row_geometry(Some(doc), dict, Some(color_spaces))?
                        .components
                };
                let layout = RowLayout { width, height, ncomp, bpc };
                write_sample_rows(doc, &mut rows, &layout, &fills, (x0, x1, y0, y1), cancel, out)
            },
            Encoding::Jbig2 | Encoding::Ccitt => {
                // Bits no matter the codec: a JBIG2 page bitmap (the ruled
                // exception) or the fax rows read as the renderer's luma.
                let mut rows = SampleRows::open(doc, &object, color_spaces)?;
                let layout = RowLayout { width, height, ncomp: 1, bpc: 1 };
                write_sample_rows(doc, &mut rows, &layout, &fills, (x0, x1, y0, y1), cancel, out)
            },
            Encoding::Jpeg => {
                let Some(jpeg) = jpeg.as_ref() else {
                    return Err(Error::Image("a JPEG copy has no coefficient header".to_string()));
                };
                // A block two regions share takes the fill of the last region in
                // the list that meets it: that region's overlay is drawn on top.
                let (w, h) = (f64::from(width), f64::from(height));
                let pick = |x0: usize, x1: usize, y0: usize, y1: usize| {
                    fills.iter().rev().find_map(|f| {
                        let [u0, v0, u1, v1] = f.rect.map(f64::from);
                        let meets = u0 * w < x1 as f64
                            && u1 * w > x0 as f64
                            && (1.0 - v1) * h < y1 as f64
                            && (1.0 - v0) * h > y0 as f64;
                        meets.then(|| f.samples.clone())
                    })
                };
                let cover = dct::Cover {
                    x0,
                    x1,
                    y0,
                    y1,
                    fill_at: &pick,
                };
                jpeg.write(&body, &cover, cancel, out)
            },
            Encoding::Jpx => write_codec_rows(
                doc,
                &object,
                &kind,
                color_spaces,
                &fills,
                (x0, x1, y0, y1),
                cancel,
                out,
            ),
            Encoding::Unknown => unreachable!("an unknown filter raised before the run"),
        }
    };

    match handler {
        None => run(&mut *writer)?,
        Some(h) => {
            let iv = crate::encryption::EncryptionWriteHandler::generate_iv()?;
            let mut enc = h
                .stream_cipher(out_id, 0)
                .encrypting(&mut *writer, iv)?;
            run(&mut enc)?;
            enc.finish()?;
        },
    }
    // The body is bound by the run; the length is the bytes the writer took.
    let len = writer.position() - body_start;
    writer.write_all(b"\nendstream\nendobj\n")?;
    xref_entries.push((out_id, offset, 0, true));
    let len_offset = writer.position();
    writer.write_all(&serializer.serialize_indirect(
        length_id,
        0,
        &Object::Integer(len as i64),
    ))?;
    xref_entries.push((length_id, len_offset, 0, true));
    Ok(())
}
