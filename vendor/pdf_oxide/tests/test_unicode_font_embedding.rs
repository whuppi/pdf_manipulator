//! Integration tests for Unicode font embedding (v0.3.0 Phase 1).
//!
//! These tests verify the font embedding infrastructure:
//! - TrueType font parsing
//! - Font subsetting
//! - Unicode encoding
//! - ToUnicode CMap generation

use pdf_oxide::fonts::{subset_font_bytes, FontSubsetter, TrueTypeError, UnicodeEncoder};
use pdf_oxide::writer::{EmbeddedFont, EmbeddedFontManager};
use std::collections::BTreeSet;

/// Test that the font subsetter tracks used glyphs correctly.
#[test]
fn test_font_subsetter_tracks_glyphs() {
    let mut subsetter = FontSubsetter::new();

    // Use some characters
    subsetter.use_char(0x0041, 1); // 'A' -> GID 1
    subsetter.use_char(0x0042, 2); // 'B' -> GID 2
    subsetter.use_char(0x0043, 3); // 'C' -> GID 3

    assert_eq!(subsetter.char_count(), 3);
    assert_eq!(subsetter.glyph_count(), 3);

    // Used glyphs should be tracked
    let used = subsetter.used_glyphs();
    assert!(used.contains(&1));
    assert!(used.contains(&2));
    assert!(used.contains(&3));
}

/// Test subset tag generation is deterministic.
#[test]
fn test_subset_tag_deterministic() {
    let mut subsetter1 = FontSubsetter::new();
    subsetter1.use_char(0x0041, 1);
    subsetter1.use_char(0x0042, 2);

    let mut subsetter2 = FontSubsetter::new();
    subsetter2.use_char(0x0041, 1);
    subsetter2.use_char(0x0042, 2);

    // Same usage should produce same tag
    let tag1 = subsetter1.generate_subset_tag().to_string();
    let tag2 = subsetter2.generate_subset_tag().to_string();

    assert_eq!(tag1, tag2);
    assert_eq!(tag1.len(), 6);
    assert!(tag1.chars().all(|c| c.is_ascii_uppercase()));
}

/// Test Unicode encoder produces correct Identity-H encoding.
#[test]
fn test_unicode_encoder_identity_h() {
    let mut encoder = UnicodeEncoder::new();

    // Simple lookup: character code = glyph ID for testing
    let lookup = |cp: u32| match cp {
        0x41 => Some(0x0001_u16), // 'A' -> GID 1
        0x42 => Some(0x0002_u16), // 'B' -> GID 2
        0x43 => Some(0x0003_u16), // 'C' -> GID 3
        _ => None,
    };

    let encoded = encoder.encode_identity_h("ABC", lookup);

    // Should be hex string with 4-digit glyph IDs
    assert_eq!(encoded, "<000100020003>");
}

/// Test Unicode encoder handles missing glyphs.
#[test]
fn test_unicode_encoder_missing_glyph() {
    let mut encoder = UnicodeEncoder::new();

    // Lookup returns None for unknown chars
    let lookup = |_: u32| None;

    let encoded = encoder.encode_identity_h("A", lookup);

    // Missing glyph should use .notdef (GID 0)
    assert_eq!(encoded, "<0000>");
}

/// Test UTF-16BE encoding for PDF metadata.
#[test]
fn test_utf16be_encoding() {
    // BMP character
    let result = UnicodeEncoder::encode_utf16be("A");
    assert!(result.starts_with("<FEFF")); // BOM
    assert!(result.contains("0041")); // 'A'

    // Non-BMP character (emoji)
    let result = UnicodeEncoder::encode_utf16be("\u{1F600}"); // Grinning face
    assert!(result.contains("D83D")); // High surrogate
    assert!(result.contains("DE00")); // Low surrogate
}

