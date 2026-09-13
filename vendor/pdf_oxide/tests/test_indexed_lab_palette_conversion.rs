//! #337 — Indexed color spaces with Lab base must convert palette bytes
//! colorimetrically to RGB, not interpret them as already-RGB.
//!
//! Per PDF 32000-1:2008 §8.6.6.3, the base of an Indexed color space can
//! be any device-independent color space (Lab, CalRGB, CalGray, ...).
//! Lab->XYZ->sRGB palette conversion is now implemented, so this test
//! validates that a synthetic Indexed+Lab PDF decodes to the expected
//! sRGB mid-gray for Lab(50, 0, 0).
use pdf_oxide::extractors::ImageData;
use pdf_oxide::PdfDocument;

/// Minimal 10x10-pixel PDF with a single-entry Lab-base Indexed palette.
/// The palette entry encodes Lab = (50, 0, 0) — perceptual mid-gray.
///
/// After correct Lab → XYZ (D65 whitepoint) → sRGB conversion with
/// standard sRGB gamma encoding, Lab(50, 0, 0) should land near
/// sRGB(119, 119, 119), which is linear-sRGB 0.18406 raised through
/// the sRGB transfer function (1.055·x^(1/2.4) − 0.055).
///
/// Byte encoding per PDF spec §8.6.5.4 with default /Range [−128 127]:
///   L* byte = round(L* · 255 / 100) = round(50 · 2.55) = 128
///   a* byte = a* + 128 = 128
///   b* byte = b* + 128 = 128
///
/// Image is 10x10 (not 1x1) so it clears the default 8px minimum
/// dimension filter in `ImageExtractFilter`.
fn indexed_lab_single_entry_pdf() -> Vec<u8> {
    // 10x10 8-bpc indexed image, single palette entry at index 0.
    // Raw image data: 100 bytes, each with value 0 = palette index 0.
    let image_data: Vec<u8> = vec![0; 100];
    // Palette: Lab triple (128, 128, 128).
    let palette: &[u8] = &[128, 128, 128];

    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = vec![0];
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let push = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };

    push(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    push(&mut out, &mut offsets, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    push(
        &mut out,
        &mut offsets,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] \
           /Resources << /XObject << /Im0 5 0 R >> >> \
           /Contents 4 0 R >>",
    );

    // Object 4: minimal content stream that paints the image.
    let cs_body = b"q 10 0 0 10 0 0 cm /Im0 Do Q\n";
    {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(format!("<< /Length {} >>\nstream\n", cs_body.len()).as_bytes());
        out.extend_from_slice(cs_body);
        out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Object 5: Indexed+Lab image. /ColorSpace is an inline array with
    // Lab as base, hival 0, and an inline-hex-string palette.
    //
    // The Lab array carries `/WhitePoint [0.9505 1.0 1.0890]` (D65) and
    // default /Range.
    {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        let palette_hex: String = palette.iter().map(|b| format!("{b:02X}")).collect();
        let dict = format!(
            "<< /Type /XObject /Subtype /Image /Width 10 /Height 10 \
               /BitsPerComponent 8 \
               /ColorSpace [/Indexed \
                 [/Lab << /WhitePoint [0.9505 1.0 1.0890] >>] \
                 0 <{palette_hex}>] \
               /Length {} >>",
            image_data.len()
        );
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(dict.as_bytes());
        out.extend_from_slice(b"\nstream\n");
        out.extend_from_slice(&image_data);
        out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &offsets[1..] {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
    out
}

#[test]
fn indexed_lab_mid_gray_yields_srgb_mid_gray() {
    let pdf = indexed_lab_single_entry_pdf();
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let images = doc.extract_images(0).expect("extract images");
    assert!(!images.is_empty(), "page should yield one image");
    let img = &images[0];

    let pixels = match img.data() {
        ImageData::Raw { pixels, .. } => pixels.clone(),
        ImageData::Jpeg(_) => {
            panic!("expected raw-pixel image, got JPEG; test setup is wrong");
        },
    };

    // The palette has Lab (50, 0, 0). After correct conversion this is
    // sRGB (119, 119, 119) ±3 to absorb rounding differences between
    // reference color-math implementations (lcms2 vs skcms vs pdfium).
    //
    // The current code path treats the palette bytes (128, 128, 128) as
    // already-RGB, so the decoded pixel is (128, 128, 128) — close but
    // wrong. Relax the tolerance below only when the math is actually
    // implemented; the tight tolerance is intentional so a placeholder
    // implementation is rejected.
    assert!(pixels.len() >= 3, "need at least one RGB pixel");
    for (label, v) in [("R", pixels[0]), ("G", pixels[1]), ("B", pixels[2])] {
        let expected: i32 = 119;
        let diff = (v as i32 - expected).abs();
        assert!(
            diff <= 3,
            "Lab(50,0,0) channel {label}: expected ~{expected}, got {v} (Δ={diff}). \
             Lab→XYZ→sRGB conversion missing."
        );
    }
}
