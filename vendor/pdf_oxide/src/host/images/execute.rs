//! Carry out a decision: read the image's rows, resample them, encode them,
//! rewrite the dictionary, stage the replacement — and the driver that runs
//! the five stages over a document.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! An image is never held whole. Its rows come from the row cursor of
//! `host::images::stream` (a raw-sample stream a strip at a time, a codec's
//! raster, a fax bitmap at one bit a pixel), pass one at a time through the
//! row resampler, and go straight into an encoder that takes rows. What is
//! held is a strip or the codec's raster, the resampler's few rows and the
//! encoded output of the one image being rewritten, which is the unit the
//! editor stages.

use std::collections::HashMap;
use std::io::Read;

use bytes::Bytes;

use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::error::{Error, Result};
use crate::extractors::images::{
    extract_image_from_xobject_at, row_geometry, ImageData, PdfFilter, PixelFormat,
};
use crate::object::{Object, ObjectRef, StreamBody};

use super::classify::{classify_stored, filter_names, ColorModel, Encoding, Kind};
use super::encode::{encode_jpeg_rows, CcittRows, FlateRows};
use super::inventory::document_images;
use super::policy::{decide, ppi_min, Decision, KeepReason, Output, Policy, Use};
use super::report::{Action, Outcome, Report};
use super::resample::RowResampler;
use super::stream::{ImageRows, Output as RowOutput};

/// The leading bytes of a JPEG that its quality is read from: the
/// quantization tables precede the first scan.
const HEAD_BYTES: u64 = 1024 * 1024;

/// Rows of a CMYK strip are read to about this many bytes at a time.
const STRIP_BYTES: usize = 256 * 1024;

/// Run the pipeline over every image drawn on the editor's visible pages
/// and stage each replacement for the next save.
///
/// All or nothing: the replacements are collected while the loop runs and
/// staged only once it has finished, so a cancel or an error at any image
/// leaves the editor as it was.
pub fn reduce_images(
    editor: &mut DocumentEditor,
    policy: &Policy,
    cancel: &CancelCheck,
) -> Result<Report> {
    let pages: Vec<usize> = editor
        .page_order_visible()
        .iter()
        .map(|&i| i as usize)
        .collect();
    let entries = document_images(editor.source(), &pages, cancel)?;
    let mut report = Report::default();
    let mut replacements: HashMap<u32, Object> = HashMap::new();
    cancel.each(entries, |entry| {
        let object = staged_or_source(editor, &replacements, entry.object)?;
        let Object::Stream { dict, body } = &object else {
            return Ok(());
        };
        let stored = body.decrypted_len()?;
        let kind = classify_stored(editor.source(), dict, stored, &head(editor.source(), dict, body)?);
        let uses: Vec<Use> = entry.placements.iter().map(|&ctm| Use { ctm }).collect();
        let mask_before = match kind.masks.soft_mask {
            Some(r) => stream_len(editor, &replacements, r)?,
            None => 0,
        };
        let bytes_before = stored + mask_before;
        let mut outcome = Outcome {
            object_id: entry.object.id,
            uses: uses.len() as u32,
            ppi_min: ppi_min(&kind, &uses),
            action: Action::Kept,
            keep_reason: None,
            bytes_before,
            bytes_after: bytes_before,
            width_after: kind.width,
            height_after: kind.height,
            kind: kind.clone(),
        };
        match decide(&kind, &uses, policy) {
            Decision::Keep(reason) => outcome.keep_reason = Some(reason),
            Decision::Reencode { target, output } => {
                let mask = match kind.masks.soft_mask {
                    Some(r) => Some((r, staged_or_source(editor, &replacements, r)?)),
                    None => None,
                };
                match apply(
                    editor.source(),
                    &object,
                    &kind,
                    mask.as_ref(),
                    target,
                    output,
                    cancel,
                ) {
                    Ok(applied) if saves_enough(applied.bytes, bytes_before, policy.min_savings) => {
                        outcome.action = if target.is_some() {
                            Action::Downsampled
                        } else {
                            Action::Recompressed
                        };
                        outcome.bytes_after = applied.bytes;
                        outcome.width_after = applied.width;
                        outcome.height_after = applied.height;
                        replacements.insert(entry.object.id, applied.parent);
                        if let Some((r, obj)) = applied.mask {
                            replacements.insert(r.id, obj);
                        }
                    },
                    Ok(_) => outcome.keep_reason = Some(KeepReason::BelowMinSavings),
                    Err(_) => {
                        // A cancelled run is not an image that cannot be read.
                        cancel.check()?;
                        outcome.keep_reason = Some(KeepReason::Undecodable);
                    },
                }
            },
        }
        report.images.push(outcome);
        Ok(())
    })?;
    for (id, object) in replacements {
        editor.insert_modified(id, object);
    }
    Ok(report)
}

