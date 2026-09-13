//! What to do with an image: a pure decision over its classification, the
//! places it is drawn, and the caller's policy. No I/O, no pixels.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).

use crate::content::Matrix;

use super::classify::{ColorModel, Encoding, Kind, Unsupported};

/// How far the pipeline may go. Mirrors `PdfImagePolicy` on the Dart side.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    /// Target resolution for RGB and CMYK images; `None` never downsamples them.
    pub color_ppi: Option<f64>,
    /// Target resolution for gray images.
    pub gray_ppi: Option<f64>,
    /// Target resolution for bilevel images.
    pub mono_ppi: Option<f64>,
    /// Downsample only when the effective resolution exceeds target × threshold.
    pub downsample_threshold: f64,
    /// Quality for every JPEG the pipeline writes.
    pub jpeg_quality: u8,
    /// May a losslessly stored continuous-tone image become a JPEG.
    pub allow_lossy: bool,
    /// CMYK output becomes RGB.
    pub convert_cmyk_to_rgb: bool,
    /// Images narrower or shorter than this are kept.
    pub min_pixels: u32,
    /// A re-encode is written only when it saves at least this fraction of
    /// the stored bytes (parent plus soft mask); 0 means any reduction.
    /// Guards trading quality for a few percent.
    pub min_savings: f64,
    /// Chroma subsampling for the JPEGs written.
    pub chroma: Chroma,
    /// Re-encode a stored JPEG at `jpeg_quality` even when it is not
    /// downsampled. Off, a JPEG changes only when its pixels do; on,
    /// `min_savings` still decides whether the result is worth keeping.
    pub recompress_jpeg: bool,
}

/// How chroma is subsampled in a written JPEG. `Auto` follows quality:
/// 4:2:0 below 90, 4:4:4 from 90; a preset may pin either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chroma {
    /// 4:4:4 when `jpeg_quality` ≥ 90, 4:2:0 below.
    Auto,
    /// 4:4:4 — every chroma sample kept; text and hard colour edges stay crisp.
    Full,
    /// 4:2:0 — chroma at half resolution both ways; smallest, fine for photos.
    Half,
}

/// The resolved subsampling an encoder is told to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsampling {
    /// 4:4:4.
    Full,
    /// 4:2:0.
    Half,
}

impl Policy {
    /// The subsampling this policy writes.
    pub fn subsampling(&self) -> Subsampling {
        match self.chroma {
            Chroma::Full => Subsampling::Full,
            Chroma::Half => Subsampling::Half,
            Chroma::Auto if self.jpeg_quality >= 90 => Subsampling::Full,
            Chroma::Auto => Subsampling::Half,
        }
    }
}

/// One placement of an image: the CTM in effect at its `Do`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Use {
    /// The CTM at the `Do`, enclosing forms composed in.
    pub ctm: Matrix,
}

/// The effective resolution of one placement: pixels per inch along the
/// tighter axis. `None` when the CTM collapses the image.
pub fn use_ppi(width: u32, height: u32, ctm: &Matrix) -> Option<f64> {
    let side_x = (ctm.a as f64).hypot(ctm.b as f64);
    let side_y = (ctm.c as f64).hypot(ctm.d as f64);
    if side_x < 1e-6 || side_y < 1e-6 {
        return None;
    }
    let ppi_x = width as f64 * 72.0 / side_x;
    let ppi_y = height as f64 * 72.0 / side_y;
    Some(ppi_x.min(ppi_y))
}

/// The most demanding placement: the lowest effective resolution.
pub fn ppi_min(kind: &Kind, uses: &[Use]) -> Option<f64> {
    uses.iter()
        .filter_map(|u| use_ppi(kind.width, kind.height, &u.ctm))
        .fold(None, |acc, p| Some(acc.map_or(p, |a: f64| a.min(p))))
}

