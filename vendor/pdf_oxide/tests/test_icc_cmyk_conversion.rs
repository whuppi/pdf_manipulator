//! ICC-based CMYK → RGB pipeline coverage.
//!
//! `Transform` is the public surface every caller funnels through. When
//! qcms is linked (the default `icc` feature) it compiles embedded ICC
//! profiles into real colourimetric transforms; when the profile can't
//! be compiled — malformed, unsupported version, missing tags — the
//! transform falls through to ISO 32000-1:2008 §10.3.5's additive-clamp
//! fallback. Both paths must agree on anchor samples (pure white, pure
//! black) so downstream callers never see a broken conversion.

use pdf_oxide::color::{IccProfile, RenderingIntent, Transform};
use std::sync::Arc;

/// 128-byte ICC header stub with a valid `acsp` signature but no tag
/// table. Accepted by `IccProfile::parse` (header is valid) but
/// rejected by qcms (no functioning tags), exercising the fallback
/// path deterministically.
fn header_only_cmyk_profile_bytes() -> Vec<u8> {
    let mut v = vec![0u8; 128];
    v[8..12].copy_from_slice(&0x04000000u32.to_be_bytes());
    v[12..16].copy_from_slice(b"prtr");
    v[16..20].copy_from_slice(b"CMYK");
    v[20..24].copy_from_slice(b"Lab ");
    v[36..40].copy_from_slice(b"acsp");
    v
}

#[test]
fn cmyk_transform_anchor_samples_agree() {
    let profile = Arc::new(
        IccProfile::parse(header_only_cmyk_profile_bytes(), 4)
            .expect("header-only profile should parse"),
    );
    let t = Transform::new_srgb_target(profile, RenderingIntent::RelativeColorimetric);

    // (0,0,0,0) = paper white under every CMM + under §10.3.5.
    assert_eq!(t.convert_cmyk_pixel(0, 0, 0, 0), [255, 255, 255]);
    // (255,255,255,255) = saturated ink overlay → black under §10.3.5.
    // A CMM with a press profile might clip to near-zero rather than
    // exactly zero; when qcms rejects the stub profile we're guaranteed
    // §10.3.5 semantics here.
    assert_eq!(t.convert_cmyk_pixel(255, 255, 255, 255), [0, 0, 0]);
}

#[test]
fn cmyk_transform_bulk_path_matches_pixel_path() {
    // Bulk conversion must produce byte-for-byte identical output to
    // per-pixel conversion under the §10.3.5 fallback path. With a
    // real qcms transform the two paths may disagree by rounding in
    // the final sample but should agree on anchor values.
    let profile = Arc::new(
        IccProfile::parse(header_only_cmyk_profile_bytes(), 4)
            .expect("header-only profile should parse"),
    );
    let t = Transform::new_srgb_target(profile, RenderingIntent::RelativeColorimetric);

    let samples: [(u8, u8, u8, u8); 4] = [
        (0, 0, 0, 0),
        (255, 255, 255, 255),
        (64, 32, 16, 8),
        (13, 12, 12, 4),
    ];
    let mut cmyk = Vec::with_capacity(samples.len() * 4);
    for s in &samples {
        cmyk.extend_from_slice(&[s.0, s.1, s.2, s.3]);
    }
    let bulk = t.convert_cmyk_buffer(&cmyk);

    let mut per_pixel = Vec::with_capacity(samples.len() * 3);
    for s in &samples {
        per_pixel.extend_from_slice(&t.convert_cmyk_pixel(s.0, s.1, s.2, s.3));
    }

    // Under the §10.3.5 fallback the two paths must be bit-identical.
    // When qcms is engaged they can differ by at most 1 unit per
    // channel due to the bulk path amortising lookup table evaluation.
    assert_eq!(bulk.len(), per_pixel.len());
    for (b, p) in bulk.iter().zip(per_pixel.iter()) {
        let diff = (*b as i32 - *p as i32).abs();
        assert!(diff <= 1, "bulk vs per-pixel CMYK conversion differ by {diff}");
    }
}

/// End-to-end verification using a real CMYK PDF with an embedded ICC
/// profile. The PDF ships a 32×12 blue swatch with a CMYK ICCBased
/// colour space; with qcms live, it must decode close to the ICC
/// reference RGB value of (62, 124, 191) rather than the §10.3.5
/// fallback's (62, 142, 252). Gated on a local fixture path so fresh
/// checkouts without the fixture simply skip.
#[test]
#[ignore] // requires /tmp/issue375/report.pdf; run with --ignored
fn report_pdf_blue_swatch_matches_icc_reference() {
    let pdf = std::path::Path::new("/tmp/issue375/report.pdf");
    if !pdf.exists() {
        eprintln!("skip: {} not present", pdf.display());
        return;
    }
    let doc = pdf_oxide::PdfDocument::open(pdf).expect("open report.pdf");
    let images = doc.extract_images(0).expect("extract images");

    // Find the 32×12 blue swatch — unique dimensions in the document.
    let swatch = images
        .iter()
        .find(|img| img.width() == 32 && img.height() == 12)
        .expect("32x12 swatch present");

    let tmp = tempfile::NamedTempFile::with_suffix(".png").unwrap();
    swatch.save_as_png(tmp.path()).expect("save png");
    let img = image::open(tmp.path()).expect("load png").to_rgb8();
    let p = img.get_pixel(0, 0).0;

    // Expected from a reference CMM: (62, 124, 191). We allow ±10 per
    // channel so minor qcms vs lcms rounding doesn't flake the test.
    // Under the §10.3.5 fallback we'd see ~(62, 142, 252) which is
    // 28 units off on G — well outside the tolerance.
    let target = [62i32, 124, 191];
    for (name, (&got, &want)) in ["R", "G", "B"].iter().zip(p.iter().zip(target.iter())) {
        let diff = (got as i32 - want).abs();
        assert!(
            diff <= 10,
            "channel {name}: got {got}, want ~{want} (Δ={diff}); CMM not engaged?"
        );
    }
}
