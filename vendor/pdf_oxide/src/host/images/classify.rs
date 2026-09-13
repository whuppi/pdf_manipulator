//! What an image is: how its samples are stored, what they mean, and which
//! masks hang off it. Read from the image dictionary only — never from a
//! sample — so an object staged earlier in the same session classifies by
//! what it is now, not by what the source file said.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).

use std::collections::HashMap;

use crate::document::PdfDocument;
use crate::extractors::images::PdfFilter;
use crate::object::{Object, ObjectRef};

/// The last filter in the chain — the one that decides whether the stored
/// bytes are a lossless container or a codec's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Stored losslessly: no filter, or only Flate / LZW / RunLength / ASCII / Crypt filters.
    Raw,
    /// `DCTDecode`.
    Jpeg,
    /// `JPXDecode`.
    Jpx,
    /// `CCITTFaxDecode`.
    Ccitt,
    /// `JBIG2Decode`.
    Jbig2,
    /// A filter name this crate does not know.
    Unknown,
}

/// What the decoded samples mean, after the extractor has expanded an
/// Indexed palette to its base and unpacked sub-byte samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorModel {
    /// A stencil mask (`/ImageMask true`) or a 1-bit gray image.
    Bilevel,
    /// One component: DeviceGray, CalGray, ICCBased N=1.
    Gray,
    /// Three components: DeviceRGB, CalRGB, ICCBased N=3.
    Rgb,
    /// Four components: DeviceCMYK, ICCBased N=4.
    Cmyk,
    /// Lab, Separation, DeviceN, Pattern, or a base that could not be parsed.
    Other,
}

/// The masks attached to an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Masks {
    /// `/SMask` stream; co-processed with the parent.
    pub soft_mask: Option<ObjectRef>,
    /// `/Mask` is a stream (a stencil).
    pub stencil_mask: bool,
    /// `/Mask` is an array (colour-key masking over stored sample ranges).
    pub color_key_mask: bool,
    /// `/SMaskInData` > 0 (JPX-only opacity channel).
    pub smask_in_data: bool,
}

/// The classification of one image XObject.
#[derive(Debug, Clone, PartialEq)]
pub struct Kind {
    /// How the samples are stored.
    pub encoding: Encoding,
    /// What the decoded samples mean.
    pub color: ColorModel,
    /// The stored samples are palette indices (the colour model is the base's).
    pub indexed: bool,
    /// `/ImageMask true`.
    pub image_mask: bool,
    /// Bits per component as stored.
    pub bits: u8,
    /// Masks attached to the image.
    pub masks: Masks,
    /// Pixel width as stored.
    pub width: u32,
    /// Pixel height as stored.
    pub height: u32,
    /// Stream bytes as stored (filters applied).
    pub compressed_len: u64,
    /// For a stored JPEG, the quality its luminance quantization table
    /// encodes on the 1–100 scale; `None` when the table is not the
    /// standard one scaled, or the stream is not a plain JPEG.
    pub jpeg_quality: Option<u8>,
}

/// Why an image is outside what the pipeline re-encodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// JPEG 2000: no decoder in this build, no encoder.
    Jpx,
    /// JBIG2: no encoder.
    Jbig2,
    /// A filter this crate cannot undo.
    UnknownFilter,
    /// Lab, Separation, DeviceN, Pattern or an unparsable base.
    OtherColor,
    /// A `/Mask` stream would need co-processing this version does not do.
    StencilMask,
    /// `/Mask` sample ranges do not survive re-encoding.
    ColorKeyMask,
    /// A JPX opacity channel.
    SmaskInData,
}

impl Kind {
    /// `None` when the pipeline can re-encode this image.
    pub fn unsupported(&self) -> Option<Unsupported> {
        match self.encoding {
            Encoding::Jpx => return Some(Unsupported::Jpx),
            Encoding::Jbig2 => return Some(Unsupported::Jbig2),
            Encoding::Unknown => return Some(Unsupported::UnknownFilter),
            Encoding::Raw | Encoding::Jpeg | Encoding::Ccitt => {},
        }
        if self.color == ColorModel::Other {
            return Some(Unsupported::OtherColor);
        }
        if self.masks.smask_in_data {
            return Some(Unsupported::SmaskInData);
        }
        if self.masks.stencil_mask {
            return Some(Unsupported::StencilMask);
        }
        if self.masks.color_key_mask {
            return Some(Unsupported::ColorKeyMask);
        }
        None
    }
}

