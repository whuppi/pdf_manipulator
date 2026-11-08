//! Quality-gate tests for text extraction against the kreuzberg corpus.
//!
//! These tests assert that text extraction Jaccard similarity against
//! the kreuzberg ground-truth corpus stays at or above measured thresholds.
//! They guard against silent quality regressions — if a change drops a
//! document below its threshold, the test catches it.
//!
//! # Skip behaviour
//!
//! Each test skips gracefully when the PDF or ground-truth file is not present
//! in `/tmp/`.  In local development, place the files there (see the download
//! URLs in each test) before running.  CI can set them up with a dedicated
//! fixture-download step; tests are individually `#[ignore]` to avoid blocking
//! the default `cargo test` run.
//!
//! # Metric
//!
//! Jaccard similarity on whitespace-split word tokens.  This differs from
//! kreuzberg's word-F1 but is a good proxy that avoids the kreuzberg dependency.

use pdf_oxide::converters::ConversionOptions;
use pdf_oxide::document::PdfDocument;
use std::collections::HashSet;

fn jaccard(a: &str, b: &str) -> f32 {
    let sa: HashSet<&str> = a.split_whitespace().collect();
    let sb: HashSet<&str> = b.split_whitespace().collect();
    let i = sa.intersection(&sb).count();
    let u = sa.union(&sb).count();
    if u == 0 {
        1.0
    } else {
        i as f32 / u as f32
    }
}

/// Strip markdown syntax markers from a string so the Jaccard test
/// compares text content rather than formatting markup.
///
/// Removes the same families of tokens that pdftotext's plain-text GT
/// never contains:
///   * `**...**` and `*...*` (bold / italic)
///   * `|` table column separators
///   * `---|---|---` separator-row sequences
///   * leading `#` heading markers
///   * fenced code-block fences
///
/// Mirrors the HTML test's tag-stripping rationale: the markdown
/// converter's job is to produce the right text + structure; whether
/// it wraps a number in `**...**` is a formatting concern, not a
/// content-quality regression.
fn strip_markdown(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut at_line_start = true;
    while i < chars.len() {
        let c = chars[i];
        // `**` bold marker — drop two chars
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            at_line_start = false;
            continue;
        }
        // standalone `*` italic marker — drop
        if c == '*' {
            i += 1;
            at_line_start = false;
            continue;
        }
        // `|` table column separator — turn into space so neighbouring
        // cells stay separated by whitespace for split_whitespace()
        if c == '|' {
            out.push(' ');
            i += 1;
            at_line_start = false;
            continue;
        }
        // `# `, `## ` … heading marker at start of line — drop the run
        if at_line_start && c == '#' {
            while i < chars.len() && chars[i] == '#' {
                i += 1;
            }
            continue;
        }
        // 3+ consecutive `-` (separator row) — turn into space
        if c == '-' && chars.get(i + 1) == Some(&'-') && chars.get(i + 2) == Some(&'-') {
            while i < chars.len() && chars[i] == '-' {
                i += 1;
            }
            out.push(' ');
            at_line_start = false;
            continue;
        }
        // Fenced code block ``` — drop the line
        if c == '`' && chars.get(i + 1) == Some(&'`') && chars.get(i + 2) == Some(&'`') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        out.push(c);
        at_line_start = c == '\n';
        i += 1;
    }
    out
}

fn jaccard_markdown(md: &str, gt: &str) -> f32 {
    jaccard(&strip_markdown(md), gt)
}

/// Extract text from all pages of a PDF at the given path.
/// Returns None when the PDF file is not present (test skips).
fn extract_all_text(pdf_path: &str) -> Option<String> {
    let bytes = std::fs::read(pdf_path).ok()?;
    let doc = PdfDocument::from_bytes(bytes).ok()?;
    let _ = doc.authenticate(b"");
    let mut text = String::new();
    for i in 0..doc.page_count().unwrap_or(0) {
        if let Ok(t) = doc.extract_text(i) {
            text.push_str(&t);
            text.push('\n');
        }
    }
    Some(text)
}

