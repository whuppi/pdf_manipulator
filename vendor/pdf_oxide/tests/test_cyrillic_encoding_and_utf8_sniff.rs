//! #317 — Cyrillic text must decode correctly through a /Differences
//! encoding that remaps byte codes to Adobe glyph names like `afii10029`.
//!
//! `pdfs_pdfjs/issue20232.pdf` (a Russian engineering title block) emits
//! Cyrillic characters as UTF-8 mojibake (`ÐÐ¸Ñ`) instead of `Лист`. The
//! issue lists three hypotheses (font has no ToUnicode; fallback treats
//! bytes as WinAnsi; UTF-8 is re-encoded twice). Without local access to
//! `issue20232.pdf` we cannot diagnose the specific structural trigger.
//!
//! This test instead pins the common real-world mechanism for Cyrillic
//! content in Type1 fonts: a `/Differences` array that remaps single
//! byte codes to Adobe Cyrillic glyph names (`afii10029` = Л,
//! `afii10074` = и, `afii10083` = с, `afii10036` = Т — together "ЛиСТ",
//! but the mixed case keeps each glyph unambiguously identifiable). The
//! glyph names map to Unicode through Adobe Glyph List
//! (`src/fonts/adobe_glyph_list.rs`).
//!
//! If the test fails today, we have a reproduction for part of the bug.
//! If it passes, this is a regression guard for one of the supported
//! Cyrillic-via-Differences paths.
use pdf_oxide::PdfDocument;

/// Build a 1-page PDF that draws the four bytes `0xC0 0xC1 0xC2 0xC3`
/// under a /Differences array binding those byte positions to
/// Cyrillic Adobe glyph names.
fn cyrillic_differences_pdf() -> Vec<u8> {
    // Content stream writes the four byte codes. `\300` = 0xC0, `\301` =
    // 0xC1, `\302` = 0xC2, `\303` = 0xC3 in PDF literal-string octal
    // escapes.
    let content = b"BT /F0 12 Tf 100 800 Td (\xC0\xC1\xC2\xC3) Tj ET\n";

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
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] \
           /Resources << /Font << /F0 5 0 R >> >> /Contents 4 0 R >>",
    );

    // Object 4: content stream.
    {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(format!("<< /Length {} >>\nstream\n", content.len()).as_bytes());
        out.extend_from_slice(content);
        out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Object 5: Type1 Helvetica with /Differences remapping byte codes
    // 0xC0..=0xC3 to Cyrillic Adobe glyph names.
    //
    // afii10029 = U+041B (Cyrillic capital Л)
    // afii10074 = U+0438 (Cyrillic и)
    // afii10083 = U+0441 (Cyrillic с)
    // afii10036 = U+0422 (Cyrillic capital Т)
    push(
        &mut out,
        &mut offsets,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
           /Encoding << /Type /Encoding /BaseEncoding /WinAnsiEncoding \
             /Differences [192 /afii10029 /afii10074 /afii10083 /afii10036] \
           >> >>",
    );

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
fn cyrillic_differences_encoding_yields_unicode_cyrillic() {
    let pdf = cyrillic_differences_pdf();
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");

    // Expected: "ЛиСТ" — capital Л, lowercase и, lowercase с, capital Т.
    // The mixed case leaves no ambiguity if only part of the mapping
    // resolves.
    assert!(
        text.contains('\u{041B}'),
        "missing Cyrillic Л (U+041B). Text: {text:?} bytes: {:?}",
        text.as_bytes()
    );
    assert!(
        text.contains('\u{0438}'),
        "missing Cyrillic и (U+0438). Text: {text:?} bytes: {:?}",
        text.as_bytes()
    );
    assert!(
        text.contains('\u{0441}'),
        "missing Cyrillic с (U+0441). Text: {text:?} bytes: {:?}",
        text.as_bytes()
    );
    assert!(
        text.contains('\u{0422}'),
        "missing Cyrillic Т (U+0422). Text: {text:?} bytes: {:?}",
        text.as_bytes()
    );

    // Negative: the output must NOT contain the Latin-1 mojibake marker
    // `Ð` (U+00D0) — that's what the bug produces when byte codes get
    // interpreted as WinAnsi instead of mapped through /Differences.
    assert!(
        !text.contains('\u{00D0}'),
        "#317 mojibake symptom: output contains Ð (U+00D0). Text: {text:?}"
    );
}

/// Variant: the producer emits UTF-8 bytes directly inside a PDF string
/// under a plain WinAnsi font (no /Differences, no ToUnicode). This is a
/// spec-violating but real-world pattern — some Russian CAD exporters
/// and report generators do this. pdftotext recovers the Cyrillic text;
/// we now recover it too via post-extraction UTF-8 mojibake repair.
///
/// The test below (`utf8_bytes_under_winansi_font_decode_as_cyrillic`) is
/// enabled and asserts the Cyrillic codepoints are present, with no Ð
/// (U+00D0) mojibake leaking through.
fn utf8_in_winansi_pdf() -> Vec<u8> {
    // "Лист" in UTF-8: 0xD0 0x9B 0xD0 0xB8 0xD1 0x81 0xD1 0x82
    let content: &[u8] = b"BT /F0 12 Tf 100 800 Td (\xD0\x9B\xD0\xB8\xD1\x81\xD1\x82) Tj ET\n";

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
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] \
           /Resources << /Font << /F0 5 0 R >> >> /Contents 4 0 R >>",
    );
    {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(format!("<< /Length {} >>\nstream\n", content.len()).as_bytes());
        out.extend_from_slice(content);
        out.extend_from_slice(b"\nendstream\nendobj\n");
    }
    push(
        &mut out,
        &mut offsets,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
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
fn utf8_bytes_under_winansi_font_decode_as_cyrillic() {
    let pdf = utf8_in_winansi_pdf();
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");

    // "Лист" — we should see each Cyrillic codepoint, not its Latin-1
    // mojibake rendering.
    assert!(text.contains('\u{041B}'), "missing Л. Text: {text:?}");
    assert!(text.contains('\u{0438}'), "missing и. Text: {text:?}");
    assert!(text.contains('\u{0441}'), "missing с. Text: {text:?}");
    assert!(text.contains('\u{0442}'), "missing т. Text: {text:?}");
    assert!(!text.contains('\u{00D0}'), "mojibake Ð (U+00D0) in output. Text: {text:?}");
}
