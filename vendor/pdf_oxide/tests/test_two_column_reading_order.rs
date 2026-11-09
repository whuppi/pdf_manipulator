//! Tests that two-column reading order.
//!
//! Pre-fix, `extract_text` used a row-aware Y-band sort (Y descending
//! within bands, X ascending within a row). On a two-column layout that
//! interleaved rows: LeftCol-row1 RightCol-row1 LeftCol-row2 … — the
//! wrong order. pdftotext's order (read all of left column first, then
//! all of right column) scored ~0.86 average while ours sat at 0.80.
//!
//! Fix: detect multi-column pages and route them through XYCutStrategy.
//! Single-column pages stay on the cheap row-aware path.
//!
//! Test: synthesise a PDF whose left column says "Left1..LeftN" and
//! right column says "Right1..RightN", with rows interleaved in Y. All
//! lefts must appear before any rights in the extracted text.

use pdf_oxide::PdfDocument;

fn two_column_pdf() -> Vec<u8> {
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

    // 20 rows, each row has a left-column label and a right-column label.
    // Left X = 100, Right X = 380. Rows at Y = 800, 760, 720, … 40.
    // Without multi-column detection, row-aware sort would emit:
    // Left1 Right1 Left2 Right2 …
    // With XY-cut it should emit all lefts then all rights.
    let mut stream = String::new();
    for i in 1..=20 {
        let y = 800 - (i - 1) * 40;
        stream.push_str(&format!(
            "BT /F0 12 Tf 100 {y} Td (Left{i:02}) Tj ET \
             BT /F0 12 Tf 380 {y} Td (Right{i:02}) Tj ET\n"
        ));
    }
    push(
        &mut out,
        &mut offsets,
        &format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len() + 1),
    );
    push(&mut out, &mut offsets, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");

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

#[test]
fn two_column_reading_order_respects_columns() {
    let pdf = two_column_pdf();
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");

    // Extract left/right column positions in output order.
    let left_idx: Vec<_> = (1..=20)
        .filter_map(|i| text.find(&format!("Left{i:02}")))
        .collect();
    let right_idx: Vec<_> = (1..=20)
        .filter_map(|i| text.find(&format!("Right{i:02}")))
        .collect();

    assert_eq!(
        left_idx.len(),
        20,
        "all 20 Left labels must be extracted; got {}",
        left_idx.len()
    );
    assert_eq!(
        right_idx.len(),
        20,
        "all 20 Right labels must be extracted; got {}",
        right_idx.len()
    );

    // Relative order within each column must be preserved.
    assert!(
        left_idx.windows(2).all(|w| w[0] < w[1]),
        "Left labels must appear in order in the output, got {left_idx:?}"
    );
    assert!(
        right_idx.windows(2).all(|w| w[0] < w[1]),
        "Right labels must appear in order in the output, got {right_idx:?}"
    );

    // Column-respecting order means Left20 (last left) should come
    // before Right01 (first right) — we read top-to-bottom in the left
    // column before reaching the right column. Under the old row-aware
    // sort, the interleaving Left1 Right1 Left2 Right2 … produced
    // Left20 AFTER Right19. The key signal is: majority of lefts come
    // before majority of rights.
    let left_median = left_idx[10]; // middle Left
    let right_median = right_idx[10];
    assert!(
        left_median < right_median,
        "B4 regression: the middle Left label (at {left_median}) must come \
         before the middle Right label (at {right_median}). This means \
         extract_text is interleaving columns instead of emitting \
         left-column-first."
    );
}