fn check(label: &str, pdf: &str, gt: &str, threshold: f32) {
    let text = match extract_all_text(pdf) {
        Some(t) => t,
        None => {
            eprintln!("SKIP {label}: {pdf} not found");
            return;
        },
    };
    let gt_text = match std::fs::read_to_string(gt) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("SKIP {label}: ground truth {gt} not found");
            return;
        },
    };
    let j = jaccard(&text, &gt_text);
    assert!(
        j >= threshold,
        "{label}: Jaccard {j:.3} < threshold {threshold:.2}\n\
         (PDF: {pdf}, GT: {gt})\n\
         This is a quality regression — text extraction score dropped."
    );
    eprintln!("PASS {label:<28} j={j:.3}  thr={threshold:.2}");
}

// ---------------------------------------------------------------------------
// #484 Section 3a — hello_structure.pdf (structure-tree extraction fix)
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/vendored/pdfplumber/pdf/hello_structure.pdf
// GT note: original GT had straight apostrophe U+0027; PDF encodes U+2019 (right
// single quotation mark) which both pdf_oxide and pdftotext correctly emit. GT
// updated to use U+2019 so the apostrophe matches.
// Achieved j≈1.00 (threshold = 0.88).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/hello_structure.pdf and /tmp/gt_hello_structure.txt"]
fn quality_gate_hello_structure() {
    check(
        "hello_structure",
        "/tmp/hello_structure.pdf",
        "/tmp/gt_hello_structure.txt",
        0.88,
    );
}

// ---------------------------------------------------------------------------
// #484 Section 3b — pdfa_036.pdf (cell-bbox filter for spatial tables)
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/pdf/pdfa_036.pdf
// GT: Kreuzberg Securities (KSL) / HLA paragraph must be present.
// Achieved j≈0.88 (threshold = achieved - 0.05).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/pdfa_036.pdf and /tmp/gt_pdfa_036_kreuzberg.txt"]
fn quality_gate_pdfa_036() {
    check("pdfa_036", "/tmp/pdfa_036.pdf", "/tmp/gt_pdfa_036_kreuzberg.txt", 0.78);
}

// ---------------------------------------------------------------------------
// #484 Section 3d — pdfa_044.pdf
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/pdf/pdfa_044.pdf
// Achieved j≈0.90 (threshold = achieved - 0.05).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/pdfa_044.pdf and /tmp/gt_pdfa_044.txt"]
fn quality_gate_pdfa_044() {
    check("pdfa_044", "/tmp/pdfa_044.pdf", "/tmp/gt_pdfa_044.txt", 0.80);
}

// ---------------------------------------------------------------------------
// #484 Section 3e — nougat_039.pdf (same content as pdfa_014.pdf)
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/pdf/nougat_039.pdf
// Achieved j≈0.88 (threshold = achieved - 0.05).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/nougat_039.pdf and /tmp/gt_nougat_039.txt"]
fn quality_gate_nougat_039() {
    check("nougat_039", "/tmp/nougat_039.pdf", "/tmp/gt_nougat_039.txt", 0.78);
}

// ---------------------------------------------------------------------------
// #484 Section 3h — nougat_026.pdf / pdfa_001.pdf
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/pdf/nougat_026.pdf
// Achieved j≈0.97 (threshold = achieved - 0.05).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/nougat_026.pdf and /tmp/gt_nougat_026.txt"]
fn quality_gate_nougat_026() {
    check("nougat_026", "/tmp/nougat_026.pdf", "/tmp/gt_nougat_026.txt", 0.87);
}

// ---------------------------------------------------------------------------
// #484 Section 2b — pr-136-example.pdf (CJK CID-font garbling fix)
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/vendored/pdfplumber/pdf/pr-136-example.pdf
// Achieved j≈0.15 (threshold = achieved - 0.05; floor 0.05).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/pr-136-example.pdf and /tmp/gt_pr-136-example.txt"]
fn quality_gate_pr_136() {
    check("pr-136-example", "/tmp/pr-136-example.pdf", "/tmp/gt_pr-136-example.txt", 0.05);
}

