//! Carry out a decision: decode, resample, encode, rewrite the dictionary,
//! stage the replacement — and the driver that runs the five stages over a
//! document. One image (plus its soft mask) is in memory at a time.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).

use std::collections::HashMap;

use bytes::Bytes;

use crate::decoders::CcittParams;
use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::error::{Error, Result};
use crate::extractors::images::decode_cmyk_jpeg_to_raw_cmyk;
use crate::extractors::{extract_image_from_xobject, ImageData, PixelFormat};
use crate::object::{Object, ObjectRef};

use super::classify::{classify, filter_names, ColorModel, Encoding, Kind};
use super::encode::{encode_ccitt_g4, encode_flate_predicted, encode_jpeg};
use super::inventory::document_images;
use super::policy::{decide, ppi_min, Decision, KeepReason, Output, Policy, Use};
use super::report::{Action, Outcome, Report};
use super::resample::{resample_bilevel, resample_continuous};

/// Run the pipeline over every image drawn on the editor's visible pages
/// and stage each replacement for the next save.
pub fn reduce_images(editor: &mut DocumentEditor, policy: &Policy) -> Result<Report> {
    let pages: Vec<usize> = editor
        .page_order_visible()
        .iter()
        .map(|&i| i as usize)
        .collect();
    let entries = document_images(editor.source(), &pages)?;
    let mut report = Report::default();
    for entry in entries {
        let object = staged_or_source(editor, entry.object)?;
        let Object::Stream { dict, data } = &object else {
            continue;
        };
        let kind = classify(editor.source(), dict, data);
        let uses: Vec<Use> = entry.placements.iter().map(|&ctm| Use { ctm }).collect();
        let mask_before = match kind.masks.soft_mask {
            Some(r) => stream_len(editor, r)?,
            None => 0,
        };
        let bytes_before = data.len() as u64 + mask_before;
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
                    Some(r) => Some((r, staged_or_source(editor, r)?)),
                    None => None,
                };
                match apply(
                    editor.source(),
                    &object,
                    entry.object,
                    &kind,
                    mask.as_ref(),
                    target,
                    output,
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
                        editor.insert_modified(entry.object.id, applied.parent);
                        if let Some((r, obj)) = applied.mask {
                            editor.insert_modified(r.id, obj);
                        }
                    },
                    Ok(_) => outcome.keep_reason = Some(KeepReason::BelowMinSavings),
                    Err(_) => outcome.keep_reason = Some(KeepReason::Undecodable),
                }
            },
        }
        report.images.push(outcome);
    }
    Ok(report)
}

/// Strictly smaller, and by at least `min_savings` of the stored bytes.
fn saves_enough(after: u64, before: u64, min_savings: f64) -> bool {
    after < before && (before - after) as f64 >= before as f64 * min_savings.clamp(0.0, 1.0)
}

/// The object as this session sees it: a staged replacement wins over the source.
fn staged_or_source(editor: &mut DocumentEditor, r: ObjectRef) -> Result<Object> {
    if let Some(staged) = editor.modified_objects_mut().get(&r.id) {
        return Ok(staged.clone());
    }
    editor.source().load_object(r)
}