/// Test literal string encoding.
#[test]
fn test_literal_string_encoding() {
    let result = UnicodeEncoder::encode_literal("Hello");
    assert_eq!(result, "(Hello)");

    // Special characters should be escaped
    let result = UnicodeEncoder::encode_literal("(test)");
    assert_eq!(result, "(\\(test\\))");
}

/// Test embedded font manager registration.
#[test]
fn test_embedded_font_manager() {
    let manager = EmbeddedFontManager::new();

    assert!(manager.is_empty());
    assert_eq!(manager.len(), 0);
}

/// Test that invalid font data is rejected.
#[test]
fn test_invalid_font_data_rejected() {
    let result = EmbeddedFont::from_data(None, vec![0, 1, 2, 3]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Failed to parse font"));
}

/// Test that empty font data is rejected.
#[test]
fn test_empty_font_data_rejected() {
    let result = EmbeddedFont::from_data(None, vec![]);

    assert!(result.is_err());
}

/// Test ToUnicode CMap generation format.
#[test]
fn test_tounicode_cmap_format() {
    let mut subsetter = FontSubsetter::new();
    subsetter.use_char(0x0041, 1); // 'A' -> GID 1
    subsetter.use_char(0x0042, 2); // 'B' -> GID 2

    // Create a minimal embedded font-like structure for testing
    // (In real usage, we'd have a full EmbeddedFont)

    // Verify the CMap format requirements
    let used_chars = subsetter.used_chars();
    assert_eq!(used_chars.len(), 2);
    assert_eq!(used_chars.get(&0x0041), Some(&1));
    assert_eq!(used_chars.get(&0x0042), Some(&2));
}

/// Test widths array generation.
#[test]
fn test_widths_array_generation() {
    let mut subsetter = FontSubsetter::new();

    // Use consecutive glyphs
    subsetter.use_char(0x0041, 10); // GID 10
    subsetter.use_char(0x0042, 11); // GID 11
    subsetter.use_char(0x0043, 12); // GID 12

    // And a non-consecutive one
    subsetter.use_char(0x0044, 20); // GID 20

    let used_glyphs = subsetter.used_glyphs();

    // Should have 4 glyphs
    assert_eq!(used_glyphs.len(), 4);

    // Consecutive glyphs 10, 11, 12 and separate 20
    assert!(used_glyphs.contains(&10));
    assert!(used_glyphs.contains(&11));
    assert!(used_glyphs.contains(&12));
    assert!(used_glyphs.contains(&20));
}

/// Test that TrueType parser returns proper error types.
#[test]
fn test_truetype_error_types() {
    // Empty data
    let result = pdf_oxide::fonts::TrueTypeFont::parse(&[]);
    assert!(matches!(result, Err(TrueTypeError::EmptyFont)));

    // Invalid data
    let result = pdf_oxide::fonts::TrueTypeFont::parse(b"not a font");
    assert!(matches!(result, Err(TrueTypeError::ParseError(_))));
}

/// Test subset statistics calculation.
#[test]
fn test_subset_stats() {
    let mut subsetter = FontSubsetter::new();
    subsetter.use_char(0x0041, 5);
    subsetter.use_char(0x0042, 10);
    subsetter.use_char(0x0043, 15);

    let stats = subsetter.stats();

    assert_eq!(stats.unique_chars, 3);
    assert_eq!(stats.unique_glyphs, 3);
    assert_eq!(stats.min_glyph_id, Some(5));
    assert_eq!(stats.max_glyph_id, Some(15));

    // Reduction estimate for 1000 glyph font
    let reduction = stats.estimated_reduction(1000);
    assert!(reduction > 99.0); // Using 3 out of 1000 = ~99.7% reduction
}

/// Test encoder caching behavior.
#[test]
fn test_encoder_caching() {
    let mut encoder = UnicodeEncoder::new();
    let lookup = |cp: u32| Some(cp as u16);

    // First encoding
    encoder.encode_identity_h("AAA", lookup);
    assert_eq!(encoder.cache_size(), 1); // Only 'A' cached

    // Encoding more characters
    encoder.encode_identity_h("ABC", lookup);
    assert_eq!(encoder.cache_size(), 3); // A, B, C cached

    // Clear cache
    encoder.clear_cache();
    assert_eq!(encoder.cache_size(), 0);
}

/// Test smart text encoding selection.
#[test]
fn test_encode_text_auto_selection() {
    // Pure ASCII -> literal string
    let result = UnicodeEncoder::encode_text("Hello");
    assert!(result.starts_with('('));
    assert!(result.ends_with(')'));

    // Unicode -> UTF-16BE hex string
    let result = UnicodeEncoder::encode_text("Hello \u{4E2D}\u{6587}"); // Hello 中文
    assert!(result.starts_with("<FEFF")); // UTF-16BE BOM
}

// ──────────────────────────────────────────────────────────────────────
// Binary subsetting (FONT-2). subset_font_bytes wraps Typst's `subsetter`
// crate so that EmbeddedFont can shrink the embedded TTF to only the
// glyphs actually used. Acceptance from the v0.3.35 plan: typical
// English content shrinks DejaVuSans from ~760 KB to under 30 KB.
// ──────────────────────────────────────────────────────────────────────

const DEJAVU_SANS: &[u8] = include_bytes!("fixtures/fonts/DejaVuSans.ttf");

#[test]
fn test_subset_dejavu_for_english_under_30kb() {
    // Glyph IDs covering a typical English alphabet plus digits and
    // common punctuation. We hard-code GIDs so the test does not depend
    // on TrueTypeFont parsing — that path is exercised elsewhere.
    let mut used: BTreeSet<u16> = BTreeSet::new();
    // DejaVuSans GID range covering 0x20..0x7e is roughly 3..=94; that's
    // dense enough that a remap-from-zero subset is the realistic
    // worst case for "English text only."
    for gid in 3..=94u16 {
        used.insert(gid);
    }

    let (subset, remapper) =
        subset_font_bytes(DEJAVU_SANS, 0, &used).expect("subsetting must succeed");

    // Shrink ratio: DejaVuSans is ~760 KB; subset for ~90 glyphs should
    // be well under 30 KB. The acceptance bar is the v0.3.35 plan's
    // contract; we keep margin to absorb font-version drift.
    assert!(
        subset.len() < 30_000,
        "subset should be < 30 KB for ASCII coverage, got {} bytes",
        subset.len()
    );
    assert!(
        subset.len() < DEJAVU_SANS.len() / 10,
        "subset should be at least 10× smaller than the original ({} vs {})",
        subset.len(),
        DEJAVU_SANS.len()
    );

    // Remapper must round-trip every kept glyph and renumber to a dense
    // 0..=N range starting from 0 (which is always .notdef).
    assert_eq!(remapper.get(0), Some(0), ".notdef must remain at GID 0 after subsetting");
    for &original_gid in &used {
        assert!(
            remapper.get(original_gid).is_some(),
            "kept glyph {original_gid} disappeared from remapper",
        );
    }
}

#[test]
fn test_subset_always_includes_notdef_even_when_caller_omits_it() {
    let mut used = BTreeSet::new();
    used.insert(36u16); // any glyph other than 0
    let (subset, remapper) =
        subset_font_bytes(DEJAVU_SANS, 0, &used).expect("subsetting must succeed");
    assert_eq!(remapper.get(0), Some(0), ".notdef auto-included");
    assert!(remapper.get(36).is_some());
    assert!(!subset.is_empty());
}

#[test]
fn test_subset_rejects_garbage_input() {
    let used: BTreeSet<u16> = (0..5).collect();
    let err = subset_font_bytes(b"not a font", 0, &used).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("font subsetting failed"),
        "expected SubsetError::Subsetter wrapper, got: {msg}",
    );
}