/// The codec the pipeline writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Output {
    /// Gray or RGB JPEG.
    Jpeg {
        /// Encoder quality, 1–100.
        quality: u8,
        /// CMYK samples become RGB before encoding.
        convert_cmyk: bool,
        /// Chroma subsampling to write.
        subsampling: Subsampling,
    },
    /// Flate with PNG predictors, 8 bits, same colour model.
    FlatePredicted,
    /// CCITT Group 4, 1 bit.
    CcittG4,
}

/// Why an image is left as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepReason {
    /// Outside what the pipeline re-encodes.
    Unsupported(Unsupported),
    /// Narrower or shorter than `Policy::min_pixels`.
    TooSmall,
    /// No placement with a usable CTM.
    NoPlacement,
    /// A target resolution exists and the image is at or under it.
    WithinResolution,
    /// Nothing was asked that could make it smaller.
    AlreadyOptimal,
    /// Re-encoding did not save `Policy::min_savings` of the stored bytes.
    BelowMinSavings,
    /// The stored samples could not be decoded.
    Undecodable,
}

/// What the pipeline does with one image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    /// Leave the object as stored.
    Keep(KeepReason),
    /// Decode, optionally resample, encode, replace.
    Reencode {
        /// New pixel size when downsampling; `None` keeps the size.
        target: Option<(u32, u32)>,
        /// The codec to write.
        output: Output,
    },
}

fn target_ppi(kind: &Kind, policy: &Policy) -> Option<f64> {
    match kind.color {
        ColorModel::Bilevel => policy.mono_ppi,
        ColorModel::Gray => policy.gray_ppi,
        ColorModel::Rgb | ColorModel::Cmyk => policy.color_ppi,
        ColorModel::Other => None,
    }
}

fn output_for(kind: &Kind, policy: &Policy) -> Output {
    match kind.color {
        ColorModel::Bilevel => Output::CcittG4,
        ColorModel::Gray | ColorModel::Rgb if policy.allow_lossy => Output::Jpeg {
            quality: policy.jpeg_quality,
            convert_cmyk: false,
            subsampling: policy.subsampling(),
        },
        ColorModel::Cmyk if policy.allow_lossy && policy.convert_cmyk_to_rgb => Output::Jpeg {
            quality: policy.jpeg_quality,
            convert_cmyk: true,
            subsampling: policy.subsampling(),
        },
        _ => Output::FlatePredicted,
    }
}

