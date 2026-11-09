//! `set_excluded_layers` must mean the same thing on a large content stream
//! as on a small one.
//!
//! ISO 32000-1:2008 §8.11 lets a conforming reader choose whether to honour
//! optional-content state, and explicitly blesses callers supplying their own
// ── pdf_manipulator patch: the header described a mechanism that is gone ──
//! — so returning hidden text *by default* is legitimate. What is not is
//! offering that mechanism and then ignoring it: a reader that keeps only
//! `BT..ET`/`Do` regions of a long stream discards the `BDC`/`EMC` pairs
//! carrying optional-content membership, so the exclusion silently does
//! nothing. The caller asked, got no error, and got the content — and hidden
//! layers routinely hold draft text and pre-redaction content.
//!
//! These tests pin both filters, on both sides of a 256 KB stream size, so a
//! reader added for long streams cannot inherit the same hole unnoticed.
// ── end pdf_manipulator patch ──

use std::collections::HashSet;

use pdf_oxide::PdfDocument;

/// One tagged-with-optional-content page. `padding_ops` no-op operators are
// ── pdf_manipulator patch: the comment described a mechanism that is gone ──
/// appended so the caller can push the stream over 256 KB
/// without changing what the page says.
// ── end pdf_manipulator patch ──
fn ocg_pdf(padding_ops: usize) -> Vec<u8> {
    let mut content = String::new();
    content.push_str("BT /F1 12 Tf 1 0 0 1 72 700 Tm (VisibleBody) Tj ET\n");
    content.push_str("/OC /MC0 BDC\n");
    content.push_str("BT /F1 12 Tf 1 0 0 1 72 660 Tm (SecretDraft) Tj ET\n");
    content.push_str("EMC\n");
    // Padding that is legal, side-effect free, and outside any text object,
    // ── pdf_manipulator patch: the comment described a mechanism that is gone ──
    // so it is exactly the material a text-region reader discards.
    // ── end pdf_manipulator patch ──
    for _ in 0..padding_ops {
        content.push_str("q 1 0 0 1 0 0 cm Q\n");
    }

    let mut buf: Vec<u8> = Vec::new();
    let mut off = vec![0usize; 8];
    let obj = |buf: &mut Vec<u8>, off: &mut Vec<usize>, id: usize, body: &str| {
        off[id] = buf.len();
        buf.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    };

    buf.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
    obj(
        &mut buf,
        &mut off,
        1,
        "<< /Type /Catalog /Pages 2 0 R /OCProperties \
         << /OCGs [6 0 R] /D << /Order [6 0 R] /ON [6 0 R] >> >> >>",
    );
    obj(&mut buf, &mut off, 2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    obj(
        &mut buf,
        &mut off,
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /Font << /F1 5 0 R >> /Properties << /MC0 6 0 R >> >> \
         /Contents 4 0 R >>",
    );
    off[4] = buf.len();
    buf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
    buf.extend_from_slice(content.as_bytes());
    buf.extend_from_slice(b"\nendstream\nendobj\n");
    obj(
        &mut buf,
        &mut off,
        5,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    obj(&mut buf, &mut off, 6, "<< /Type /OCG /Name (Draft) >>");

    let xref = buf.len();
    buf.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
    for id in 1..=6 {
        buf.extend_from_slice(format!("{:010} 00000 n \n", off[id]).as_bytes());
    }
    buf.extend_from_slice(b"trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n");
    buf.extend_from_slice(format!("{xref}\n%%EOF\n").as_bytes());
    buf
}

/// Enough padding operators to clear 256 KB.
const OVER_THRESHOLD: usize = 20_000;

fn excluding_draft() -> HashSet<String> {
    HashSet::from(["Draft".to_string()])
}

/// Confirm the fixture really does straddle the threshold, so the two halves
/// of this file are testing what they claim to.
#[test]
// ── pdf_manipulator patch: the name described a mechanism that is gone ──
fn fixture_sizes_straddle_256_kb() {
    // ── end pdf_manipulator patch ──
    assert!(ocg_pdf(0).len() < 256 * 1024, "small fixture must be under 256 KB");
    assert!(ocg_pdf(OVER_THRESHOLD).len() > 256 * 1024, "large fixture must be over 256 KB");
}

/// The control: below the threshold, exclusion has always worked.
#[test]
fn excluded_layer_is_absent_from_chars_on_a_small_stream() {
    let doc = PdfDocument::from_bytes(ocg_pdf(0)).expect("parse");
    let chars = doc
        .extract_chars_filtered(0, excluding_draft(), HashSet::new())
        .expect("extract");
    let text: String = chars.iter().map(|c| c.char).collect();
    assert!(text.contains("VisibleBody"), "body text lost: {text:?}");
    assert!(!text.contains("SecretDraft"), "excluded layer leaked: {text:?}");
}

/// The defect: above the threshold the same call returned the hidden layer.
#[test]
fn excluded_layer_is_absent_from_chars_on_a_large_stream() {
    let doc = PdfDocument::from_bytes(ocg_pdf(OVER_THRESHOLD)).expect("parse");
    let chars = doc
        .extract_chars_filtered(0, excluding_draft(), HashSet::new())
        .expect("extract");
    let text: String = chars.iter().map(|c| c.char).collect();
    assert!(text.contains("VisibleBody"), "body text lost: {text:?}");
    assert!(
        !text.contains("SecretDraft"),
        "excluded layer leaked above the 256 KB threshold: {text:?}"
    );
}

/// The same contract on the span surface, which shares the gate.
#[test]
fn excluded_layer_is_absent_from_text_on_a_large_stream() {
    let doc = PdfDocument::from_bytes(ocg_pdf(OVER_THRESHOLD)).expect("parse");
    let text = doc
        .extract_text_filtered(0, excluding_draft(), HashSet::new())
        .expect("extract");
    assert!(text.contains("VisibleBody"), "body text lost: {text:?}");
    assert!(
        !text.contains("SecretDraft"),
        "excluded layer leaked above the 256 KB threshold: {text:?}"
    );
}

/// Asking for no exclusion must still return everything, on both sides of the
/// threshold — the fix must not turn the filter on by accident.
#[test]
fn unfiltered_extraction_still_returns_every_layer() {
    for padding in [0, OVER_THRESHOLD] {
        let doc = PdfDocument::from_bytes(ocg_pdf(padding)).expect("parse");
        let text = doc
            .extract_text_filtered(0, HashSet::new(), HashSet::new())
            .expect("extract");
        assert!(text.contains("VisibleBody"), "body lost at padding {padding}: {text:?}");
        assert!(
            text.contains("SecretDraft"),
            "an unfiltered read must return hidden layers too (padding {padding}): {text:?}"
        );
    }
}
