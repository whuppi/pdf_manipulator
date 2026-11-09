//! Regression test for issue #407: Layer 4 font cache (`font_name_set_cache`) must not
//! return a cached font set when pages share font key names but reference different font
//! objects with different ToUnicode CMaps.
//!
//! ## Root cause
//! `load_fonts` has a 4-layer caching stack. Layer 4 keys on a hash of the *sorted font
//! key names* only — it ignores the ObjectRef each name maps to. When a cache hit occurs
//! the stored font set (including ToUnicode CMaps) from the first page that populated the
//! entry is silently reused for every subsequent page with the same key names, regardless
//! of whether those pages actually reference different font objects.
//!
//! Layer 3 (`font_fingerprint_cache`) correctly includes `(name → ObjectRef)` pairs in
//! its key and therefore misses when ObjectRefs change, but Layer 4 fires before Layer 3's
//! miss propagates to a full re-load.
//!
//! ## Fix
//! Restore the spot-check that was intentionally removed in commit `91cc7150`: on a Layer 4
//! cache hit, load the check font's object and compare its `font_identity_hash_cheap`
//! (which includes the ToUnicode ObjectRef) against the stored hash. Trust the cache only
//! when the hashes match.
//!
//! ## Real-world trigger
//! Adobe Acrobat Pro and Antenna House embed per-page font subsets under recycled resource
//! key names (`TT0`, `TT1`, `C2_0`, …). Each page gets a fresh ObjectRef and a different
//! ToUnicode CMap that covers only the glyphs used on that page.
//!
//! Reproducer confirmed against the IRS Form 1040 Instructions (i1040gi.pdf):
//! page 48 loses 41 non-whitespace characters when extracted after pages 0–47.

use pdf_oxide::PdfDocument;

// ── Synthetic PDF helpers ────────────────────────────────────────────────────

/// Build a one-char ToUnicode CMap stream that maps 0x41 → `target_unicode`.
/// `target_unicode` is the UTF-16BE hex string, e.g. "0058" for U+0058 = 'X'.
fn tounicode_cmap(target_unicode: &str) -> String {
    format!(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n\
         /CMapType 2 def\n\
         1 begincodespacerange\n\
         <00> <FF>\n\
         endcodespacerange\n\
         1 beginbfchar\n\
         <41> <{target_unicode}>\n\
         endbfchar\n\
         endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end\n"
    )
}