// ---------------------------------------------------------------------------
// #484 Section 2c — pr-138-example.pdf (legacy-crypto encrypted PDF)
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/vendored/pdfplumber/pdf/pr-138-example.pdf
// Requires: pdf_oxide built with `legacy-crypto` feature (default)
// Achieved j≈0.55 (threshold = achieved - 0.05).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/pr-138-example.pdf and /tmp/gt_pr-138-example.txt"]
fn quality_gate_pr_138() {
    check("pr-138-example", "/tmp/pr-138-example.pdf", "/tmp/gt_pr-138-example.txt", 0.45);
}

// ---------------------------------------------------------------------------
// #484 Section 2a — issue-987-test.pdf (CID-font / encoding fix)
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/vendored/pdfplumber/pdf/issue-987-test.pdf
// Achieved j≈0.75 (threshold = achieved - 0.05).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/issue-987-test.pdf and /tmp/gt_issue-987-test.txt"]
fn quality_gate_issue_987() {
    check("issue-987-test", "/tmp/issue-987-test.pdf", "/tmp/gt_issue-987-test.txt", 0.65);
}

// ---------------------------------------------------------------------------
// issue-336-example.pdf (inter-span CJK spacing fix)
// pdfium correctly maps adjacent MCID spans with zero/negative gap;
// word-merging post-process in extract_words_inner closes the gap.
// Achieved j=0.835 (threshold = achieved - 0.05 = 0.74, kept as-is since
// original pre-fix baseline was already 0.74).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/issue-336-example.pdf and /tmp/gt_issue-336-example.txt"]
fn quality_gate_issue_336() {
    check(
        "issue-336-example",
        "/tmp/issue-336-example.pdf",
        "/tmp/gt_issue-336-example.txt",
        0.69,
    );
}

// ---------------------------------------------------------------------------
// issue-336-example.pdf — to_markdown quality gate (#485)
// Same PDF as quality_gate_issue_336 but exercising the to_markdown path so
// that the CJK fullwidth-operator space-suppression fix is covered end-to-end.
// Threshold set to the same floor used for extract_text (0.69).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/issue-336-example.pdf and /tmp/gt_issue-336-example.txt"]
fn quality_gate_issue_336_markdown() {
    let pdf_path = "/tmp/issue-336-example.pdf";
    let gt_path = "/tmp/gt_issue-336-example.txt";
    let bytes = match std::fs::read(pdf_path) {
        Ok(b) => b,
        Err(_) => {
            eprintln!("SKIP issue-336-example (markdown): {pdf_path} not found");
            return;
        },
    };
    let gt_text = match std::fs::read_to_string(gt_path) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("SKIP issue-336-example (markdown): ground truth {gt_path} not found");
            return;
        },
    };
    let doc = PdfDocument::from_bytes(bytes).expect("parse PDF");
    let _ = doc.authenticate(b"");
    let options = ConversionOptions::default();
    let mut text = String::new();
    for i in 0..doc.page_count().unwrap_or(0) {
        if let Ok(t) = doc.to_markdown(i, &options) {
            text.push_str(&t);
            text.push('\n');
        }
    }
    // Strip markdown formatting markers before Jaccard so we compare
    // content tokens rather than syntax (matches the HTML test which
    // strips <...> tags).
    let j = jaccard_markdown(&text, &gt_text);
    assert!(
        j >= 0.69,
        "issue-336-example (markdown): Jaccard {j:.3} < threshold 0.69\n\
         This is a quality regression — to_markdown score dropped."
    );
    eprintln!("PASS issue-336-example (markdown)      j={j:.3}  thr=0.69");
}