fn stream_len(editor: &mut DocumentEditor, r: ObjectRef) -> Result<u64> {
    Ok(match staged_or_source(editor, r)? {
        Object::Stream { data, .. } => data.len() as u64,
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

/// Decoded samples in the working form the encoders take.
struct Samples {
    data: Vec<u8>,
    /// 1 (gray or bilevel 0/255), 3 (RGB) or 4 (CMYK).
    channels: u8,
    /// Samples are 0/255 only; resampling thresholds instead of blending.
    bilevel: bool,
    width: u32,
    height: u32,
}

fn apply(
    doc: &PdfDocument,
    object: &Object,
    r: ObjectRef,
    kind: &Kind,
    mask: Option<&(ObjectRef, Object)>,
    target: Option<(u32, u32)>,
    output: Output,
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
    let mut samples = decode(doc, object, r, kind, convert_cmyk)?;
    if let Some((tw, th)) = target {
        samples = resample(samples, tw, th)?;
    }
    let (w, h) = (samples.width, samples.height);

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
            let bytes = encode_jpeg(&samples.data, w, h, samples.channels, quality, subsampling)?;
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
            let encoded = encode_flate_predicted(&samples.data, w, h, samples.channels)?;
            new_dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
            new_dict.insert("DecodeParms".to_string(), predictor_parms(&encoded));
            new_dict.insert("BitsPerComponent".to_string(), Object::Integer(8));
            if kind.indexed {
                new_dict.insert("ColorSpace".to_string(), indexed_base(doc, dict)?);
            }
            encoded.bytes
        },
        Output::CcittG4 => {
            let bytes = encode_ccitt_g4(&samples.data, w, h)?;
            new_dict.insert("Filter".to_string(), Object::Name("CCITTFaxDecode".to_string()));
            new_dict.insert("DecodeParms".to_string(), ccitt_parms(w, h));
            new_dict.insert("BitsPerComponent".to_string(), Object::Integer(1));
            bytes
        },
    };
    new_dict.insert("Length".to_string(), Object::Integer(bytes.len() as i64));
    let mut total = bytes.len() as u64;
    let parent = Object::Stream {
        dict: new_dict,
        data: Bytes::from(bytes),
    };

    let mask = match mask {
        Some((mask_ref, mask_obj)) => {
            let (obj, len) = rewrite_soft_mask(doc, mask_obj, *mask_ref, w, h)?;
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

/// Decode to the working form. Continuous-tone images go through the
/// extractor (Indexed expanded, `/Decode` folded, 16-bit collapsed, CMYK
/// JPEG through the Adobe-aware path); bilevel images are unpacked here
/// because the extractor has no path for a stencil mask.
fn decode(
    doc: &PdfDocument,
    object: &Object,
    r: ObjectRef,
    kind: &Kind,
    convert_cmyk: bool,
) -> Result<Samples> {
    if kind.color == ColorModel::Bilevel {
        return decode_bilevel(doc, object, kind);
    }
    let image = extract_image_from_xobject(Some(doc), object, Some(r), None)?;
    if kind.color == ColorModel::Cmyk && !convert_cmyk {
        let (data, width, height) = match image.data() {
            ImageData::Raw {
                pixels,
                format: PixelFormat::CMYK,
            } => (pixels.clone(), image.width(), image.height()),
            ImageData::Raw { .. } => {
                return Err(Error::Image("CMYK image decoded to another model".to_string()))
            },
            ImageData::Jpeg(bytes) => {
                (decode_cmyk_jpeg_to_raw_cmyk(bytes)?, image.width(), image.height())
            },
        };
        return checked(Samples {
            data,
            channels: 4,
            bilevel: false,
            width,
            height,
        });
    }
    let dynamic = image.to_dynamic_image()?;
    let (width, height) = (dynamic.width(), dynamic.height());
    match kind.color {
        ColorModel::Gray => checked(Samples {
            data: dynamic.to_luma8().into_raw(),
            channels: 1,
            bilevel: false,
            width,
            height,
        }),
        _ => checked(Samples {
            data: dynamic.to_rgb8().into_raw(),
            channels: 3,
            bilevel: false,
            width,
            height,
        }),
    }
}

fn checked(s: Samples) -> Result<Samples> {
    let expected = s.width as usize * s.height as usize * s.channels as usize;
    if s.width == 0 || s.height == 0 || s.data.len() != expected {
        return Err(Error::Image(format!(
            "decoded {} bytes for {}x{}x{}",
            s.data.len(),
            s.width,
            s.height,
            s.channels
        )));
    }
    Ok(s)
}

/// Unpack a 1-bit image to one byte per pixel, 0 = black (or "paint" for a
/// stencil mask) and 255 = white, with `/Decode [1 0]` applied.
///
/// Both DeviceGray and a stencil mask read a 0 bit as black/paint under the
/// default `/Decode`. A CCITT stream is decoded first; upstream's decoder
/// returns 1 = black (0 = black when `/BlackIs1 true`), which is the
/// complement of the filter's own output bits in both cases.
fn decode_bilevel(doc: &PdfDocument, object: &Object, kind: &Kind) -> Result<Samples> {
    let Object::Stream { dict, data } = object else {
        return Err(Error::Image("image XObject is not a stream".to_string()));
    };
    let (width, height) = (kind.width, kind.height);
    let row_bytes = (width as usize).div_ceil(8);
    let (packed, complement) = match kind.encoding {
        Encoding::Raw => (object.decode_stream_data()?, false),
        Encoding::Ccitt => {
            let filters = filter_names(doc, dict);
            if filters.len() != 1 {
                return Err(Error::Image("CCITT behind another filter".to_string()));
            }
            let params = ccitt_params_of(doc, dict, width, height);
            (crate::extractors::ccitt_bilevel::decompress_ccitt(data, &params)?, true)
        },
        _ => return Err(Error::Image("bilevel image in a codec without a decoder".to_string())),
    };
    if packed.len() < row_bytes * height as usize {
        return Err(Error::Image("bilevel data shorter than its dimensions".to_string()));
    }
    let inverted = dict
        .get("Decode")
        .and_then(|d| doc.resolve_object(d).ok())
        .and_then(|d| {
            d.as_array()
                .and_then(|a| a.first().and_then(|v| v.as_real()))
        })
        == Some(1.0);
    let mut out = Vec::with_capacity(width as usize * height as usize);
    for row in 0..height as usize {
        for col in 0..width as usize {
            let byte = packed[row * row_bytes + col / 8];
            let mut bit = (byte >> (7 - (col % 8))) & 1;
            if complement {
                bit ^= 1;
            }
            if inverted {
                bit ^= 1;
            }
            out.push(if bit == 0 { 0 } else { 255 });
        }
    }
    Ok(Samples {
        data: out,
        channels: 1,
        bilevel: true,
        width,
        height,
    })
}

fn ccitt_params_of(
    doc: &PdfDocument,
    dict: &HashMap<String, Object>,
    width: u32,
    height: u32,
) -> CcittParams {
    let parms = dict
        .get("DecodeParms")
        .and_then(|p| doc.resolve_object(p).ok())
        .and_then(|p| match p {
            Object::Dictionary(d) => Some(d),
            Object::Array(items) => items
                .last()
                .and_then(|o| doc.resolve_object(o).ok())
                .and_then(|o| o.as_dict().cloned()),
            _ => None,
        })
        .unwrap_or_default();
    let int = |key: &str| parms.get(key).and_then(|o| o.as_integer());
    let flag = |key: &str| parms.get(key).and_then(|o| o.as_bool());
    CcittParams {
        k: int("K").unwrap_or(0),
        columns: int("Columns").map_or(width.max(1), |c| c.max(1) as u32),
        rows: Some(int("Rows").map_or(height, |r| r.max(0) as u32)),
        black_is_1: flag("BlackIs1").unwrap_or(false),
        end_of_line: flag("EndOfLine").unwrap_or(false),
        encoded_byte_align: flag("EncodedByteAlign").unwrap_or(false),
        end_of_block: flag("EndOfBlock").unwrap_or(true),
    }
}

fn resample(s: Samples, tw: u32, th: u32) -> Result<Samples> {
    if tw == s.width && th == s.height {
        return Ok(s);
    }
    let data = if s.bilevel {
        resample_bilevel(&s.data, s.width, s.height, tw, th)?
    } else {
        resample_continuous(&s.data, s.width, s.height, s.channels, tw, th)?
    };
    Ok(Samples {
        data,
        channels: s.channels,
        bilevel: s.bilevel,
        width: tw,
        height: th,
    })
}

/// Decode the soft mask, bring it to the parent's new size, store it as
/// Flate-predicted 8-bit gray. Returns the object and its stored length.
fn rewrite_soft_mask(
    doc: &PdfDocument,
    mask: &Object,
    r: ObjectRef,
    w: u32,
    h: u32,
) -> Result<(Object, u64)> {
    let Object::Stream { dict, .. } = mask else {
        return Err(Error::Image("soft mask is not a stream".to_string()));
    };
    let image = extract_image_from_xobject(Some(doc), mask, Some(r), None)?;
    let dynamic = image.to_dynamic_image()?;
    let (mw, mh) = (dynamic.width(), dynamic.height());
    let mut gray = dynamic.to_luma8().into_raw();
    if (mw, mh) != (w, h) {
        gray = resample_continuous(&gray, mw, mh, 1, w, h)?;
    }
    let encoded = encode_flate_predicted(&gray, w, h, 1)?;
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
        Object::Stream {
            dict: new_dict,
            data: Bytes::from(encoded.bytes),
        },
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

