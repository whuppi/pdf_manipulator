//! The streaming redaction pass: the text decisions it makes, the bytes it
//! writes around them, and the refusals it makes.

use crate::error::{Error, Result};
use crate::host::content::{Mode, Operators};
use crate::redaction::text_engine::FontMetrics;
use crate::redaction::transform::{ImageResources, RedactPass};
use crate::redaction::{RedactionOptions, RedactionRegion, RedactionReport, RegionSet};

/// Fixed-pitch stub: every code is 500/1000 em wide, simple one-byte.
struct Stub;

impl FontMetrics for Stub {
    fn width(&self, _font: &str, _code: u32) -> f32 {
        500.0
    }
}

/// Every font is composite, so the pass must refuse any show.
struct Composite;

impl FontMetrics for Composite {
    fn width(&self, _font: &str, _code: u32) -> f32 {
        500.0
    }

    fn is_simple(&self, _font: &str) -> bool {
        false
    }
}

/// The secret bytes must not appear anywhere in the output stream.
fn assert_absent(out: &[u8], secret: &[u8]) {
    assert!(
        out.windows(secret.len()).all(|w| w != secret),
        "secret {:?} still present in redacted output: {:?}",
        String::from_utf8_lossy(secret),
        String::from_utf8_lossy(out)
    );
}

fn one_region(x0: f32, y0: f32, x1: f32, y1: f32) -> RegionSet {
    let mut set = RegionSet::new(0);
    set.push(RedactionRegion::from_rect(x0, y0, x1, y1, Some([0.0, 0.0, 0.0])));
    set
}

fn redact(
    content: &[u8],
    regions: &RegionSet,
    opts: &RedactionOptions,
    fonts: &dyn FontMetrics,
) -> Result<(Vec<u8>, RedactionReport)> {
    let mut out = Vec::new();
    let images = ImageResources::none();
    let outcome = RedactPass::new(regions, opts, fonts, &images)
        .run(Operators::new(content, Mode::Full), &mut out)?;
    Ok((out, outcome.report))
}

const SECRET_DOC: &[u8] = b"BT\n/F1 10 Tf\n1 0 0 1 100 700 Tm\n(TOPSECRET) Tj\nET\n";

/// No battery sees this: the pass runs over bare operator bytes, before any
/// document is saved, so only the pass itself can show what it removed.
#[test]
fn secret_fully_in_region_is_removed_and_overlaid() {
    // "TOPSECRET" 9 glyphs ×5pt from x=100 → 100..145 at y≈700.
    let regions = one_region(90.0, 695.0, 160.0, 715.0);
    let (out, report) = redact(SECRET_DOC, &regions, &RedactionOptions::default(), &Stub).unwrap();
    assert_absent(&out, b"TOPSECRET");
    assert_absent(&out, b"SECRET");
    assert_eq!(report.glyphs_removed, 9);
    assert_eq!(report.regions, 1);
    assert!(report.bytes_removed > 0);
    // An opaque overlay block is present (G7): the overlay emits
    // `q\n<r> <g> <b> rg\n<x> <y> <w> <h> re\nf\nQ\n`.
    let s = String::from_utf8_lossy(&out);
    assert!(
        s.contains("rg\n") && s.contains(" re\n") && s.contains("\nf\n"),
        "overlay missing in: {s}"
    );
}

/// No battery sees this: the pass runs over bare operator bytes, before any
/// document is saved, so only the pass itself can show what it kept.
#[test]
fn public_text_outside_region_survives_verbatim() {
    let doc = b"BT\n/F1 10 Tf\n1 0 0 1 100 700 Tm\n(PUBLIC) Tj\nET\n";
    let regions = one_region(0.0, 0.0, 5.0, 5.0); // far away
    let (out, report) = redact(doc, &regions, &RedactionOptions::default(), &Stub).unwrap();
    assert_eq!(report.glyphs_removed, 0);
    assert!(
        out.windows(6).any(|w| w == b"PUBLIC"),
        "public text must survive: {}",
        String::from_utf8_lossy(&out)
    );
}

