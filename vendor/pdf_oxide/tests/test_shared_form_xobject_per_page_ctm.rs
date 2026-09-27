//! Tests that Form XObject reuse across pages with per-page CTM.
//!
//! Before the fix, `extract_text(n)` returned page 0's content for every
//! `n` on PDFs where a single Form XObject carried every page's text and
//! each page's content stream applied its own CTM translation to clip
//! into the XObject. The bug had two causes:
//!
//! 1. `xobject_spans_cache` stored CTM-transformed page coordinates and
//!    was reused across pages with different CTMs — so page N retrieved
//!    page 0's coordinates.
//! 2. Even with the cache disabled, spans were emitted at different Y
//!    offsets per page (correct) but never filtered to the page's
//!    MediaBox — so every page returned every page's text.
//!
//! Fix: cache only when CTM is identity, and post-filter spans by the
//! page's MediaBox bounds.
//!
//! This test synthesises a two-page PDF that uses the pattern
//! (ExpertPdf-style): one Form XObject containing text spans in two
//! distinct Y regions, and two pages that each translate into one
//! region via `cm`.

use pdf_oxide::PdfDocument;

/// Build a minimal 2-page PDF where both pages invoke the same Form
/// XObject but with different CTM translations to render distinct text.
///
/// XObject coord system: `/Top Page` at Y=800, `/Bottom Page` at Y=100.
/// Page 1 uses CTM that leaves both Y values on-page (shows both, but we
/// only keep the one matching the MediaBox). Page 2 translates by -700
/// so the "Bottom Page" label moves to Y=-600 (off page).
fn minimal_shared_xobject_pdf() -> Vec<u8> {
    // Minimal PDF is easier to build as bytes directly than via a crate.
    // Objects (all generation 0):
    //   1  Catalog
    //   2  Pages (Kids = [3, 4])
    //   3  Page 1 (MediaBox [0 0 600 900], Contents 5, Resources -> /Font F0, /XObject X0)
    //   4  Page 2 (MediaBox [0 0 600 900], Contents 6, Resources -> F0, X0)
    //   5  Page 1 content stream (invokes X0 at identity CTM)
    //   6  Page 2 content stream (invokes X0 at CTM translated Y -700)
    //   7  Font F0 (Type1 Helvetica)
    //   8  Form XObject X0 with two TJ calls:
    //         BT /F0 24 Tf 100 800 Td (Top Page) Tj ET
    //         BT /F0 24 Tf 100 100 Td (Bottom Page) Tj ET
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = vec![0]; // obj 0 is reserved

    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let push = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &str| {
        offsets.push(out.len());
        let id = offsets.len() - 1;
        out.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    };

    push(&mut out, &mut offsets, "<< /Type /Catalog /Pages 2 0 R >>");
    push(&mut out, &mut offsets, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>");
    let page_common = "/Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] \
                       /Resources << /Font << /F0 7 0 R >> /XObject << /X0 8 0 R >> >>";
    push(&mut out, &mut offsets, &format!("<< {page_common} /Contents 5 0 R >>"));
    push(&mut out, &mut offsets, &format!("<< {page_common} /Contents 6 0 R >>"));

    // Page 1 content stream: invoke X0 at identity CTM — both labels render.
    let page1 = "q /X0 Do Q\n";
    push(
        &mut out,
        &mut offsets,
        &format!("<< /Length {} >>\nstream\n{page1}\nendstream", page1.len() + 1),
    );

    // Page 2 content stream: translate by (0, -700) before invoking X0.
    // `Top Page` at XObject Y=800 lands at page Y=100 (on page).
    // `Bottom Page` at XObject Y=100 lands at page Y=-600 (off page,
    // must be filtered).
    let page2 = "q 1 0 0 1 0 -700 cm /X0 Do Q\n";
    push(
        &mut out,
        &mut offsets,
        &format!("<< /Length {} >>\nstream\n{page2}\nendstream", page2.len() + 1),
    );

    push(&mut out, &mut offsets, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");

    // Form XObject: text operators draw two labels in distinct Y bands.
    let xo = b"q BT /F0 24 Tf 100 800 Td (Top Page) Tj ET \
                 BT /F0 24 Tf 100 100 Td (Bottom Page) Tj ET Q\n";
    push(
        &mut out,
        &mut offsets,
        &format!(
            "<< /Type /XObject /Subtype /Form /BBox [0 0 600 900] \
               /Resources << /Font << /F0 7 0 R >> >> /Length {} >>\nstream\n{}\nendstream",
            xo.len(),
            std::str::from_utf8(xo).unwrap()
        ),
    );

    // xref table
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
fn shared_xobject_with_per_page_ctm_yields_distinct_page_text() {
    let pdf = minimal_shared_xobject_pdf();
    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &pdf).unwrap();

    let doc = PdfDocument::open(tmp.path()).expect("open");
    assert_eq!(doc.page_count().unwrap(), 2, "fixture has 2 pages");

    let p0 = doc.extract_text(0).expect("page 0");
    let p1 = doc.extract_text(1).expect("page 1");

    // Page 0 applies identity CTM — both labels are within MediaBox
    // [0 0 600 900], so both stay.
    assert!(p0.contains("Top Page"), "page 0 should contain 'Top Page', got {p0:?}");
    assert!(p0.contains("Bottom Page"), "page 0 should contain 'Bottom Page', got {p0:?}");

    // Page 1 translates by -700. `Top Page` at Y_obj=800 lands at page
    // Y=100 (on page). `Bottom Page` at Y_obj=100 lands at page Y=-600
    // (off page — must be filtered by MediaBox clip).
    assert!(p1.contains("Top Page"), "page 1 should contain 'Top Page', got {p1:?}");
    assert!(
        !p1.contains("Bottom Page"),
        "page 1 must NOT contain 'Bottom Page' (off-page, MediaBox filter should drop it); got {p1:?}"
    );

    // The two pages' text must not be identical — the whole bug report.
    assert_ne!(p0, p1, "page 0 and page 1 must yield different text ");
}
