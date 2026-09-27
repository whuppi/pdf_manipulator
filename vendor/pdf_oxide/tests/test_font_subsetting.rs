//! Regression tests for issue #385 — real font subsetting on the
//! `DocumentBuilder::register_embedded_font` path (FONT-3b).
//!
//! Before v0.3.38, the `subsetter` wrapper at
//! `crate::fonts::subset_font_bytes` was present and `FontSubsetter`
//! tracked used glyphs, but nothing was wired up: `FontFile2` carried
//! the entire original face, and content streams were emitted with
//! original-face GIDs, because a GID-remapping pass hadn't landed.
//! Users embedding a ~20 MB CJK face to render a handful of glyphs
//! got a ~20 MB output PDF.
//!
//! v0.3.38 wires the whole thing up:
//!
//! 1. `ContentStreamBuilder` buffers embedded-font text as a structured
//!    `ShowEmbeddedText { font_name, glyph_ids }` op carrying the
//!    *original-face* GIDs.
//! 2. `PdfWriter::finish` runs `subset_font_bytes` per font, collects
//!    the `GlyphRemapper`s, and passes them into every page's
//!    `ContentStreamBuilder::build_with_remappers`.
//! 3. Hex glyph IDs emitted in the content stream are the **subset**
//!    GIDs; `/W` widths and `ToUnicode` CMap are keyed on the same
//!    subset GID space. Every reader sees a coherent subset.
//!
//! These tests lock in that behaviour: the output PDF is dramatically
//! smaller than the input face, and `extract_text` still round-trips.

use pdf_oxide::writer::{DocumentBuilder, EmbeddedFont};
use pdf_oxide::PdfDocument;
use std::path::Path;

const DEJAVU_PATH: &str = "tests/fixtures/fonts/DejaVuSans.ttf";

fn load_dejavu() -> EmbeddedFont {
    EmbeddedFont::from_file(Path::new(DEJAVU_PATH))
        .expect("tests/fixtures/fonts/DejaVuSans.ttf should exist and parse")
}

fn dejavu_byte_len() -> usize {
    std::fs::metadata(DEJAVU_PATH)
        .expect("DejaVuSans.ttf present")
        .len() as usize
}

/// With full-font embedding, a PDF containing 10 ASCII characters from
/// DejaVuSans was ~760 KB — roughly the size of the face. With real
/// subsetting, the embedded font should trim by ~99%, so the total PDF
/// (including overhead) is well under 30 KB for this glyph set.
///
/// This assertion also catches regressions where subsetting is wired
/// to the wrong path (e.g. the FontFile2 stream is re-embedded full
/// while `/W` is subset-keyed, which would still pass round-trip tests
/// but blow up file size).
#[test]
fn ascii_document_is_much_smaller_than_original_face() {
    let font = load_dejavu();
    let face_bytes = dejavu_byte_len();

    let mut builder = DocumentBuilder::new().register_embedded_font("DejaVu", font);
    builder
        .a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 700.0)
        .text("Hello world")
        .done();

    let bytes = builder.build().expect("build should succeed");

    // Sanity floor — if the PDF shrinks to zero bytes, something is
    // very wrong regardless of subsetting.
    assert!(bytes.len() > 256, "PDF is suspiciously small: {} bytes", bytes.len());

    // 11 unique ASCII characters. A subset FontFile2 for that set is
    // on the order of ~5 KB; the rest of the PDF (xref, catalog, page
    // tree, content stream) adds a few hundred more. 50 KB gives us
    // ~10× headroom over the expected size while still catching any
    // regression to full-font embedding (760 KB).
    assert!(
        bytes.len() < 50_000,
        "PDF is {} bytes (original face alone is {} bytes) — \
         subsetter is probably not actually wired up",
        bytes.len(),
        face_bytes,
    );

    // And the PDF is at least an order of magnitude smaller than the
    // raw face it came from, which is the user-visible behaviour we
    // care about.
    assert!(
        bytes.len() * 10 < face_bytes,
        "Expected PDF to be at least 10× smaller than the original \
         face ({} bytes); got {} bytes",
        face_bytes,
        bytes.len(),
    );
}

/// After subsetting, the content stream emits subset-local GIDs; the
/// `ToUnicode` CMap maps those same subset GIDs back to codepoints.
/// If any one of (FontFile2 / `/W` / ToUnicode / content stream) is
/// keyed on a different GID space, `extract_text` returns garbage.
/// This is the FONT-3b correctness invariant.
#[test]
fn extract_text_round_trips_post_subsetting_ascii() {
    let font = load_dejavu();
    let mut builder = DocumentBuilder::new().register_embedded_font("DejaVu", font);
    builder
        .a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 720.0)
        .text("The quick brown fox")
        .at(72.0, 700.0)
        .text("jumps over the lazy dog")
        .done();

    let bytes = builder.build().expect("build should succeed");
    let doc = PdfDocument::from_bytes(bytes).expect("parse round-tripped pdf");
    let text = doc.extract_text(0).expect("extract_text should succeed");

    assert!(
        text.contains("The quick brown fox"),
        "ASCII round-trip failed after subsetting — got: {text:?}"
    );
    assert!(
        text.contains("jumps over the lazy dog"),
        "ASCII round-trip failed after subsetting — got: {text:?}"
    );
}