/// The first bytes of a bare JPEG stream, which are all its quality is read
/// from; nothing for any other stream.
fn head(doc: &PdfDocument, dict: &HashMap<String, Object>, body: &StreamBody) -> Result<Vec<u8>> {
    let filters = filter_names(doc, dict);
    if filters.len() != 1 || !matches!(PdfFilter::from_name(&filters[0]), PdfFilter::DCTDecode) {
        return Ok(Vec::new());
    }
    let mut head = Vec::new();
    body.raw_reader()?
        .take(HEAD_BYTES)
        .read_to_end(&mut head)
        .map_err(crate::encryption::stream::from_io)?;
    Ok(head)
}

/// Strictly smaller, and by at least `min_savings` of the stored bytes.
fn saves_enough(after: u64, before: u64, min_savings: f64) -> bool {
    after < before && (before - after) as f64 >= before as f64 * min_savings.clamp(0.0, 1.0)
}

/// The object as this run sees it: a replacement this run has collected wins
/// over one the editor has staged, which wins over the source.
fn staged_or_source(
    editor: &mut DocumentEditor,
    replacements: &HashMap<u32, Object>,
    r: ObjectRef,
) -> Result<Object> {
    if let Some(collected) = replacements.get(&r.id) {
        return Ok(collected.clone());
    }
    if let Some(staged) = editor.modified_objects_mut().get(&r.id) {
        return Ok(staged.clone());
    }
    editor.source().load_object(r)
}

fn stream_len(
    editor: &mut DocumentEditor,
    replacements: &HashMap<u32, Object>,
    r: ObjectRef,
) -> Result<u64> {
    Ok(match staged_or_source(editor, replacements, r)? {
        Object::Stream { body, .. } => body.len(),
        _ => 0,
    })
}

struct Applied {
    parent: Object,
    mask: Option<(ObjectRef, Object)>,
    /// New stored bytes, parent plus mask.
    bytes: u64,
    width: u32,
    height: u32,
}

/// What a row holds per pixel on its way to the encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// One byte, 0 or 255; resampling thresholds instead of blending.
    Bilevel,
    /// One byte.
    Gray,
    /// Three bytes.
    Rgb,
    /// Four bytes, C M Y K.
    Cmyk,
}

impl Shape {
    pub(crate) fn channels(self) -> u8 {
        match self {
            Self::Bilevel | Self::Gray => 1,
            Self::Rgb => 3,
            Self::Cmyk => 4,
        }
    }

    /// `raw`, one row of this shape, as the four lanes a pixel the resampler
    /// takes.
    pub(crate) fn lanes(self, raw: &[u8], out: &mut [u8]) {
        match self {
            Self::Bilevel | Self::Gray => {
                for (&g, px) in raw.iter().zip(out.chunks_exact_mut(4)) {
                    px.copy_from_slice(&[g, 0, 0, 255]);
                }
            },
            Self::Rgb => {
                for (rgb, px) in raw.chunks_exact(3).zip(out.chunks_exact_mut(4)) {
                    px[..3].copy_from_slice(rgb);
                    px[3] = 255;
                }
            },
            Self::Cmyk => out.copy_from_slice(raw),
        }
    }

    /// The inverse of [`Self::lanes`] over resampled lanes.
    pub(crate) fn pack(self, lanes: &[u8], out: &mut [u8]) {
        match self {
            Self::Bilevel => {
                for (px, g) in lanes.chunks_exact(4).zip(out.iter_mut()) {
                    *g = if px[0] >= 128 { 255 } else { 0 };
                }
            },
            Self::Gray => {
                for (px, g) in lanes.chunks_exact(4).zip(out.iter_mut()) {
                    *g = px[0];
                }
            },
            Self::Rgb => {
                for (px, rgb) in lanes.chunks_exact(4).zip(out.chunks_exact_mut(3)) {
                    rgb.copy_from_slice(&px[..3]);
                }
            },
            Self::Cmyk => out.copy_from_slice(lanes),
        }
    }
}