/// Classify an image XObject from its dictionary and stored bytes.
pub fn classify(doc: &PdfDocument, dict: &HashMap<String, Object>, data: &[u8]) -> Kind {
    let filters = filter_names(doc, dict);
    let encoding = match filters.last().map(|n| PdfFilter::from_name(n)) {
        None
        | Some(PdfFilter::FlateDecode)
        | Some(PdfFilter::LZWDecode)
        | Some(PdfFilter::RunLengthDecode)
        | Some(PdfFilter::ASCIIHexDecode)
        | Some(PdfFilter::ASCII85Decode)
        | Some(PdfFilter::Crypt) => Encoding::Raw,
        Some(PdfFilter::DCTDecode) => Encoding::Jpeg,
        Some(PdfFilter::JPXDecode) => Encoding::Jpx,
        Some(PdfFilter::CCITTFaxDecode) => Encoding::Ccitt,
        Some(PdfFilter::JBIG2Decode) => Encoding::Jbig2,
        Some(PdfFilter::Other(_)) => Encoding::Unknown,
    };
    let int = |key: &str| -> Option<i64> {
        dict.get(key)
            .and_then(|o| doc.resolve_object(o).ok())
            .and_then(|o| o.as_integer())
    };
    let image_mask = dict.get("ImageMask").and_then(|o| o.as_bool()) == Some(true);
    let bits = int("BitsPerComponent")
        .unwrap_or(if image_mask { 1 } else { 8 })
        .clamp(1, 16) as u8;
    let space = dict
        .get("ColorSpace")
        .map(|cs| color_space_of(doc, cs, 0))
        .unwrap_or(Space {
            model: ColorModel::Other,
            indexed: false,
        });
    let color = if image_mask {
        ColorModel::Bilevel
    } else if space.model == ColorModel::Gray && bits == 1 && !space.indexed {
        // A 1-bit palette image expands to its palette colours; only a
        // direct 1-bit gray image is bilevel.
        ColorModel::Bilevel
    } else {
        space.model
    };
    Kind {
        encoding,
        color,
        indexed: space.indexed,
        image_mask,
        bits,
        masks: masks(doc, dict),
        width: int("Width").unwrap_or(0).max(0) as u32,
        height: int("Height").unwrap_or(0).max(0) as u32,
        compressed_len: data.len() as u64,
        // Only a bare DCT stream is a JPEG we can read the tables of.
        jpeg_quality: if encoding == Encoding::Jpeg && filters.len() == 1 {
            estimate_jpeg_quality(data)
        } else {
            None
        },
    }
}

