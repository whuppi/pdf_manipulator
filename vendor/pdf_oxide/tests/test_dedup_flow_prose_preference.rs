//! #318 — when deduplicating two overlapping copies of the same content,
//! prefer the flow-prose rendering over the wide-spaced positioned one.
//!
//! JSTOR-sourced textbooks (`pdfs_slow9/[Vaclav-Smil]-Energy-and-Civilization…`)
//! frequently emit two copies of the same paragraph in one content stream:
//!
//!   1. A **flow-prose** copy where every line is a single `Tj` with normal
//!      word spacing inside a literal string.
//!   2. A **positioned** copy where each word is a separate `Tj` at an
//!      explicit x-coordinate, producing runs of 4–20 spaces between words
//!      when emitted as plain text.
//!
//! v0.3.23 emitted both copies (the output had each page twice). v0.3.25's
//! #315/#316 reading-order fixes correctly dedup them but consistently
//! keep the positioned copy and drop the clean flow-prose one — producing
//! unreadable wide-spaced output even though all content is preserved.
//!
//! This test synthesises the pattern: two `BT/ET` blocks at the same Y
//! containing the same words, the second of which uses per-word `Tj`s
//! separated by large `Td` moves. The extractor must keep the prose
//! rendering, not the positioned one.
//!
//! Current status (v0.3.33): the synthetic assertion passes — our
//! extractor does not keep the wide-spaced positioned copy in
//! preference to the flow-prose copy. Validated against the
//! `[Vaclav-Smil]-Energy-and-Civilization` fixture from the issue:
//! "In 1894 a new Daimler-Maybach gasoline engine …" comes through as
//! clean flow prose without the run-of-many-spaces rendering or a
//! duplicate wide-spaced copy. The cumulative effect of B3 (running
//! headers), B4 (XY-cut), B7 (stroke+fill overlap), and content-based
//! dedup keeps this case on the happy path.
use pdf_oxide::PdfDocument;

fn two_copy_pdf() -> Vec<u8> {
    // Copy 1 (flow prose): single Tj with literal spaces between words.
    //   "In 1894 a new Daimler-Maybach gasoline engine installed"
    //
    // Copy 2 (positioned): per-word Tj with explicit Td moves that
    // produce the wide-spaced layout. Both copies draw on the same
    // baseline Y=800 in the same page's MediaBox.
    //
    // The content stream emits copy 1 FIRST and copy 2 SECOND, matching
    // the JSTOR producer ordering reported in the issue.
    let content = "\
BT /F0 12 Tf 1 0 0 1 50 800 Tm (In 1894 a new Daimler-Maybach gasoline engine installed) Tj ET\n\
BT /F0 12 Tf 1 0 0 1 50 800 Tm (In) Tj \
50 0 Td (1894) Tj \
50 0 Td (a) Tj \
50 0 Td (new) Tj \
50 0 Td (Daimler-Maybach) Tj \
60 0 Td (gasoline) Tj \
60 0 Td (engine) Tj \
60 0 Td (installed) Tj ET\n";

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
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 700 900] \
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
fn dedup_prefers_flow_prose_copy_over_positioned_copy() {
    let pdf = two_copy_pdf();
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");

    // Baseline sanity: the extracted output must contain every word in
    // the paragraph — whichever copy dedup keeps, no content should be
    // lost.
    for word in [
        "In",
        "1894",
        "Daimler-Maybach",
        "gasoline",
        "engine",
        "installed",
    ] {
        assert!(text.contains(word), "missing word {word:?}. Text: {text:?}");
    }

    // The flow-prose copy has one-space-per-word separators. The
    // positioned copy produces runs of 4+ spaces between words. The
    // extractor must prefer the tight rendering: reject any run of 4+
    // consecutive spaces on the extracted line.
    assert!(
        !text.contains("    "),
        "#318: output contains a 4+ space run, which means the positioned \
         (wide-spaced) copy was kept instead of the flow-prose copy. Text: {text:?}"
    );
}