/// The decision table. Rows are evaluated top to bottom; the first match wins.
pub fn decide(kind: &Kind, uses: &[Use], policy: &Policy) -> Decision {
    if let Some(u) = kind.unsupported() {
        return Decision::Keep(KeepReason::Unsupported(u));
    }
    if kind.width < policy.min_pixels || kind.height < policy.min_pixels {
        return Decision::Keep(KeepReason::TooSmall);
    }
    let Some(ppi) = ppi_min(kind, uses) else {
        return Decision::Keep(KeepReason::NoPlacement);
    };
    if let Some(target) = target_ppi(kind, policy) {
        if ppi > target * policy.downsample_threshold {
            let scale = target / ppi;
            let new_w = ((kind.width as f64 * scale).round() as u32).max(1);
            let new_h = ((kind.height as f64 * scale).round() as u32).max(1);
            return Decision::Reencode {
                target: Some((new_w, new_h)),
                output: output_for(kind, policy),
            };
        }
    }
    // Not downsampled from here on. "Within resolution" when a target existed
    // and the image sits under it; "already optimal" when nothing was asked.
    let kept = if target_ppi(kind, policy).is_some() {
        KeepReason::WithinResolution
    } else {
        KeepReason::AlreadyOptimal
    };
    match kind.encoding {
        // A JPEG re-encoded in place only loses another generation, so it
        // happens only when the policy asks, a lossy codec is allowed, and
        // the stored quality is above the target — a JPEG already at or
        // below it is done, which is what makes a second pass a no-op.
        Encoding::Jpeg if policy.recompress_jpeg && policy.allow_lossy => {
            return match kind.jpeg_quality {
                Some(q) if q <= policy.jpeg_quality => Decision::Keep(KeepReason::AlreadyOptimal),
                _ => Decision::Reencode {
                    target: None,
                    output: output_for(kind, policy),
                },
            };
        },
        Encoding::Jpeg => return Decision::Keep(kept),
        Encoding::Ccitt if kind.color == ColorModel::Bilevel => return Decision::Keep(kept),
        _ => {},
    }
    // A palette image is already compact; only a lossy codec or a smaller
    // size can beat it, and neither applies here.
    if kind.indexed && !policy.allow_lossy {
        return Decision::Keep(KeepReason::AlreadyOptimal);
    }
    Decision::Reencode {
        target: None,
        output: output_for(kind, policy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::images::classify::Masks;

    fn kind(encoding: Encoding, color: ColorModel, bits: u8, width: u32, height: u32) -> Kind {
        Kind {
            encoding,
            color,
            indexed: false,
            image_mask: false,
            bits,
            masks: Masks::default(),
            width,
            height,
            compressed_len: 1000,
            jpeg_quality: None,
        }
    }

    /// A placement `pt` points wide and high, axis aligned.
    fn placed(pt: f32) -> Use {
        Use {
            ctm: Matrix {
                a: pt,
                b: 0.0,
                c: 0.0,
                d: pt,
                e: 10.0,
                f: 10.0,
            },
        }
    }

    fn screen() -> Policy {
        Policy {
            color_ppi: Some(72.0),
            gray_ppi: Some(72.0),
            mono_ppi: Some(300.0),
            downsample_threshold: 1.5,
            jpeg_quality: 60,
            allow_lossy: true,
            convert_cmyk_to_rgb: true,
            min_pixels: 32,
            min_savings: 0.0,
            chroma: Chroma::Auto,
            recompress_jpeg: false,
        }
    }

    fn lossless() -> Policy {
        Policy {
            color_ppi: None,
            gray_ppi: None,
            mono_ppi: None,
            allow_lossy: false,
            convert_cmyk_to_rgb: false,
            ..screen()
        }
    }

    #[test]
    fn effective_ppi_uses_the_tighter_axis_and_survives_rotation() {
        // 128 px over 32 pt = 288 ppi on both axes.
        assert_eq!(use_ppi(128, 128, &placed(32.0).ctm), Some(288.0));
        // Rotated 90°: a = 0, b = 32, c = -32, d = 0 — same sides.
        let rotated = Matrix {
            a: 0.0,
            b: 32.0,
            c: -32.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
        };
        assert_eq!(use_ppi(128, 128, &rotated), Some(288.0));
        // Anisotropic: 128 px over 64 pt horizontally (144) and 32 pt vertically (288) → 144.
        let wide = Matrix {
            a: 64.0,
            b: 0.0,
            c: 0.0,
            d: 32.0,
            e: 0.0,
            f: 0.0,
        };
        assert_eq!(use_ppi(128, 128, &wide), Some(144.0));
        assert_eq!(
            use_ppi(
                128,
                128,
                &Matrix {
                    a: 0.0,
                    b: 0.0,
                    c: 0.0,
                    d: 32.0,
                    e: 0.0,
                    f: 0.0
                }
            ),
            None
        );
    }

    #[test]
    fn the_most_demanding_placement_decides() {
        let k = kind(Encoding::Raw, ColorModel::Gray, 8, 32, 32);
        // 32 px at 32 pt (72 ppi) and at 8 pt (288 ppi): the 72 ppi use wins.
        assert_eq!(ppi_min(&k, &[placed(8.0), placed(32.0)]), Some(72.0));
        assert_eq!(
            decide(&k, &[placed(8.0), placed(32.0)], &screen()),
            Decision::Reencode {
                target: None,
                output: Output::Jpeg {
                    quality: 60,
                    convert_cmyk: false,
                    subsampling: Subsampling::Half,
                },
            }
        );
    }

    #[test]
    fn unsupported_kinds_are_kept_before_any_other_rule() {
        let mut k = kind(Encoding::Jpx, ColorModel::Rgb, 8, 4, 4);
        assert_eq!(
            decide(&k, &[], &screen()),
            Decision::Keep(KeepReason::Unsupported(Unsupported::Jpx))
        );
        k.encoding = Encoding::Raw;
        k.color = ColorModel::Other;
        assert_eq!(
            decide(&k, &[], &screen()),
            Decision::Keep(KeepReason::Unsupported(Unsupported::OtherColor))
        );
        k.color = ColorModel::Rgb;
        k.masks.color_key_mask = true;
        assert_eq!(
            decide(&k, &[], &screen()),
            Decision::Keep(KeepReason::Unsupported(Unsupported::ColorKeyMask))
        );
    }

    #[test]
    fn small_and_unplaced_images_are_kept() {
        let k = kind(Encoding::Raw, ColorModel::Rgb, 8, 2, 2);
        assert_eq!(decide(&k, &[placed(100.0)], &screen()), Decision::Keep(KeepReason::TooSmall));
        let k = kind(Encoding::Raw, ColorModel::Rgb, 8, 128, 128);
        assert_eq!(decide(&k, &[], &screen()), Decision::Keep(KeepReason::NoPlacement));
    }

    #[test]
    fn downsampling_targets_the_policy_resolution_only_past_the_threshold() {
        let k = kind(Encoding::Raw, ColorModel::Rgb, 8, 128, 128);
        // 288 ppi > 72 × 1.5 → 128 × 72 / 288 = 32 px.
        assert_eq!(
            decide(&k, &[placed(32.0)], &screen()),
            Decision::Reencode {
                target: Some((32, 32)),
                output: Output::Jpeg {
                    quality: 60,
                    convert_cmyk: false,
                    subsampling: Subsampling::Half,
                },
            }
        );
        // 100 ppi ≤ 108 → no downsample, still re-encoded (lossless source, lossy allowed).
        assert_eq!(
            decide(&k, &[placed(92.16)], &screen()),
            Decision::Reencode {
                target: None,
                output: Output::Jpeg {
                    quality: 60,
                    convert_cmyk: false,
                    subsampling: Subsampling::Half,
                },
            }
        );
    }

    #[test]
    fn a_jpeg_is_reencoded_only_when_downsampled() {
        let k = kind(Encoding::Jpeg, ColorModel::Rgb, 8, 128, 128);
        assert_eq!(
            decide(&k, &[placed(128.0)], &screen()),
            Decision::Keep(KeepReason::WithinResolution)
        );
        assert_eq!(
            decide(&k, &[placed(128.0)], &lossless()),
            Decision::Keep(KeepReason::AlreadyOptimal)
        );
        assert!(matches!(
            decide(&k, &[placed(32.0)], &screen()),
            Decision::Reencode {
                target: Some((32, 32)),
                ..
            }
        ));
    }

    #[test]
    fn cmyk_output_follows_the_conversion_flag() {
        let k = kind(Encoding::Jpeg, ColorModel::Cmyk, 8, 64, 64);
        assert_eq!(
            decide(&k, &[placed(16.0)], &screen()),
            Decision::Reencode {
                target: Some((16, 16)),
                output: Output::Jpeg {
                    quality: 60,
                    convert_cmyk: true,
                    subsampling: Subsampling::Half,
                },
            }
        );
        let print = Policy {
            color_ppi: Some(300.0),
            gray_ppi: Some(300.0),
            mono_ppi: Some(1200.0),
            convert_cmyk_to_rgb: false,
            ..screen()
        };
        // 288 ppi ≤ 450: kept.
        assert_eq!(
            decide(&k, &[placed(16.0)], &print),
            Decision::Keep(KeepReason::WithinResolution)
        );
        // Forced past the threshold without conversion: Flate CMYK.
        assert_eq!(
            decide(&k, &[placed(4.0)], &print),
            Decision::Reencode {
                target: Some((17, 17)),
                output: Output::FlatePredicted,
            }
        );
    }

    #[test]
    fn bilevel_goes_to_ccitt_and_ccitt_stays_unless_downsampled() {
        let flate = kind(Encoding::Raw, ColorModel::Bilevel, 1, 64, 64);
        assert_eq!(
            decide(&flate, &[placed(16.0)], &screen()),
            Decision::Reencode {
                target: None,
                output: Output::CcittG4
            }
        );
        assert_eq!(
            decide(&flate, &[placed(16.0)], &lossless()),
            Decision::Reencode {
                target: None,
                output: Output::CcittG4
            }
        );
        let g4 = kind(Encoding::Ccitt, ColorModel::Bilevel, 1, 64, 64);
        assert_eq!(
            decide(&g4, &[placed(16.0)], &screen()),
            Decision::Keep(KeepReason::WithinResolution)
        );
        assert_eq!(
            decide(&g4, &[placed(16.0)], &lossless()),
            Decision::Keep(KeepReason::AlreadyOptimal)
        );
        // 1200 ppi > 300 × 1.5 → 16 px.
        assert_eq!(
            decide(&g4, &[placed(3.84)], &screen()),
            Decision::Reencode {
                target: Some((16, 16)),
                output: Output::CcittG4
            }
        );
    }

    #[test]
    fn lossless_policy_never_picks_a_lossy_codec_and_leaves_palettes_alone() {
        let rgb = kind(Encoding::Raw, ColorModel::Rgb, 16, 64, 64);
        assert_eq!(
            decide(&rgb, &[placed(64.0)], &lossless()),
            Decision::Reencode {
                target: None,
                output: Output::FlatePredicted
            }
        );
        let mut indexed = kind(Encoding::Raw, ColorModel::Rgb, 4, 64, 64);
        indexed.indexed = true;
        assert_eq!(
            decide(&indexed, &[placed(16.0)], &lossless()),
            Decision::Keep(KeepReason::AlreadyOptimal)
        );
        assert!(matches!(
            decide(&indexed, &[placed(16.0)], &screen()),
            Decision::Reencode {
                target: Some((16, 16)),
                output: Output::Jpeg { .. }
            }
        ));
    }


    #[test]
    fn chroma_auto_follows_quality_and_presets_can_pin_it() {
        let mut p = screen();
        assert_eq!(p.subsampling(), Subsampling::Half);
        p.jpeg_quality = 90;
        assert_eq!(p.subsampling(), Subsampling::Full);
        p.jpeg_quality = 85;
        p.chroma = Chroma::Full;
        assert_eq!(p.subsampling(), Subsampling::Full);
        p.chroma = Chroma::Half;
        p.jpeg_quality = 100;
        assert_eq!(p.subsampling(), Subsampling::Half);
    }

    #[test]
    fn recompress_jpeg_reencodes_a_jpeg_only_when_asked_and_lossy() {
        let k = kind(Encoding::Jpeg, ColorModel::Rgb, 8, 128, 128);
        let mut p = screen();
        p.recompress_jpeg = true;
        assert!(matches!(
            decide(&k, &[placed(128.0)], &p),
            Decision::Reencode { target: None, output: Output::Jpeg { quality: 60, .. } }
        ));
        p.allow_lossy = false;
        assert_eq!(decide(&k, &[placed(128.0)], &p), Decision::Keep(KeepReason::WithinResolution));
        // Stored quality at or below the target: nothing left to take.
        p.allow_lossy = true;
        let mut done = k.clone();
        done.jpeg_quality = Some(60);
        assert_eq!(decide(&done, &[placed(128.0)], &p), Decision::Keep(KeepReason::AlreadyOptimal));
        done.jpeg_quality = Some(61);
        assert!(matches!(decide(&done, &[placed(128.0)], &p), Decision::Reencode { .. }));
    }
}