/// Same round-trip invariant for Cyrillic + Greek — the scripts users
/// were actually reporting #382/#385 against. DejaVuSans has cmap entries
/// for all of these.
#[test]
fn extract_text_round_trips_post_subsetting_cyrillic_and_greek() {
    let font = load_dejavu();
    let mut builder = DocumentBuilder::new().register_embedded_font("DejaVu", font);
    builder
        .a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 720.0)
        .text("Привет, мир!")
        .at(72.0, 700.0)
        .text("Καλημέρα κόσμε")
        .done();

    let bytes = builder.build().expect("build should succeed");
    let doc = PdfDocument::from_bytes(bytes).expect("parse round-tripped pdf");
    let text = doc.extract_text(0).expect("extract_text should succeed");

    assert!(
        text.contains("Привет, мир!"),
        "Cyrillic round-trip failed after subsetting — got: {text:?}"
    );
    assert!(
        text.contains("Καλημέρα κόσμε"),
        "Greek round-trip failed after subsetting — got: {text:?}"
    );
}

/// Two fonts registered under different resource names get independent
/// subsetters + remappers — glyphs used in font A must not leak into
/// font B's subset face or vice versa.
#[test]
fn two_embedded_fonts_produce_independent_subsets() {
    let font_a = load_dejavu();
    let font_b = load_dejavu();

    let mut builder = DocumentBuilder::new()
        .register_embedded_font("FontA", font_a)
        .register_embedded_font("FontB", font_b);
    builder
        .a4_page()
        .font("FontA", 12.0)
        .at(72.0, 720.0)
        .text("Only-A-text")
        .font("FontB", 12.0)
        .at(72.0, 700.0)
        .text("Different-B-text")
        .done();

    let bytes = builder.build().expect("build should succeed");

    // Round-trip sanity: both strings must come back intact, which
    // requires both fonts' remappers to be applied to their respective
    // runs (cross-applying would produce garbled text).
    let doc = PdfDocument::from_bytes(bytes.clone()).expect("parse round-tripped pdf");
    let text = doc.extract_text(0).expect("extract_text should succeed");
    assert!(text.contains("Only-A-text"), "FontA text missing — got {text:?}");
    assert!(text.contains("Different-B-text"), "FontB text missing — got {text:?}");

    // Both subsets should be small — if the pipeline accidentally
    // shared a single remapper across fonts, one would contain the
    // full face worth of glyphs and blow up the total size.
    let face_bytes = dejavu_byte_len();
    assert!(
        bytes.len() < 80_000,
        "Two-font PDF is {} bytes (each original face is {} bytes)",
        bytes.len(),
        face_bytes,
    );
}

/// A font that is *registered* but never *used* still needs a valid
/// minimal subset (just `.notdef`) so the PDF stays structurally sound
/// — any page that references the resource name but never actually
/// emits text must not produce corrupt output.
#[test]
fn registered_but_unused_font_emits_minimal_subset_without_panic() {
    let font = load_dejavu();
    let mut builder = DocumentBuilder::new().register_embedded_font("Unused", font);

    // Don't actually emit any text with Unused.
    builder.a4_page().at(72.0, 700.0).done();

    let bytes = builder
        .build()
        .expect("build must not fail for registered-but-unused font");
    assert!(bytes.len() > 128, "PDF is too small: {} bytes", bytes.len());
    // And the output must still parse.
    let _doc = PdfDocument::from_bytes(bytes).expect("parse registered-but-unused-font pdf");
}

/// The `base_font` name includes the 6-letter subset tag (`/ABCDEF+FontName`)
/// because the subsetter produced a real subset. This is the reader-
/// facing signal that the PDF is subsetted. We don't pin the specific
/// `FontName` — TTF PostScript names vary slightly between foundries /
/// versions — only the `XXXXXX+` shape.
#[test]
fn output_pdf_carries_subset_tag_in_font_name() {
    let font = load_dejavu();
    let mut builder = DocumentBuilder::new().register_embedded_font("DejaVu", font);
    builder
        .a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 700.0)
        .text("Hello")
        .done();

    let bytes = builder.build().expect("build should succeed");
    let pdf_text = String::from_utf8_lossy(&bytes);

    // Scan for `/XXXXXX+` where XXXXXX is exactly 6 uppercase ASCII
    // letters — the canonical PDF subset-tag prefix (ISO 32000-1 §9.6.4).
    let has_subset_prefix = pdf_text
        .as_bytes()
        .windows(8)
        .any(|w| w[0] == b'/' && w[1..7].iter().all(|&b| b.is_ascii_uppercase()) && w[7] == b'+');
    assert!(
        has_subset_prefix,
        "Expected `/XXXXXX+` subset-tag prefix somewhere in the PDF, \
         indicating the font was subsetted",
    );
}
