//! Font metrics for the redaction pass: glyph widths and encodings come from
//! the page's own `FontInfo`, the same source the text extractor reads, so the
//! redactor's geometry cannot diverge from extraction.

use super::text_engine::FontMetrics;
use crate::fonts::{Encoding, FontInfo};
use std::collections::HashMap;
use std::sync::Arc;

// ── pdf_manipulator patch: the font metrics the redaction pass reads ──

/// [`FontMetrics`] over the real per-page `FontInfo` map (resource name
/// → font). The single source of glyph widths is `FontInfo`
/// (`get_glyph_width`) — the *same* metric the text extractor uses, so
/// the redactor's geometry cannot diverge from extraction (DRY; feature
/// plan §9 risk 1). An unknown font, or any composite/Type0 font, is
/// reported non-simple so the engine refuses rather than under-redacts.
pub struct FontInfoMetrics {
    fonts: HashMap<String, Arc<FontInfo>>,
}

impl FontInfoMetrics {
    /// Build from a page's resolved `/Font` resources (resource name
    /// → parsed [`FontInfo`]).
    pub fn new(fonts: HashMap<String, Arc<FontInfo>>) -> Self {
        Self { fonts }
    }

    /// `true` only for a Type0 font whose encoding is the **horizontal**
    /// identity CMap (Identity-H). Per ISO 32000-1 §9.7.5.2 Identity-H maps
    /// each 2-byte code, high-order byte first, to the CID of the same
    /// value, so the engine can decode the show string into CIDs and look
    /// up each CID's width via `/W`+`/DW` exactly as the text extractor
    /// does. Identity-V (vertical, `wmode == 1`) is deliberately excluded —
    /// its glyphs advance down the page, which the horizontal advance/box
    /// model here does not handle — as are all non-identity predefined and
    /// custom CMaps (their code lengths and code→CID mapping are not
    /// reconstructable here), so those stay fail-closed.
    fn is_identity_h(&self, font: &str) -> bool {
        self.fonts.get(font).is_some_and(|fi| {
            fi.subtype == "Type0" && matches!(fi.encoding, Encoding::Identity) && fi.wmode == 0
        })
    }
}

impl FontMetrics for FontInfoMetrics {
    fn width(&self, font: &str, code: u32) -> f32 {
        match self.fonts.get(font) {
            // `get_glyph_width` returns /1000 glyph-space units.
            Some(fi) => fi.get_glyph_width(code.min(u32::from(u16::MAX)) as u16),
            // Unknown font: full-em over-estimate (the show is refused
            // anyway via `is_simple`, but never under-size a box).
            None => 1000.0,
        }
    }

    fn is_simple(&self, font: &str) -> bool {
        // Only single-byte simple fonts have a reliable byte→code→width
        // path here. Type0/CID and unknown ⇒ refuse (fail closed).
        match self.fonts.get(font) {
            Some(fi) => fi.subtype != "Type0",
            None => false,
        }
    }

    fn can_prune(&self, font: &str) -> bool {
        // Single-byte simple fonts, plus Identity-H Type0 (2-byte CID =
        // code). Everything else — Identity-V, legacy/custom CMaps, unknown
        // fonts — stays fail-closed.
        self.is_simple(font) || self.is_identity_h(font)
    }

    fn decode(&self, font: &str, s: &[u8]) -> Vec<(u32, Vec<u8>)> {
        if self.is_identity_h(font) {
            // Identity-H: every two bytes are one code, high-order byte
            // first, and that code IS the CID (§9.7.5.2). A trailing odd
            // byte is malformed; `redaction_safe_show` refuses such a show
            // before it reaches here, so emit only whole 2-byte codes.
            return s
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| (u32::from(u16::from_be_bytes([pair[0], pair[1]])), pair.to_vec()))
                .collect();
        }
        // Simple fonts: one byte, one code.
        s.iter().map(|&b| (b as u32, vec![b])).collect()
    }

    fn is_word_space(&self, font: &str, code: u32) -> bool {
        // ISO 32000-1 §9.3.3: word spacing (Tw) applies only to a
        // single-byte code 32. It never applies to a byte value of 32 that
        // is part of a multi-byte code, so Identity-H glyphs never trigger
        // it. Simple fonts keep the single-byte-32 rule.
        !self.is_identity_h(font) && code == 32
    }

    fn redaction_safe_show(&self, font: &str, bytes: &[u8]) -> bool {
        // Identity-H codes are exactly two bytes; an odd-length show string
        // cannot be split into whole codes without guessing a boundary, so
        // refuse rather than risk mis-aligning the prune (fail closed).
        !self.is_identity_h(font) || bytes.len().is_multiple_of(2)
    }
}
// ── end pdf_manipulator patch ──