/// Where an image's rows come from, in the shape the encoder is given.
pub(crate) enum Source {
    /// An image read as RGBA, its samples cut to the shape.
    Image { rows: ImageRows, shape: Shape, row: Vec<u8> },
    /// A gray mask read as luma; `invert` writes 0 where the stencil paints.
    Luma { rows: ImageRows, invert: bool },
    /// A CMYK image, its samples as stored.
    Cmyk(CmykRows),
}

impl Source {
    pub(crate) fn open(
        doc: &PdfDocument,
        object: &Object,
        kind: &Kind,
        shape: Shape,
        color_spaces: &HashMap<String, Object>,
        target: Option<(u32, u32)>,
    ) -> Result<Self> {
        if shape == Shape::Cmyk {
            return Ok(Self::Cmyk(CmykRows::open(doc, object, kind)?));
        }
        if kind.image_mask {
            return Ok(Self::Luma {
                rows: ImageRows::open_stencil(doc, object)?,
                invert: true,
            });
        }
        let rows = ImageRows::open(doc, object, color_spaces, target, RowOutput::Rgba, None)?;
        let (width, _) = rows.size();
        Ok(Self::Image {
            rows,
            shape,
            row: vec![0u8; width as usize * 4],
        })
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        match self {
            Self::Image { rows, .. } | Self::Luma { rows, .. } => rows.size(),
            Self::Cmyk(rows) => rows.size(),
        }
    }

    /// The next row into `out`: `width * channels` bytes.
    pub(crate) fn read(&mut self, doc: &PdfDocument, out: &mut [u8]) -> Result<()> {
        match self {
            Self::Image { rows, shape, row } => {
                rows.read(doc, row)?;
                if *shape == Shape::Rgb {
                    for (px, rgb) in row.chunks_exact(4).zip(out.chunks_exact_mut(3)) {
                        rgb.copy_from_slice(&px[..3]);
                    }
                } else {
                    for (px, g) in row.chunks_exact(4).zip(out.iter_mut()) {
                        *g = px[0];
                    }
                }
                Ok(())
            },
            Self::Luma { rows, invert } => {
                rows.read(doc, out)?;
                if *invert {
                    for g in out.iter_mut() {
                        *g = 255 - *g;
                    }
                }
                Ok(())
            },
            Self::Cmyk(rows) => rows.read(doc, out),
        }
    }
}

/// The samples of a CMYK image as stored, not converted to RGB.
pub(crate) enum CmykRows {
    /// A JPEG's raster, which the codec returns whole.
    Raster { pixels: Vec<u8>, width: u32, height: u32, next: u32 },
    /// A raw-sample stream read through its filters a strip of rows at a time.
    Strips(CmykStrips),
}

pub(crate) struct CmykStrips {
    /// The stream's dictionary without its filters, which `reader` applies.
    dict: HashMap<String, Object>,
    reader: Box<dyn Read>,
    row_bytes: usize,
    strip_rows: u32,
    width: u32,
    height: u32,
    /// CMYK samples of the rows `first..first + count`.
    pixels: Vec<u8>,
    first: u32,
    count: u32,
    next: u32,
}

impl CmykRows {
    pub(crate) fn open(doc: &PdfDocument, object: &Object, kind: &Kind) -> Result<Self> {
        let Object::Stream { dict, body } = object else {
            return Err(Error::Image("image XObject is not a stream".to_string()));
        };
        match kind.encoding {
            Encoding::Jpeg => {
                let image = extract_image_from_xobject_at(Some(doc), object, None, None, None)?;
                let (width, height) = (image.width(), image.height());
                let ImageData::Raw { pixels, format: PixelFormat::CMYK } = image.into_data() else {
                    return Err(Error::Image("CMYK JPEG decoded to another model".to_string()));
                };
                if width == 0 || height == 0 || pixels.len() != width as usize * height as usize * 4 {
                    return Err(Error::Image(format!(
                        "decoded {} bytes for {width}x{height}x4",
                        pixels.len()
                    )));
                }
                Ok(Self::Raster { pixels, width, height, next: 0 })
            },
            Encoding::Raw => {
                let geometry = row_geometry(Some(doc), dict, None)?;
                let mut plain = dict.clone();
                plain.remove("Filter");
                plain.remove("DecodeParms");
                let owned = body.clone();
                let reader = crate::decoders::stream::reader_in(dict, body.origin(), move || {
                    owned.owned_reader()
                })?;
                let strip_rows = u32::try_from((STRIP_BYTES / (geometry.width as usize * 4)).max(1))
                    .unwrap_or(1)
                    .min(geometry.height);
                Ok(Self::Strips(CmykStrips {
                    dict: plain,
                    reader,
                    row_bytes: geometry.row_bytes,
                    strip_rows,
                    width: geometry.width,
                    height: geometry.height,
                    pixels: Vec::new(),
                    first: 0,
                    count: 0,
                    next: 0,
                }))
            },
            _ => Err(Error::Image("CMYK image in a codec without a decoder".to_string())),
        }
    }

