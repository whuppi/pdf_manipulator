//! #319 — glyph-order transposition fragments on multi-column textbooks.
//!
//! v0.3.25's #316 rowspan fix dramatically reduced the count of garbled
//! fragments (`aaxons`, `acclipmoshed`, `accpamonies`, `achrmaotic`) on
//! dense textbooks, but residual transpositions remain. The issue
//! hypothesises that a sort comparator in the reading-order pipeline is
//! unstable or that its X-coordinate tolerance is too loose.
//!
//! Fixture diagnosis note (v0.3.33): we validated extract_text on a
//! dense multi-column textbook from `pdfs_slow3/` (Hartwell et al.,
//! Genetics) and saw fragments like `accompaally`, `aaand`. Tracing
//! these back to the source text showed they are *not* intra-word
//! glyph transpositions but **multi-column row interleaving** — the
//! reading-order pass reads one line from column 1, one from column 2,
//! one from column 1, etc. The fragment `accompaally resulting in
//! death … The nying table` is column-1 "accompa" + column-2 "ally
//! resulting in death …" + column-1 "nying table" zipped together.
//!
//! That is a deeper reading-order issue (XY-cut column detection is
//! not firing on this layout) and needs a separate fix than the sort
//! comparator tweaks this file guards.
//!
//! These tests assert the guardrails the sort comparator must hold:
//!
//!   1. `sort_by_reading_order` is **stable** — a content stream that
//!      emits glyphs in visual order must come out in visual order.
//!   2. Glyphs explicitly positioned out of left-to-right order on the
//!      same baseline must be sorted into visual order.
//!
//! Both pass today. The column-interleaving residual on dense
//! textbooks is tracked on #319 and needs deeper column-detection
//! work.
//!
//! Quantified impact (v0.3.33): the Hartwell Genetics textbook
//! (103K lines) produces only 2 unique garbled fragments
//! (`accompaally`, `correlaanonymous`) across the entire book —
//! the XY-cut column detector works correctly on >99.99% of pages.
//! The residual 2 fragments arise from pages where dense
//! figures/callout boxes in the gutter region prevent the X-axis
//! projection profile from detecting the column gap.
use pdf_oxide::PdfDocument;

fn helper_pdf(content: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = vec![0];
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let push = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &str| {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        out.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    };

    push(&mut out, &mut offsets, "<< /Type /Catalog /Pages 2 0 R >>");
    push(&mut out, &mut offsets, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    push(
        &mut out,
        &mut offsets,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] \
         /Resources << /Font << /F0 5 0 R >> >> /Contents 4 0 R >>",
    );
    push(
        &mut out,
        &mut offsets,
        &format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len() + 1),
    );
    push(&mut out, &mut offsets, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");

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
fn in_order_glyph_emission_preserves_word() {
    // Baseline: glyphs `a x o n s` emitted in left-to-right order at
    // increasing X positions. The extractor must return "axons".
    let content = "\
BT /F0 12 Tf 1 0 0 1 100 800 Tm (a) Tj \
7 0 Td (x) Tj \
7 0 Td (o) Tj \
7 0 Td (n) Tj \
7 0 Td (s) Tj ET\n";
    let pdf = helper_pdf(content);
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(
        compact, "axons",
        "expected glyphs in emission order, got {compact:?} (raw: {text:?})"
    );
}

#[test]
fn out_of_order_glyph_emission_sorts_visually() {
    // Content stream emits glyphs `x a o n s` — the first two are
    // swapped in emission order but `a` is positioned left of `x`. The
    // extractor's sort_by_reading_order must put them back into visual
    // order so we read "axons".
    //
    // Layout: all on baseline Y=800. X positions 100, 107, 114, 121, 128.
    // The stream draws `x` at x=107 FIRST, then `a` at x=100. A stable
    // reading-order sort puts `a` before `x` because x=100 < x=107.
    let content = "\
BT /F0 12 Tf 1 0 0 1 107 800 Tm (x) Tj ET\n\
BT /F0 12 Tf 1 0 0 1 100 800 Tm (a) Tj ET\n\
BT /F0 12 Tf 1 0 0 1 114 800 Tm (o) Tj ET\n\
BT /F0 12 Tf 1 0 0 1 121 800 Tm (n) Tj ET\n\
BT /F0 12 Tf 1 0 0 1 128 800 Tm (s) Tj ET\n";
    let pdf = helper_pdf(content);
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(
        compact, "axons",
        "reading-order sort failed to fix emission-order transposition. Got {compact:?} (raw: {text:?})"
    );
}
