//! What happened to each image, and the names those facts travel under on
//! the wire (mirrored by `PdfImageOutcome` on the Dart side).
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).

use super::classify::{ColorModel, Encoding, Kind, Unsupported};
use super::policy::KeepReason;

/// What the pipeline did to an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Bytes unchanged.
    Kept,
    /// Same pixel size, new encoding.
    Recompressed,
    /// Fewer pixels, new encoding.
    Downsampled,
}

/// One image XObject's result. `bytes_*` count the parent and its soft mask.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The XObject's object number.
    pub object_id: u32,
    /// Classification as stored, before any change.
    pub kind: Kind,
    /// Placements across the visible pages, forms included.
    pub uses: u32,
    /// `None` when no placement had a usable CTM.
    pub ppi_min: Option<f64>,
    /// What happened.
    pub action: Action,
    /// Why nothing happened; `None` unless `action` is `Kept`.
    pub keep_reason: Option<KeepReason>,
    /// Stored bytes before, parent plus soft mask.
    pub bytes_before: u64,
    /// Stored bytes after; equals `bytes_before` when kept.
    pub bytes_after: u64,
    /// Pixel width after.
    pub width_after: u32,
    /// Pixel height after.
    pub height_after: u32,
}

/// Outcomes in ascending object id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// One row per painted image XObject.
    pub images: Vec<Outcome>,
}

impl Report {
    /// Images whose bytes changed.
    pub fn changed(&self) -> usize {
        self.images
            .iter()
            .filter(|o| o.action != Action::Kept)
            .count()
    }
}

impl Encoding {
    /// The name this value travels under on the wire.
    pub fn wire_name(self) -> &'static str {
        match self {
            Encoding::Raw => "raw",
            Encoding::Jpeg => "jpeg",
            Encoding::Jpx => "jpx",
            Encoding::Ccitt => "ccitt",
            Encoding::Jbig2 => "jbig2",
            Encoding::Unknown => "unknown",
        }
    }
}

impl ColorModel {
    /// The name this value travels under on the wire.
    pub fn wire_name(self) -> &'static str {
        match self {
            ColorModel::Bilevel => "bilevel",
            ColorModel::Gray => "gray",
            ColorModel::Rgb => "rgb",
            ColorModel::Cmyk => "cmyk",
            ColorModel::Other => "other",
        }
    }
}

impl Action {
    /// The name this value travels under on the wire.
    pub fn wire_name(self) -> &'static str {
        match self {
            Action::Kept => "kept",
            Action::Recompressed => "recompressed",
            Action::Downsampled => "downsampled",
        }
    }
}

impl KeepReason {
    /// The name this value travels under on the wire.
    pub fn wire_name(self) -> &'static str {
        match self {
            KeepReason::Unsupported(Unsupported::Jpx) => "unsupportedJpx",
            KeepReason::Unsupported(Unsupported::Jbig2) => "unsupportedJbig2",
            KeepReason::Unsupported(Unsupported::UnknownFilter) => "unsupportedFilter",
            KeepReason::Unsupported(Unsupported::OtherColor) => "unsupportedColor",
            KeepReason::Unsupported(Unsupported::StencilMask) => "unsupportedStencilMask",
            KeepReason::Unsupported(Unsupported::ColorKeyMask) => "unsupportedColorKeyMask",
            KeepReason::Unsupported(Unsupported::SmaskInData) => "unsupportedSmaskInData",
            KeepReason::TooSmall => "tooSmall",
            KeepReason::NoPlacement => "noPlacement",
            KeepReason::WithinResolution => "withinResolution",
            KeepReason::AlreadyOptimal => "alreadyOptimal",
            KeepReason::BelowMinSavings => "belowMinSavings",
            KeepReason::Undecodable => "undecodable",
        }
    }
}