    fn size(&self) -> (u32, u32) {
        match self {
            Self::Raster { width, height, .. } => (*width, *height),
            Self::Strips(s) => (s.width, s.height),
        }
    }

    fn read(&mut self, doc: &PdfDocument, out: &mut [u8]) -> Result<()> {
        match self {
            Self::Raster { pixels, width, height, next } => {
                if *next >= *height {
                    return Err(Error::Image("image has no more rows".to_string()));
                }
                let stride = *width as usize * 4;
                let at = *next as usize * stride;
                out.copy_from_slice(&pixels[at..at + stride]);
                *next += 1;
                Ok(())
            },
            Self::Strips(s) => {
                if s.next >= s.height {
                    return Err(Error::Image("image has no more rows".to_string()));
                }
                if s.next >= s.first + s.count {
                    s.fill(doc)?;
                }
                let stride = s.width as usize * 4;
                let at = (s.next - s.first) as usize * stride;
                out.copy_from_slice(&s.pixels[at..at + stride]);
                s.next += 1;
                Ok(())
            },
        }
    }
}

impl CmykStrips {
    /// Decodes the strip that starts at row `next`.
    fn fill(&mut self, doc: &PdfDocument) -> Result<()> {
        let rows = self.strip_rows.min(self.height - self.next);
        let mut data = vec![0u8; rows as usize * self.row_bytes];
        // A stream shorter than its declared size is padded with zero samples.
        let mut filled = 0;
        while filled < data.len() {
            match self.reader.read(&mut data[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {},
                Err(e) => return Err(crate::encryption::stream::from_io(e)),
            }
        }
        let mut dict = self.dict.clone();
        dict.insert("Height".to_string(), Object::Integer(i64::from(rows)));
        let strip = Object::stream(dict, data);
        let image = extract_image_from_xobject_at(Some(doc), &strip, None, None, None)?;
        match image.data() {
            ImageData::Raw { pixels, format: PixelFormat::CMYK }
                if pixels.len() == rows as usize * self.width as usize * 4 =>
            {
                self.pixels.clear();
                self.pixels.extend_from_slice(pixels);
            },
            _ => return Err(Error::Image("CMYK image decoded to another model".to_string())),
        }
        self.first = self.next;
        self.count = rows;
        Ok(())
    }
}

/// An image's rows at the size they are written: read, resampled when the
/// size differs from the source's, one row at a time.
pub(crate) struct Pipe<'a> {
    doc: &'a PdfDocument,
    cancel: &'a CancelCheck,
    source: Source,
    pub(crate) shape: Shape,
    pub(crate) width: u32,
    pub(crate) height: u32,
    resampler: Option<RowResampler>,
    /// The next row to give.
    next: u32,
    /// One source row.
    raw: Vec<u8>,
    /// One source row, and one resampled row, as lanes.
    lanes_in: Vec<u8>,
    lanes_out: Vec<u8>,
}

impl<'a> Pipe<'a> {
    /// `target` is the size to write; `None` keeps the source's. A codec that
    /// decodes in reduced form has produced at least that size.
    pub(crate) fn new(
        doc: &'a PdfDocument,
        cancel: &'a CancelCheck,
        source: Source,
        shape: Shape,
        target: Option<(u32, u32)>,
    ) -> Self {
        let (src_w, src_h) = source.size();
        let (width, height) = target.unwrap_or((src_w, src_h));
        let resampler = ((width, height) != (src_w, src_h))
            .then(|| RowResampler::new(src_w, src_h, width, height));
        let channels = shape.channels() as usize;
        Self {
            doc,
            cancel,
            source,
            shape,
            width,
            height,
            resampler,
            next: 0,
            raw: vec![0u8; src_w as usize * channels],
            lanes_in: vec![0u8; src_w as usize * 4],
            lanes_out: vec![0u8; width as usize * 4],
        }
    }