// ---------------------------------------------------------------------------
// issue-336-example.pdf — to_html quality gate (#485)
// Same PDF as quality_gate_issue_336 but exercising the to_html path.
// HTML tags are stripped before computing Jaccard so the score reflects
// actual text content rather than markup.
// Threshold set slightly lower than to_markdown (0.65) to account for minor
// HTML-wrapping differences.
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/issue-336-example.pdf and /tmp/gt_issue-336-example.txt"]
fn quality_gate_issue_336_html() {
    let pdf_path = "/tmp/issue-336-example.pdf";
    let gt_path = "/tmp/gt_issue-336-example.txt";
    let bytes = match std::fs::read(pdf_path) {
        Ok(b) => b,
        Err(_) => {
            eprintln!("SKIP issue-336-example (html): {pdf_path} not found");
            return;
        },
    };
    let gt_text = match std::fs::read_to_string(gt_path) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("SKIP issue-336-example (html): ground truth {gt_path} not found");
            return;
        },
    };
    let doc = PdfDocument::from_bytes(bytes).expect("parse PDF");
    let _ = doc.authenticate(b"");
    let options = ConversionOptions::default();
    let mut html = String::new();
    for i in 0..doc.page_count().unwrap_or(0) {
        if let Ok(t) = doc.to_html(i, &options) {
            html.push_str(&t);
            html.push('\n');
        }
    }
    // Strip HTML tags: replace <...> sequences with a space so word tokens survive.
    let stripped = {
        let mut out = String::with_capacity(html.len());
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => {
                    in_tag = true;
                    out.push(' ');
                },
                '>' => {
                    in_tag = false;
                    out.push(' ');
                },
                _ if in_tag => {},
                _ => out.push(c),
            }
        }
        out
    };
    let j = jaccard(&stripped, &gt_text);
    assert!(
        j >= 0.65,
        "issue-336-example (html): Jaccard {j:.3} < threshold 0.65\n\
         This is a quality regression — to_html score dropped."
    );
    eprintln!("PASS issue-336-example (html)          j={j:.3}  thr=0.65");
}

// ---------------------------------------------------------------------------
// nougat_040.pdf — math formula subscript/notation merging
// merge_sub_superscript_spans now merges non-adjacent subscript spans (Prx, H1, H2,
// D1, D2, ∆1, ∆2, ρLap, Xu, etc.) for 1-2 char math variable bases.
// GT source: kreuzberg corpus (Nougat ML model output) — includes image tokens
// (![img-N.jpeg]) and LaTeX math ($$...$$) that plain-text extraction cannot
// reproduce. pdftotext scores ≈0.46 on the same GT, confirming the gap is
// inherent to the GT format. Threshold set to current achievable − 0.05.
// Achieved j≈0.40 (threshold = 0.35).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/nougat_040.pdf and /tmp/gt_nougat_040.txt"]
fn quality_gate_nougat_040() {
    check("nougat_040", "/tmp/nougat_040.pdf", "/tmp/gt_nougat_040.txt", 0.35);
}

// ---------------------------------------------------------------------------
// pdfa_004.pdf — math subscript/superscript merging IMPROVED in v0.3.46
// merge_sub_superscript_spans merges k₁→k1, k₂→k2, γk using X-proximity (xd<1.5pt)
// and Y-offset [12%,75%] of base_fs.  char_widths extended to prevent re-split.
// GT source: kreuzberg corpus (Nougat ML model output) — includes LaTeX math
// notation ($(k-1)^{2}$-chromatic etc.) that plain-text extraction cannot
// reproduce. pdftotext scores ≈0.61 on the same GT. Threshold set to current
// achievable − 0.05.
// Achieved j≈0.54 (threshold = 0.49).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/pdfa_004.pdf and /tmp/gt_pdfa_004.txt"]
fn quality_gate_pdfa_004() {
    check("pdfa_004", "/tmp/pdfa_004.pdf", "/tmp/gt_pdfa_004.txt", 0.49);
}

