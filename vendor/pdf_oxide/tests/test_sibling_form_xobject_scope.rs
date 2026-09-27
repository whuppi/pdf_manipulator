//! Regression test: path extraction must switch XObject resource scope when
//! descending into a Form XObject that carries its own /Resources.
//!
//! Previously, `process_form_xobject_paths` kept the parent page's cached
//! XObject name→ref map when it recursed into a Form. When the Form's own
//! content referenced a name (e.g. `/Im2 Do`) that existed in BOTH the Form's
//! local /Resources and the page's /Resources, the name resolved against the
//! parent scope by mistake. With N sibling forms whose local content uses
//! overlapping names, this turns the DAG into a cross-recursive graph:
//! O(N!) traversals and unbounded path accumulation — on a real LaTeX paper
//! (8 sibling forms each with ~1.9 KB of dense path ops) this manifests as a
//! hang with runaway memory growth on `extract_text` (extract_text runs table
//! detection, which runs the path extractor).
//!
//! The synthetic PDF here has 8 sibling Form XObjects, each carrying its own
//! /Resources with the same set of local names pointing at empty forms. With
//! correct scope handling, each form's Do ops resolve to its local dead-end
//! forms (O(N) total work). With broken scope handling they resolve to the
//! sibling parent-scope forms and cross-recurse — 8! = 40 320 visits, each
//! parsing a few hundred bytes of content, which takes long enough that a
//! one-second wall-clock guard reliably catches the regression.

use pdf_oxide::document::PdfDocument;
use std::time::{Duration, Instant};

const N_SIBLINGS: u32 = 8;
const N_OPS_PER_FORM: usize = 200;

fn build_sibling_form_collision_pdf() -> Vec<u8> {
    // Object layout:
    //   1  Catalog
    //   2  Pages
    //   3  Page
    //   4  Page content stream
    //   5..5+N-1   Parent-scope sibling Form XObjects
    //   5+N        Font
    //   5+N+1..    Local empty forms used as dead-ends inside siblings
    let first_form = 5u32;
    let font_obj = first_form + N_SIBLINGS;
    let first_local = font_obj + 1;

    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut off: Vec<usize> = Vec::new();

    let push = |pdf: &mut Vec<u8>, off: &mut Vec<usize>, body: &[u8]| {
        off.push(pdf.len());
        pdf.extend_from_slice(body);
    };

    push(&mut pdf, &mut off, b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    push(
        &mut pdf,
        &mut off,
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
    );

    // Build /XObject entries for the page: /Im1..ImN → sibling forms
    let page_xobjects: String = (0..N_SIBLINGS)
        .map(|i| format!("/Im{} {} 0 R ", i + 1, first_form + i))
        .collect();
    let page_obj = format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]\n\
         /Contents 4 0 R\n\
         /Resources << /Font << /F1 {} 0 R >> /XObject << {}>> >>\n\
         >>\nendobj\n",
        font_obj, page_xobjects,
    );
    push(&mut pdf, &mut off, page_obj.as_bytes());

    // Page content: paint all sibling forms and emit a marker text.
    let mut content = b"BT /F1 12 Tf 50 700 Td (marker) Tj ET\n".to_vec();
    for i in 0..N_SIBLINGS {
        content.extend_from_slice(format!("/Im{} Do\n", i + 1).as_bytes());
    }
    let stream = format!(
        "4 0 obj\n<< /Length {} >>\nstream\n{}endstream\nendobj\n",
        content.len(),
        std::str::from_utf8(&content).unwrap()
    );
    push(&mut pdf, &mut off, stream.as_bytes());

    // Sibling Form XObjects: each has local /Resources mapping /Im1../ImN to
    // distinct empty local forms, and a content stream that Do's every name.
    for i in 0..N_SIBLINGS {
        let id = first_form + i;
        // Local /XObject entries → empty forms dedicated to this sibling.
        let local_xobjects: String = (0..N_SIBLINGS)
            .map(|j| format!("/Im{} {} 0 R ", j + 1, first_local + i * N_SIBLINGS + j))
            .collect();

        // Content: dense path ops (to match real-world LaTeX Form XObjects) +
        // sibling-name Do calls.
        let mut form_content = Vec::new();
        for k in 0..N_OPS_PER_FORM {
            form_content
                .extend_from_slice(format!("{} {} m {} {} l S\n", k, k, k + 1, k + 1).as_bytes());
        }
        for j in 0..N_SIBLINGS {
            form_content.extend_from_slice(format!("/Im{} Do\n", j + 1).as_bytes());
        }

        let form = format!(
            "{} 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] /FormType 1\n\
             /Resources << /XObject << {}>> >>\n\
             /Length {} >>\nstream\n{}endstream\nendobj\n",
            id,
            local_xobjects,
            form_content.len(),
            std::str::from_utf8(&form_content).unwrap()
        );
        push(&mut pdf, &mut off, form.as_bytes());
    }

    // Font
    let font = format!(
        "{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
        font_obj
    );
    push(&mut pdf, &mut off, font.as_bytes());

    // Local empty forms.
    for i in 0..N_SIBLINGS {
        for j in 0..N_SIBLINGS {
            let id = first_local + i * N_SIBLINGS + j;
            let form = format!(
                "{} 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] /FormType 1 /Length 0 >>\nstream\n\nendstream\nendobj\n",
                id,
            );
            push(&mut pdf, &mut off, form.as_bytes());
        }
    }

    // xref
    let xref_pos = pdf.len();
    let count = off.len() + 1;
    pdf.extend_from_slice(format!("xref\n0 {}\n", count).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for o in &off {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", o).as_bytes());
    }

    pdf.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", count, xref_pos)
            .as_bytes(),
    );

    pdf
}

#[test]
fn sibling_form_xobject_name_collision_does_not_hang() {
    let pdf_bytes = build_sibling_form_collision_pdf();

    let doc = PdfDocument::from_bytes(pdf_bytes).expect("doc should open");
    assert_eq!(doc.page_count().unwrap(), 1);

    let start = Instant::now();
    let text = doc.extract_text(0).expect("extract_text should succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "extract_text took {:?} — sibling Form XObject scope regression",
        elapsed
    );
    assert!(text.contains("marker"));
}