    /// The next row into `out`: `width * channels` bytes.
    /// The next row into `out`: `width * channels` bytes.
    pub(crate) fn pull(&mut self, out: &mut [u8]) -> Result<()> {
        self.cancel.check()?;
        let Some(resampler) = self.resampler.as_mut() else {
            return self.source.read(self.doc, out);
        };
        let row = self.next;
        let (start, stop) = resampler.source_rows_of(row);
        while resampler.next_source_row() < stop {
            self.source.read(self.doc, &mut self.raw)?;
            if resampler.next_source_row() < start {
                resampler.skip();
            } else {
                self.shape.lanes(&self.raw, &mut self.lanes_in);
                resampler.feed(&self.lanes_in);
            }
        }
        if self.shape == Shape::Cmyk {
            resampler.take_lanes(row, &mut self.lanes_out);
        } else {
            resampler.take(row, &mut self.lanes_out);
        }
        self.shape.pack(&self.lanes_out, out);
        self.next += 1;
        Ok(())
    }

    /// Every row, into a Flate stream with predictors.
    pub(crate) fn flate(&mut self) -> Result<super::encode::FlateEncoded> {
        let channels = self.shape.channels();
        let mut encoder = FlateRows::new(self.width, channels)?;
        let mut row = vec![0u8; self.width as usize * channels as usize];
        for _ in 0..self.height {
            crate::cancel::poll()?;
            self.pull(&mut row)?;
            encoder.push(&row)?;
        }
        encoder.finish()
    }

    /// Every row, as CCITT Group 4.
    fn ccitt(&mut self) -> Result<Vec<u8>> {
        let mut encoder = CcittRows::new(self.width)?;
        let mut row = vec![0u8; self.width as usize];
        for _ in 0..self.height {
            crate::cancel::poll()?;
            self.pull(&mut row)?;
            encoder.push(&row)?;
        }
        Ok(encoder.finish())
    }
}

fn apply(
    doc: &PdfDocument,
    object: &Object,
    kind: &Kind,
    mask: Option<&(ObjectRef, Object)>,
    target: Option<(u32, u32)>,
    output: Output,
    cancel: &CancelCheck,
) -> Result<Applied> {
    let Object::Stream { dict, .. } = object else {
        return Err(Error::Image("image XObject is not a stream".to_string()));
    };
    let convert_cmyk = matches!(
        output,
        Output::Jpeg {
            convert_cmyk: true,
            ..
        }
    );
    let shape = match kind.color {
        ColorModel::Bilevel => Shape::Bilevel,
        ColorModel::Gray => Shape::Gray,
        ColorModel::Cmyk if !convert_cmyk => Shape::Cmyk,
        _ => Shape::Rgb,
    };
    let source = Source::open(doc, object, kind, shape, &HashMap::new(), target)?;
    let mut pipe = Pipe::new(doc, cancel, source, shape, target);
    let (w, h) = (pipe.width, pipe.height);

    let mut new_dict = dict.clone();
    new_dict.remove("Decode");
    new_dict.remove("DecodeParms");
    new_dict.remove("Alternates");
    new_dict.insert("Width".to_string(), Object::Integer(w as i64));
    new_dict.insert("Height".to_string(), Object::Integer(h as i64));
    let bytes = match output {
        Output::Jpeg {
            quality,
            convert_cmyk,
            subsampling,
        } => {
            let bytes = encode_jpeg_rows(w, h, shape.channels(), quality, subsampling, &mut |row| {
                pipe.pull(row)
            })?;
            new_dict.insert("Filter".to_string(), Object::Name("DCTDecode".to_string()));
            new_dict.insert("BitsPerComponent".to_string(), Object::Integer(8));
            if convert_cmyk {
                new_dict.insert("ColorSpace".to_string(), Object::Name("DeviceRGB".to_string()));
            } else if kind.indexed {
                new_dict.insert("ColorSpace".to_string(), indexed_base(doc, dict)?);
            }
            bytes
        },
        Output::FlatePredicted => {
            let encoded = pipe.flate()?;
            new_dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
            new_dict.insert("DecodeParms".to_string(), predictor_parms(&encoded));
            new_dict.insert("BitsPerComponent".to_string(), Object::Integer(8));
            if kind.indexed {
                new_dict.insert("ColorSpace".to_string(), indexed_base(doc, dict)?);
            }
            encoded.bytes
        },
        Output::CcittG4 => {
            let bytes = pipe.ccitt()?;
            new_dict.insert("Filter".to_string(), Object::Name("CCITTFaxDecode".to_string()));
            new_dict.insert("DecodeParms".to_string(), ccitt_parms(w, h));
            new_dict.insert("BitsPerComponent".to_string(), Object::Integer(1));
            bytes
        },
    };
    new_dict.insert("Length".to_string(), Object::Integer(bytes.len() as i64));
    let mut total = bytes.len() as u64;
    let parent = Object::stream(new_dict, Bytes::from(bytes));

    let mask = match mask {
        Some((mask_ref, mask_obj)) => {
            let (obj, len) = rewrite_soft_mask(doc, cancel, mask_obj, w, h)?;
            total += len;
            Some((*mask_ref, obj))
        },
        None => None,
    };
    Ok(Applied {
        parent,
        mask,
        bytes: total,
        width: w,
        height: h,
    })
}