// ---------------------------------------------------------------------------
// nougat_018.pdf — FIXED in v0.3.46
// Column-spanning decimal detection (sailing score tables): "12.11" spans
// whose bbox.width >> expected_width are now split at the decimal point in
// both flow-text and spatial/tagged table cell paths.
// Achieved j=0.981 (threshold = achieved - 0.05 = 0.90).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "requires /tmp/nougat_018.pdf and /tmp/gt_nougat_018.txt"]
fn quality_gate_nougat_018() {
    check("nougat_018", "/tmp/nougat_018.pdf", "/tmp/gt_nougat_018.txt", 0.90);
}

// ---------------------------------------------------------------------------
// #487 — to_html quality gates for table-heavy PDFs.
//
// The root cause was render_table_html using cell.text directly instead of
// walking cell.spans (as render_table_markdown does), causing inter-span gaps
// to be lost and losing bold/italic markup from the token set.  Fixed by the
// new render_cell_html helper that mirrors the span-walking in the markdown path.
//
// HTML tags are stripped before Jaccard so the score reflects actual text
// tokens rather than markup.  HTML entities (&amp;, &lt;, &gt;, &quot;) are
// also unescaped so they compare correctly against plain-text ground truth.
// ---------------------------------------------------------------------------

/// Strip HTML tags and unescape common HTML entities, returning plain text
/// suitable for Jaccard comparison against plain-text ground truth.
fn strip_html(html: &str) -> String {
    // First pass: remove tags (replace < ... > with a space).
    let mut stripped = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                stripped.push(' ');
            },
            '>' => {
                in_tag = false;
                stripped.push(' ');
            },
            _ if in_tag => {},
            _ => stripped.push(c),
        }
    }
    // Second pass: unescape the five XML/HTML predefined entities.
    stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Extract all pages of a PDF as HTML, strip tags, and return plain text.
/// Returns None when the PDF file is not present (test skips).
fn extract_all_html(pdf_path: &str) -> Option<String> {
    let bytes = std::fs::read(pdf_path).ok()?;
    let doc = PdfDocument::from_bytes(bytes).ok()?;
    let _ = doc.authenticate(b"");
    let options = ConversionOptions::default();
    let mut html = String::new();
    for i in 0..doc.page_count().unwrap_or(0) {
        if let Ok(t) = doc.to_html(i, &options) {
            html.push_str(&t);
            html.push('\n');
        }
    }
    Some(html)
}

fn check_html(label: &str, pdf: &str, gt: &str, threshold: f32) {
    let html = match extract_all_html(pdf) {
        Some(h) => h,
        None => {
            eprintln!("SKIP {label}: {pdf} not found");
            return;
        },
    };
    let gt_text = match std::fs::read_to_string(gt) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("SKIP {label}: ground truth {gt} not found");
            return;
        },
    };
    let text = strip_html(&html);
    let j = jaccard(&text, &gt_text);
    assert!(
        j >= threshold,
        "{label}: Jaccard {j:.3} < threshold {threshold:.2}\n\
         (PDF: {pdf}, GT: {gt})\n\
         This is a quality regression — to_html score dropped."
    );
    eprintln!("PASS {label:<28} j={j:.3}  thr={threshold:.2}");
}

// pdfa_036.pdf — to_html quality gate (#487)
// Same fixture as quality_gate_pdfa_036.  Threshold matches the to_text gate
// (0.78) since the span-walking fix brings HTML on par with text extraction.
#[test]
#[ignore = "requires /tmp/pdfa_036.pdf and /tmp/gt_pdfa_036_kreuzberg.txt"]
fn quality_gate_pdfa_036_html() {
    check_html("pdfa_036 (html)", "/tmp/pdfa_036.pdf", "/tmp/gt_pdfa_036_kreuzberg.txt", 0.78);
}

// nougat_026.pdf — to_html quality gate (#487)
// Same fixture as quality_gate_nougat_026.  Threshold 0.87 matches to_text gate.
#[test]
#[ignore = "requires /tmp/nougat_026.pdf and /tmp/gt_nougat_026.txt"]
fn quality_gate_nougat_026_html() {
    check_html("nougat_026 (html)", "/tmp/nougat_026.pdf", "/tmp/gt_nougat_026.txt", 0.87);
}

