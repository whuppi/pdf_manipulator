//! B8b / #365 — synthetic minimal reproducer for in-TJ-array kerning.
//!
//! Status: **ignored** — the synthetic single-TJ-array case
//! `[(diffe) -150 (rent)] TJ` cannot be reliably distinguished from a
//! tightly-justified real-word boundary at the TJ-side level without
//! introducing catastrophic false-positives on real-world LaTeX/Docling
//! corpora. An earlier TJ-side guard (commit b2c6484) suppressed this
//! case using a `letter-letter + |offset| < space-glyph-width` rule but
//! glued real word boundaries on academic papers (e.g.
//! `Bayesianhierarchicalmodels`, `JournalofEconometrics`,
//! `ABayesianCoursewithExamplesinR`). It was reverted.
//!
//! The real-world manifestation of #365 (split-words like "diffe rent",
//! "cha nge", "operation al" on the Kreuzberg corpus) IS handled by the
//! span-merge-time intra-word kerning guard in `should_insert_space`,
//! which has access to full bbox and `WordBoundaryDetector` context.
//! See `tests/` PDFs and the Kreuzberg corpus validation in commit
//! c916c8c for that path.
//!
//! This synthetic test remains as a documented limitation reproducer.
//! Re-enable only if a tighter discriminator is found that handles both
//! the synthetic case AND tightly-justified PDFs.
use pdf_oxide::PdfDocument;

/// Build a 1-page PDF with a single content stream whose TJ array embeds
/// 150-thousandths-of-em inter-glyph kerning between the halves of
/// `different`, `change`, and `equivalent`. The kerning magnitude is
/// chosen to land just above the 0.5-space-width threshold for
/// 12-pt Helvetica so that the current heuristic will split the words
/// and the fix can close the gap.
fn kerning_pdf() -> Vec<u8> {
    // Content stream. `0.15 em` kerning at 12 pt Helvetica ≈ 1.8 pt gap,
    // which exceeds the default 1.5-pt 0.5-space-width threshold.
    //
    // TJ array layout: [(left-half) NEG-OFFSET (right-half)] TJ
    // The NEG-OFFSET is in units of 1/1000 em (positive offset moves
    // cursor *backwards*, i.e. glyph pairs tighter; negative offset moves
    // cursor *forwards*, opening a gap). We want a forward gap: -150.
    //
    // The three phrases are on separate text lines so the test can pin
    // each failure mode independently.
    let content = "\
BT /F0 12 Tf 1 0 0 1 100 800 Tm [(diffe) -150 (rent)] TJ ET\n\
BT /F0 12 Tf 1 0 0 1 100 780 Tm [(cha) -150 (nge)] TJ ET\n\
BT /F0 12 Tf 1 0 0 1 100 760 Tm [(equivalen) -150 (t)] TJ ET\n";

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
#[ignore = "synthetic single-TJ-array case — see module docs; real Kreuzberg #365 \
            cases are covered by should_insert_space guard"]
fn tj_kerning_within_word_does_not_insert_space() {
    let pdf = kerning_pdf();
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");

    for (halves, joined) in [
        (("diffe", "rent"), "different"),
        (("cha", "nge"), "change"),
        (("equivalen", "t"), "equivalent"),
    ] {
        // Both halves must be present (otherwise the test is broken)...
        assert!(text.contains(halves.0), "missing left half '{}'. Text: {text:?}", halves.0);
        assert!(text.contains(halves.1), "missing right half '{}'. Text: {text:?}", halves.1);

        // ...and they must be joined, not split by a spurious space.
        let split = format!("{} {}", halves.0, halves.1);
        assert!(
            !text.contains(&split),
            "#365: TJ intra-word kerning produced spurious space: '{split}' should be '{joined}'. Text: {text:?}"
        );
        assert!(
            text.contains(joined),
            "#365: expected '{joined}' in extracted text. Text: {text:?}"
        );
    }
}