/// Read the soft mask's rows at the parent's new size and store them as
/// Flate-predicted 8-bit gray. Returns the object and its stored length.
fn rewrite_soft_mask(
    doc: &PdfDocument,
    cancel: &CancelCheck,
    mask: &Object,
    w: u32,
    h: u32,
) -> Result<(Object, u64)> {
    let Object::Stream { dict, .. } = mask else {
        return Err(Error::Image("soft mask is not a stream".to_string()));
    };
    let rows = ImageRows::open(doc, mask, &HashMap::new(), Some((w, h)), RowOutput::Luma, None)?;
    let source = Source::Luma { rows, invert: false };
    let encoded = Pipe::new(doc, cancel, source, Shape::Gray, Some((w, h))).flate()?;
    let mut new_dict = dict.clone();
    new_dict.remove("Decode");
    new_dict.insert("Width".to_string(), Object::Integer(w as i64));
    new_dict.insert("Height".to_string(), Object::Integer(h as i64));
    new_dict.insert("BitsPerComponent".to_string(), Object::Integer(8));
    new_dict.insert("ColorSpace".to_string(), Object::Name("DeviceGray".to_string()));
    new_dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
    new_dict.insert("DecodeParms".to_string(), predictor_parms(&encoded));
    new_dict.insert("Length".to_string(), Object::Integer(encoded.bytes.len() as i64));
    let len = encoded.bytes.len() as u64;
    Ok((
        Object::stream(new_dict, Bytes::from(encoded.bytes)),
        len,
    ))
}

/// The base of an `[/Indexed base hival lookup]` colour space, as written.
fn indexed_base(doc: &PdfDocument, dict: &HashMap<String, Object>) -> Result<Object> {
    let cs = dict
        .get("ColorSpace")
        .ok_or_else(|| Error::Image("indexed image without /ColorSpace".to_string()))?;
    match doc.resolve_object(cs)? {
        Object::Array(items) if items.len() >= 2 => Ok(items[1].clone()),
        _ => Err(Error::Image("indexed colour space is not an array".to_string())),
    }
}

fn predictor_parms(encoded: &super::encode::FlateEncoded) -> Object {
    let mut d = HashMap::new();
    d.insert("Predictor".to_string(), Object::Integer(encoded.predictor as i64));
    d.insert("Colors".to_string(), Object::Integer(encoded.colors as i64));
    d.insert("BitsPerComponent".to_string(), Object::Integer(8));
    d.insert("Columns".to_string(), Object::Integer(encoded.columns as i64));
    Object::Dictionary(d)
}

fn ccitt_parms(w: u32, h: u32) -> Object {
    let mut d = HashMap::new();
    d.insert("K".to_string(), Object::Integer(-1));
    d.insert("Columns".to_string(), Object::Integer(w as i64));
    d.insert("Rows".to_string(), Object::Integer(h as i64));
    d.insert("BlackIs1".to_string(), Object::Boolean(false));
    Object::Dictionary(d)
}
