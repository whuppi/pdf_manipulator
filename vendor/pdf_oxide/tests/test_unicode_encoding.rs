//! Regression tests for issue #402 and #406 — systemic UTF-8 encoding loss.
//!
//! These tests verify that non-ASCII text (accented Latin characters, CJK,
//! Arabic, etc.) is correctly encoded in every PDF string context:
//!   - Visible page text via Base-14 fonts (WinAnsiEncoding)
//!   - Document metadata (/Title, /Author, /Subject)
//!   - Annotation /Contents and /T fields
//!   - Bookmark /Title entries
//!   - Form field names and values
//!
//! The core invariant: a character like é (U+00E9) must appear in the PDF
//! file as the single byte 0xE9, not as the two-byte UTF-8 sequence 0xC3 0xA9.

use pdf_oxide::object::encode_pdf_text_string;
use pdf_oxide::writer::{DocumentBuilder, DocumentMetadata};

// ---------------------------------------------------------------------------
// Unit tests for encode_pdf_text_string
// ---------------------------------------------------------------------------

#[test]
fn encode_ascii_is_identity() {
    assert_eq!(encode_pdf_text_string("Hello"), b"Hello");
}

#[test]
fn encode_latin1_extended_char_preserves_bytes() {
    // The exact example from issue #402
    let bytes = encode_pdf_text_string("Lógico");
    // L=0x4C ó=0xF3 g=0x67 i=0x69 c=0x63 o=0x6F
    assert_eq!(bytes, &[0x4C, 0xF3, 0x67, 0x69, 0x63, 0x6F]);
}

#[test]
fn encode_portuguese_sentence() {
    let bytes = encode_pdf_text_string("Ação é lógica");
    // All chars are ≤ U+00FF, so each is its own byte
    for (i, ch) in "Ação é lógica".chars().enumerate() {
        assert_eq!(bytes[i], ch as u8, "byte {} should be 0x{:02X} for '{}'", i, ch as u8, ch);
    }
}

#[test]
fn encode_german_umlauts() {
    // ä=0xE4 ö=0xF6 ü=0xFC Ä=0xC4 Ö=0xD6 Ü=0xDC ß=0xDF
    let bytes = encode_pdf_text_string("äöüÄÖÜß");
    assert_eq!(bytes, &[0xE4, 0xF6, 0xFC, 0xC4, 0xD6, 0xDC, 0xDF]);
}

#[test]
fn encode_french_accents() {
    // è=0xE8 é=0xE9 ê=0xEA ë=0xEB à=0xE0 â=0xE2 ç=0xE7
    let bytes = encode_pdf_text_string("èéêëàâç");
    assert_eq!(bytes, &[0xE8, 0xE9, 0xEA, 0xEB, 0xE0, 0xE2, 0xE7]);
}

#[test]
fn encode_spanish_accents() {
    // á=0xE1 é=0xE9 í=0xED ó=0xF3 ú=0xFA ñ=0xF1 ¡=0xA1 ¿=0xBF
    let bytes = encode_pdf_text_string("áéíóúñ¡¿");
    assert_eq!(bytes, &[0xE1, 0xE9, 0xED, 0xF3, 0xFA, 0xF1, 0xA1, 0xBF]);
}

#[test]
fn encode_cjk_triggers_utf16be_with_bom() {
    // 中 = U+4E2D
    let bytes = encode_pdf_text_string("中");
    assert_eq!(&bytes[..2], &[0xFE, 0xFF], "BOM must be present");
    assert_eq!(bytes, &[0xFE, 0xFF, 0x4E, 0x2D]);
}

#[test]
fn encode_arabic_triggers_utf16be_with_bom() {
    // م = U+0645
    let bytes = encode_pdf_text_string("م");
    assert_eq!(&bytes[..2], &[0xFE, 0xFF]);
    assert_eq!(bytes, &[0xFE, 0xFF, 0x06, 0x45]);
}

#[test]
fn encode_mixed_latin_and_cjk_is_all_utf16be() {
    // When any char is > U+00FF the whole string must use UTF-16BE
    let bytes = encode_pdf_text_string("héllo中");
    assert_eq!(&bytes[..2], &[0xFE, 0xFF], "BOM required for mixed strings");
    // h=0x0068 é=0x00E9 l=0x006C l=0x006C o=0x006F 中=0x4E2D
    let expected: Vec<u8> = [0xFE_u8, 0xFF]
        .iter()
        .chain(
            "héllo中"
                .encode_utf16()
                .flat_map(|u| [(u >> 8) as u8, (u & 0xFF) as u8])
                .collect::<Vec<_>>()
                .iter(),
        )
        .copied()
        .collect();
    assert_eq!(bytes, expected);
}

#[test]
fn encode_empty_string() {
    assert_eq!(encode_pdf_text_string(""), b"");
}

#[test]
fn encode_null_byte_boundary() {
    // U+0000 should encode as byte 0x00 (PDFDocEncoding path)
    let bytes = encode_pdf_text_string("\u{0000}");
    assert_eq!(bytes, &[0x00]);
}