/// The luminance quantization table of ITU-T T.81 Annex K, in natural
/// order. The sum is what the estimate uses, so the order is irrelevant.
const ANNEX_K_LUMA: [u32; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// The quality (1–100) a JPEG's luminance quantization table encodes,
/// assuming the common encoders' scaling of the Annex K table
/// (`scale = q < 50 ? 5000 / q : 200 − 2q`, entries `(base × scale + 50) / 100`).
/// Inverts that from the table sum; `None` when no DQT table 0 is found.
pub fn estimate_jpeg_quality(jpeg: &[u8]) -> Option<u8> {
    let mut i = 2usize; // past SOI
    while i + 4 <= jpeg.len() {
        if jpeg[i] != 0xFF {
            return None;
        }
        let marker = jpeg[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if marker == 0xDA || marker == 0xD9 {
            return None; // start of scan / end of image without a table 0
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if len < 2 || i + 2 + len > jpeg.len() {
            return None;
        }
        if marker == 0xDB {
            let mut p = i + 4;
            let end = i + 2 + len;
            while p < end {
                let precision16 = jpeg[p] >> 4 == 1;
                let id = jpeg[p] & 0x0F;
                let entry = if precision16 { 2 } else { 1 };
                if p + 1 + 64 * entry > end {
                    return None;
                }
                if id == 0 {
                    let sum: u32 = (0..64)
                        .map(|k| {
                            let at = p + 1 + k * entry;
                            if precision16 {
                                u16::from_be_bytes([jpeg[at], jpeg[at + 1]]) as u32
                            } else {
                                jpeg[at] as u32
                            }
                        })
                        .sum();
                    let base: u32 = ANNEX_K_LUMA.iter().sum();
                    // scale in percent, from the table sum ratio.
                    let scale = (sum as f64 * 100.0 / base as f64).round().max(1.0);
                    let quality = if scale <= 100.0 { (200.0 - scale) / 2.0 } else { 5000.0 / scale };
                    return Some(quality.round().clamp(1.0, 100.0) as u8);
                }
                p += 1 + 64 * entry;
            }
        }
        i += 2 + len;
    }
    None
}


/// `/Filter` as a list of names (a single name, an array, or absent).
pub fn filter_names(doc: &PdfDocument, dict: &HashMap<String, Object>) -> Vec<String> {
    let Some(filter) = dict.get("Filter") else {
        return Vec::new();
    };
    match doc.resolve_object(filter).unwrap_or(Object::Null) {
        Object::Name(n) => vec![n],
        Object::Array(items) => items
            .iter()
            .filter_map(|o| o.as_name().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

struct Space {
    model: ColorModel,
    indexed: bool,
}

/// The colour model of a `/ColorSpace` value; Indexed resolves to its base.
fn color_space_of(doc: &PdfDocument, value: &Object, depth: u8) -> Space {
    let other = Space {
        model: ColorModel::Other,
        indexed: false,
    };
    // A colour space nests at most Indexed → ICCBased in practice; the bound
    // keeps a self-referencing file finite.
    if depth > 4 {
        return other;
    }
    let resolved = match doc.resolve_object(value) {
        Ok(o) => o,
        Err(_) => return other,
    };
    match resolved {
        Object::Name(n) => Space {
            model: model_of_name(&n),
            indexed: false,
        },
        Object::Array(items) => {
            let Some(family) = items.first().and_then(|o| o.as_name()) else {
                return other;
            };
            match family {
                "ICCBased" => {
                    let n = items
                        .get(1)
                        .and_then(|s| doc.resolve_object(s).ok())
                        .and_then(|s| {
                            s.as_dict()
                                .and_then(|d| d.get("N").and_then(|n| n.as_integer()))
                        });
                    let model = match n {
                        Some(1) => ColorModel::Gray,
                        Some(3) => ColorModel::Rgb,
                        Some(4) => ColorModel::Cmyk,
                        _ => ColorModel::Other,
                    };
                    Space {
                        model,
                        indexed: false,
                    }
                },
                "Indexed" | "I" => {
                    let Some(base) = items.get(1) else {
                        return other;
                    };
                    Space {
                        model: color_space_of(doc, base, depth + 1).model,
                        indexed: true,
                    }
                },
                "CalGray" => Space {
                    model: ColorModel::Gray,
                    indexed: false,
                },
                "CalRGB" => Space {
                    model: ColorModel::Rgb,
                    indexed: false,
                },
                // A one-element array wrapping a device name is legal.
                "DeviceGray" | "DeviceRGB" | "DeviceCMYK" | "G" | "RGB" | "CMYK"
                    if items.len() == 1 =>
                {
                    Space {
                        model: model_of_name(family),
                        indexed: false,
                    }
                },
                _ => other,
            }
        },
        _ => other,
    }
}

fn model_of_name(name: &str) -> ColorModel {
    match name {
        "DeviceGray" | "G" | "CalGray" => ColorModel::Gray,
        "DeviceRGB" | "RGB" | "CalRGB" => ColorModel::Rgb,
        "DeviceCMYK" | "CMYK" => ColorModel::Cmyk,
        _ => ColorModel::Other,
    }
}

fn masks(doc: &PdfDocument, dict: &HashMap<String, Object>) -> Masks {
    let mut masks = Masks {
        soft_mask: dict.get("SMask").and_then(|o| o.as_reference()),
        ..Masks::default()
    };
    if let Some(mask) = dict.get("Mask") {
        match doc.resolve_object(mask).unwrap_or(Object::Null) {
            Object::Array(_) => masks.color_key_mask = true,
            Object::Stream { .. } => masks.stencil_mask = true,
            _ => {},
        }
    }
    masks.smask_in_data = dict
        .get("SMaskInData")
        .and_then(|o| o.as_integer())
        .unwrap_or(0)
        > 0;
    masks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::images::encode::encode_jpeg;
    use crate::host::images::policy::Subsampling;

    #[test]
    fn estimated_quality_round_trips_the_encoder() {
        let (w, h) = (16u32, 16u32);
        let gray: Vec<u8> = (0..w * h).map(|i| (i * 7 % 256) as u8).collect();
        for q in [20u8, 50, 75, 90, 95] {
            let jpeg = encode_jpeg(&gray, w, h, 1, q, Subsampling::Half).unwrap();
            let got = estimate_jpeg_quality(&jpeg).unwrap();
            assert!((got as i32 - q as i32).abs() <= 1, "q {q} estimated {got}");
        }
        assert_eq!(estimate_jpeg_quality(b"not a jpeg"), None);
    }
}