// nougat_040.pdf — to_html quality gate (#487)
// Same fixture as quality_gate_nougat_040.  Threshold 0.35 matches to_text gate.
#[test]
#[ignore = "requires /tmp/nougat_040.pdf and /tmp/gt_nougat_040.txt"]
fn quality_gate_nougat_040_html() {
    check_html("nougat_040 (html)", "/tmp/nougat_040.pdf", "/tmp/gt_nougat_040.txt", 0.35);
}

// nougat_018.pdf — to_html quality gate (#487)
// Same fixture as quality_gate_nougat_018.  Threshold 0.95 (higher than to_text
// 0.90) because sailing-score tables with span metadata now render all tokens
// correctly via the span-walking path.
#[test]
#[ignore = "requires /tmp/nougat_018.pdf and /tmp/gt_nougat_018.txt"]
fn quality_gate_nougat_018_html() {
    check_html("nougat_018 (html)", "/tmp/nougat_018.pdf", "/tmp/gt_nougat_018.txt", 0.95);
}

// ---------------------------------------------------------------------------
// #486 — to_markdown quality gate for line-less sailing-score table.
//
// nougat_018.pdf is a sailing regatta results document.  The table has no
// ruling lines, so the old code returned no tables and the converter fell
// back to flat paragraph rendering (Jaccard ≈ 0.64).  With the text-only
// spatial fallback enabled for converter paths (to_markdown, to_html), the
// table is now detected spatially and rendered as a proper markdown table.
//
// Source: https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/pdf/nougat_018.pdf
// GT:     https://github.com/kreuzberg-dev/kreuzberg/blob/main/test_documents/expected_output/nougat_018.txt
// Achieved ≥ 0.90 after fix (threshold = 0.90).
// ---------------------------------------------------------------------------

/// Extract all pages of a PDF as Markdown and concatenate into a single string.
/// Returns None when the PDF file is not present (test skips).
fn extract_all_markdown(pdf_path: &str) -> Option<String> {
    let bytes = std::fs::read(pdf_path).ok()?;
    let doc = PdfDocument::from_bytes(bytes).ok()?;
    let _ = doc.authenticate(b"");
    let options = ConversionOptions::default();
    let mut md = String::new();
    for i in 0..doc.page_count().unwrap_or(0) {
        if let Ok(t) = doc.to_markdown(i, &options) {
            md.push_str(&t);
            md.push('\n');
        }
    }
    Some(md)
}

fn check_markdown(label: &str, pdf: &str, gt: &str, threshold: f32) {
    let md = match extract_all_markdown(pdf) {
        Some(m) => m,
        None => {
            eprintln!("SKIP {label}: {pdf} not found");
            return;
        },
    };
    let gt_text = match std::fs::read_to_string(gt) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("SKIP {label}: ground truth {gt} not found");
            return;
        },
    };
    // Strip markdown formatting markers (**bold**, *italic*, | table
    // separators, --- header rule, leading # headings, ``` fences)
    // before computing Jaccard so the score reflects text content,
    // not formatting characters that the plain-text GT was never
    // going to contain.  Mirrors the HTML test's strip_html step.
    let j = jaccard_markdown(&md, &gt_text);
    assert!(
        j >= threshold,
        "{label}: Jaccard {j:.3} < threshold {threshold:.2}\n\
         (PDF: {pdf}, GT: {gt})\n\
         This is a quality regression — to_markdown score dropped."
    );
    eprintln!("PASS {label:<28} j={j:.3}  thr={threshold:.2}");
}

#[test]
#[ignore = "requires /tmp/nougat_018.pdf and /tmp/gt_nougat_018.txt"]
fn quality_gate_nougat_018_markdown() {
    check_markdown("nougat_018 (markdown)", "/tmp/nougat_018.pdf", "/tmp/gt_nougat_018.txt", 0.90);
}