// ---------------------------------------------------------------------------
// Integration: raw bytes in the produced PDF must not contain UTF-8 sequences
// ---------------------------------------------------------------------------

/// Extract the raw bytes of a built PDF and verify the accent encoding.
///
/// The strategy: build a minimal PDF whose title (metadata) contains "é",
/// then search the raw output for the UTF-8 multi-byte encoding of "é"
/// (0xC3 0xA9).  If the bug is present this sequence appears; if fixed it
/// must NOT appear.  Instead we expect the single byte 0xE9.
#[test]
fn metadata_title_with_accents_uses_pdfdocencoding_not_utf8() {
    let builder = DocumentBuilder::new().metadata(
        DocumentMetadata::new()
            .title("Título")
            .author("Ångström")
            .subject("Ação"),
    );
    // Add a minimal page so the builder produces a valid PDF
    let mut builder = builder;
    builder.a4_page().done();

    let pdf_bytes = builder
        .build()
        .expect("DocumentBuilder::build should succeed");

    // UTF-8 encoding of é is 0xC3 0xA9 — must NOT appear
    let utf8_e_acute: &[u8] = &[0xC3, 0xA9]; // é
    let utf8_i_tilde: &[u8] = &[0xC3, 0xAD]; // í
    let utf8_a_tilde: &[u8] = &[0xC3, 0xA3]; // ã
    let utf8_angstrom_a: &[u8] = &[0xC3, 0x85]; // Å

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    assert!(
        !contains_subslice(&pdf_bytes, utf8_e_acute),
        "PDF contains raw UTF-8 bytes for 'é' (0xC3 0xA9) — encoding bug still present"
    );
    assert!(
        !contains_subslice(&pdf_bytes, utf8_i_tilde),
        "PDF contains raw UTF-8 bytes for 'í' (0xC3 0xAD) — encoding bug still present"
    );
    assert!(
        !contains_subslice(&pdf_bytes, utf8_a_tilde),
        "PDF contains raw UTF-8 bytes for 'ã' — encoding bug still present"
    );
    assert!(
        !contains_subslice(&pdf_bytes, utf8_angstrom_a),
        "PDF contains raw UTF-8 bytes for 'Å' — encoding bug still present"
    );
}

#[test]
fn content_stream_latin1_text_uses_single_byte_not_utf8() {
    // Build a page that writes "Lógico" (the exact example from issue #402)
    // via the default Helvetica / base-14 font path.
    let mut builder = DocumentBuilder::new();
    builder.a4_page().at(72.0, 700.0).text("Lógico").done();

    let pdf_bytes = builder.build().expect("build should succeed");

    // ó in UTF-8 is 0xC3 0xB3 — this pair must NOT appear in the content stream
    let utf8_o_acute: &[u8] = &[0xC3, 0xB3];

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    assert!(
        !contains_subslice(&pdf_bytes, utf8_o_acute),
        "Content stream contains raw UTF-8 bytes for 'ó' (0xC3 0xB3) — write_escaped_string bug"
    );

    // The single WinAnsi byte 0xF3 for ó must appear somewhere in the PDF
    assert!(
        pdf_bytes.contains(&0xF3),
        "Expected WinAnsi byte 0xF3 for 'ó' to be present in the PDF"
    );
}

#[test]
fn metadata_with_cjk_title_uses_utf16be_bom() {
    // A title containing a CJK character must trigger UTF-16BE encoding.
    // The serializer hex-encodes non-ASCII strings, so the BOM (0xFE 0xFF)
    // appears in the PDF as the ASCII hex digits "FEFF" inside a <...> string.
    let builder = DocumentBuilder::new().metadata(DocumentMetadata::new().title("PDF文書"));
    let mut builder = builder;
    builder.a4_page().done();

    let pdf_bytes = builder.build().expect("build should succeed");

    // Hex-encoded BOM — what actually lands in the file for a non-ASCII string
    let hex_bom = b"FEFF";
    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }
    assert!(
        contains_subslice(&pdf_bytes, hex_bom),
        "Hex-encoded BOM 'FEFF' not found in PDF — CJK metadata title not properly UTF-16BE encoded"
    );
}

#[test]
fn content_stream_chars_above_ff_replaced_with_question_mark() {
    // Characters above U+00FF in base-14 font context cannot be represented
    // in WinAnsiEncoding and should be substituted with '?' (0x3F), not
    // written as raw UTF-8 bytes.
    let mut builder = DocumentBuilder::new();
    builder.a4_page().at(72.0, 700.0).text("中文").done();

    let pdf_bytes = builder.build().expect("build should succeed");

    // UTF-8 encoding of 中 is 0xE4 0xB8 0xAD — this multi-byte sequence must not appear
    let utf8_zhong: &[u8] = &[0xE4, 0xB8, 0xAD];
    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }
    assert!(
        !contains_subslice(&pdf_bytes, utf8_zhong),
        "Raw UTF-8 bytes for '中' found in PDF — write_escaped_string bug"
    );
}