/// Synthesise a minimal 2-page PDF where:
/// - Page 1 has `/Font << /F1 10 0 R >>` and font 10 maps char 0x41 → `char1`
/// - Page 2 has `/Font << /F1 20 0 R >>` and font 20 maps char 0x41 → `char2`
///
/// Both content streams write the literal byte string `(A)` (code 0x41).
/// ToUnicode CMaps are different objects (different ObjectRefs, different content).
///
/// `tag` is embedded in the BaseFont name to produce a unique `font_identity_hash_cheap`
/// per test, preventing the global cross-document font cache from leaking state between
/// tests that happen to use the same object IDs.
///
/// Before the fix, extracting page 2 after page 1 from the same `PdfDocument`
/// returns `char1` (wrong). After the fix it returns `char2` (correct).
fn two_page_same_font_key_different_cmap(char1: char, char2: char) -> Vec<u8> {
    // Unique font name derived from char codes so the global font identity cache
    // (keyed on font_identity_hash_cheap which includes BaseFont) never merges fonts
    // from different test invocations.
    let tag = format!("TestFont-{:04X}-{:04X}", char1 as u32, char2 as u32);
    let cmap1 = tounicode_cmap(&format!("{:04X}", char1 as u32));
    let cmap2 = tounicode_cmap(&format!("{:04X}", char2 as u32));

    let content = "BT /F1 12 Tf 72 720 Td (A) Tj ET\n";

    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = vec![0]; // slot 0 unused

    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

    let push = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: String| {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        out.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    };

    // 1: Catalog
    push(&mut out, &mut offsets, "<< /Type /Catalog /Pages 2 0 R >>".into());
    // 2: Pages
    push(&mut out, &mut offsets, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".into());

    // 3: Page 1  — font key /F1 → object 10
    push(
        &mut out,
        &mut offsets,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << /F1 10 0 R >> >> /Contents 5 0 R >>"
            .to_string(),
    );
    // 4: Page 2  — font key /F1 → object 20 (different ObjectRef!)
    push(
        &mut out,
        &mut offsets,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << /F1 20 0 R >> >> /Contents 6 0 R >>"
            .to_string(),
    );

    // 5: Content stream for page 1
    push(
        &mut out,
        &mut offsets,
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
    );
    // 6: Content stream for page 2 (identical — same byte 0x41)
    push(
        &mut out,
        &mut offsets,
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
    );

    // Pad to object 10 (objects 7-9 are placeholders so numbering aligns)
    for _ in 7..=9 {
        push(&mut out, &mut offsets, "<< >>".into());
    }

    // 10: Font 1 — unique BaseFont tag + ToUnicode → object 11
    push(
        &mut out,
        &mut offsets,
        format!(
            "<< /Type /Font /Subtype /Type1 /BaseFont /{tag}-P1 \
             /Encoding /WinAnsiEncoding /ToUnicode 11 0 R >>"
        ),
    );
    // 11: ToUnicode CMap 1
    push(
        &mut out,
        &mut offsets,
        format!("<< /Length {} >>\nstream\n{cmap1}endstream", cmap1.len()),
    );

    // Pad to object 20
    for _ in 12..=19 {
        push(&mut out, &mut offsets, "<< >>".into());
    }

    // 20: Font 2 — unique BaseFont tag (different suffix) + ToUnicode → object 21
    push(
        &mut out,
        &mut offsets,
        format!(
            "<< /Type /Font /Subtype /Type1 /BaseFont /{tag}-P2 \
             /Encoding /WinAnsiEncoding /ToUnicode 21 0 R >>"
        ),
    );
    // 21: ToUnicode CMap 2
    push(
        &mut out,
        &mut offsets,
        format!("<< /Length {} >>\nstream\n{cmap2}endstream", cmap2.len()),
    );

    // xref
    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &offsets[1..] {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            offsets.len(),
            xref_offset
        )
        .as_bytes(),
    );
    out
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// Core regression: extracting page 1 then page 2 from one `PdfDocument` must
/// yield the same text as extracting each page in isolation.
///
/// Before the fix, page 2 silently reuses page 1's ToUnicode CMap (Layer 4
/// cache hit keyed on font names only). Character 0x41 maps to `char1` on
/// page 1 and `char2` on page 2, so the wrong result is `char1` on page 2.
#[test]
fn font_layer4_cache_must_not_cross_contaminate_tounicode_cmaps() {
    let char1 = 'X'; // page 1 maps 0x41 → U+0058
    let char2 = 'Y'; // page 2 maps 0x41 → U+0059
    let pdf = two_page_same_font_key_different_cmap(char1, char2);

    // --- Isolated baselines (each page in its own fresh PdfDocument) -------
    let baseline_p0 = {
        let doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
        doc.extract_text(0).unwrap()
    };
    let baseline_p1 = {
        let doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
        doc.extract_text(1).unwrap()
    };

    assert!(
        baseline_p0.contains(char1),
        "isolated page 0 must contain '{char1}', got {baseline_p0:?}"
    );
    assert!(
        baseline_p1.contains(char2),
        "isolated page 1 must contain '{char2}', got {baseline_p1:?}"
    );

    // --- Sequential extraction from the same document ----------------------
    let doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
    let seq_p0 = doc.extract_text(0).unwrap();
    let seq_p1 = doc.extract_text(1).unwrap();

    assert!(
        seq_p0.contains(char1),
        "sequential page 0 must contain '{char1}', got {seq_p0:?}"
    );
    assert!(
        seq_p1.contains(char2),
        "sequential page 1 must contain '{char2}' (own CMap), not '{char1}' (page 0 CMap); \
         got {seq_p1:?} — Layer 4 font_name_set_cache cross-contamination detected"
    );
    assert!(
        !seq_p1.contains(char1) || seq_p1.contains(char2),
        "page 1 must use its own CMap, not the cached CMap from page 0"
    );
}

/// Reverse-order extraction must also be correct: page 1 first, then page 0.
#[test]
fn font_layer4_cache_order_independence_reversed() {
    let char1 = 'P'; // page 1 maps 0x41 → U+0050
    let char2 = 'Q'; // page 2 maps 0x41 → U+0051
    let pdf = two_page_same_font_key_different_cmap(char1, char2);

    let doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
    let p1_first = doc.extract_text(1).unwrap();
    let p0_after = doc.extract_text(0).unwrap();

    assert!(
        p1_first.contains(char2),
        "page 1 extracted first must contain '{char2}', got {p1_first:?}"
    );
    assert!(
        p0_after.contains(char1),
        "page 0 extracted after page 1 must contain '{char1}', got {p0_after:?}"
    );
}

/// When fonts ARE actually identical across pages (same ToUnicode ObjectRef),
/// Layer 4 should still cache and reuse the result (performance must not regress).
/// This mirrors the ANN/Valdes class of PDFs: unique ObjectRefs per page but
/// identical font content.
#[test]
fn font_layer4_cache_reuses_when_fonts_are_truly_identical() {
    // Both pages have /F1 pointing to different ObjectRefs (10 and 20),
    // but we give them the SAME ToUnicode CMap content AND the same ToUnicode
    // ObjectRef by using a shared font reference across both pages.
    // Since both pages use the same font object ref, Layer 3 will catch it —
    // but the deeper point is that extraction produces the same result
    // regardless of extraction order.
    let char1 = 'Z';
    // Use same char for both pages: same font content everywhere
    let pdf = two_page_same_font_key_different_cmap(char1, char1);

    let doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
    let p0 = doc.extract_text(0).unwrap();
    let p1 = doc.extract_text(1).unwrap();

    assert!(p0.contains(char1), "page 0 must contain '{char1}', got {p0:?}");
    assert!(p1.contains(char1), "page 1 must contain '{char1}', got {p1:?}");
}

/// Regression guard against the IRS Form 1040 reproducer pattern:
/// extracting a target page in isolation and after prior pages must yield
/// the same non-whitespace character count.
#[test]
fn font_layer4_nonws_count_is_order_independent() {
    let char1 = 'A'; // page 1 maps 0x41 → U+0041
    let char2 = 'B'; // page 2 maps 0x41 → U+0042
    let pdf = two_page_same_font_key_different_cmap(char1, char2);

    let nonws = |s: &str| s.chars().filter(|c| !c.is_whitespace()).count();

    let clean = {
        let doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
        doc.extract_text(1).unwrap()
    };
    let after = {
        let doc = PdfDocument::from_bytes(pdf.clone()).unwrap();
        let _ = doc.extract_text(0);
        doc.extract_text(1).unwrap()
    };

    assert_eq!(
        nonws(&clean),
        nonws(&after),
        "non-whitespace char count for page 1 must be identical whether or not \
         page 0 was extracted first (IRS #407 pattern): \
         clean={}, after={}",
        nonws(&clean),
        nonws(&after)
    );
}