/// No battery sees this: the pass runs over bare operator bytes, before any
/// document is saved, so only the pass itself can show what it split.
#[test]
fn straddling_secret_partially_removed_public_kept() {
    // "PUBSECRET": region from x≈124.5 onward removes "SECRET",
    // keeps "PUB". Glyph i at x = 100 + 5i: P100 U105 B110 S115…
    let doc = b"BT\n/F1 10 Tf\n1 0 0 1 100 700 Tm\n(PUBSECRET) Tj\nET\n";
    let regions = one_region(120.0, 695.0, 400.0, 715.0);
    let (out, report) = redact(doc, &regions, &RedactionOptions::default(), &Stub).unwrap();
    assert_absent(&out, b"SECRET");
    assert!(report.glyphs_removed >= 6);
    assert!(
        out.windows(3).any(|w| w == b"PUB"),
        "public prefix must survive: {}",
        String::from_utf8_lossy(&out)
    );
}

/// No battery sees this: the refusal happens inside the pass before any byte
/// is returned, and a battery sees only the save that never happens.
#[test]
fn composite_font_is_refused_not_under_redacted() {
    let regions = one_region(0.0, 0.0, 1000.0, 1000.0);
    let err = redact(SECRET_DOC, &regions, &RedactionOptions::default(), &Composite).unwrap_err();
    assert!(matches!(err, Error::Unsupported(_)), "expected refusal, got {err:?}");
}

/// No battery sees this: the pass sees the operators and their damage, which
/// a document-level battery reaches only through a whole save.
#[test]
fn no_regions_keeps_content_and_draws_nothing() {
    let (out, report) =
        redact(SECRET_DOC, &RegionSet::new(0), &RedactionOptions::default(), &Stub).unwrap();
    assert_eq!(report.glyphs_removed, 0);
    assert_eq!(report.regions, 0);
    // Nothing redacted, no overlay; the text is intact.
    assert!(out.windows(9).any(|w| w == b"TOPSECRET"));
}

/// No battery sees this: the pass runs over bare operator bytes, and a
/// malformed stream is a case of the operator reader, not a document.
#[test]
fn malformed_stream_is_a_clean_error_not_a_panic() {
    let regions = one_region(0.0, 0.0, 1000.0, 1000.0);
    let _ = redact(b"q Q (unbalanced", &regions, &RedactionOptions::default(), &Stub);
    let _ = redact(b"", &regions, &RedactionOptions::default(), &Stub);
}

/// No battery sees this: the wrapper is a property of the bytes the pass writes
/// around its operators, not of any document a battery could open.
#[test]
fn pruned_body_wrapped_in_own_q_before_overlay() {
    // A trailing, unmatched `cm` (e.g. a Y-flip a producer applies once and
    // never restores — legal per ISO 32000-1: the CTM is not required to be
    // identity at end-of-stream) must not leak into the overlay, which draws in
    // the region's absolute page-space coordinates. The pruned body must be
    // wrapped in its own `q`/`Q` so the overlay always starts from the stream's
    // original CTM.
    let doc = b"1 0 0 -1 0 792 cm\n";
    let regions = one_region(90.0, 695.0, 160.0, 715.0);
    let (out, _report) = redact(doc, &regions, &RedactionOptions::default(), &Stub).unwrap();

    let s = String::from_utf8_lossy(&out);
    assert!(s.starts_with("q\n"), "pruned body must open with q: {s}");
    let wrapper_q_close = s.find("Q\n").expect("closing Q for the pruned-body wrapper");
    let overlay_rg = s.find("rg\n").expect("overlay rg op");
    assert!(
        wrapper_q_close < overlay_rg,
        "the wrapper Q must close (restoring the original CTM) before the \
         overlay starts drawing: {s}"
    );
}

/// No battery sees this: a damaged stream is refused inside the pass, which
/// writes nothing a caller could mistake for a redaction.
#[test]
fn damaged_stream_is_refused() {
    let content = b"q ) ) Q BT /F1 10 Tf 1 0 0 1 100 700 Tm (SECRET) Tj ET\n";
    let regions = one_region(90.0, 695.0, 160.0, 715.0);
    assert!(matches!(
        redact(content, &regions, &RedactionOptions::default(), &Stub),
        Err(Error::Unsupported(_))
    ));
}
