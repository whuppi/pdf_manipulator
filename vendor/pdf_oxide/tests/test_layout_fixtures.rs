//! Layout regression fixtures.
//!
//! Hand-crafted PDFs that pin down each text-layout invariant the
//! reading-order pipeline must preserve: single-column flow, two-column
//! reading order, narrow-gutter detection, mixed-size column handling,
//! multi-column hyphenation rejoin, and resilience against
//! scattered-fragment layouts that previously routed through XY-cut
//! and produced reversed or column-major output.
//!
//! All fixtures are built in-process via the low-level [`PdfWriter`]
//! API. PDF coordinate origin is bottom-left; Letter page = 612 × 792
//! pt; default body is 12 pt with 14.4 pt line height unless a fixture
//! sets otherwise.
//!
//! BT/ET separation: between consecutive `add_text` calls we insert a
//! zero-size `draw_rect(0, 0, 0, 0)` which forces an `ET` marker so
//! the next `add_text` starts a fresh `BT`. Without this the writer
//! keeps one open BT block across all calls and the extractor merges
//! adjacent text into a single wide span, which would defeat any
//! multi-column assertion.

use pdf_oxide::document::PdfDocument;
use pdf_oxide::writer::{PageBuilder, PdfWriter};

const PAGE_H: f32 = 792.0;
const MARGIN: f32 = 72.0;
const BODY: f32 = 12.0;
const LH: f32 = BODY * 1.2; // 14.4 pt

/// Add one text fragment and force a BT/ET boundary so the next text
/// fragment starts a fresh text object (otherwise the extractor merges
/// adjacent text on the same baseline into one span).
fn put(page: &mut PageBuilder<'_>, text: &str, x: f32, y: f32, font: &str, size: f32) {
    page.add_text(text, x, y, font, size);
    page.draw_rect(0.0, 0.0, 0.0, 0.0);
}

fn build_and_extract(build_fn: impl FnOnce(&mut PdfWriter)) -> String {
    let mut writer = PdfWriter::new();
    build_fn(&mut writer);
    let bytes = writer.finish().expect("build PDF");
    let doc = PdfDocument::from_bytes(bytes).expect("open PDF");
    doc.extract_text(0).expect("extract page 0")
}

/// Place body-text lines top-down at column x, starting from y_top.
fn place_column(page: &mut PageBuilder<'_>, x: f32, y_top: f32, lines: &[String]) {
    let mut y = y_top;
    for line in lines {
        put(page, line, x, y, "Helvetica", BODY);
        y -= LH;
    }
}

// =====================================================================
// Fixtures that MUST stay green from day one.
// =====================================================================

/// `body_single_column.pdf` — 30 lines of body text in a single column.
/// Should never trigger XY-cut. Output preserves input order.
#[test]
fn fixture_body_single_column_preserves_order() {
    let lines: Vec<String> = (1..=30)
        .map(|i| format!("Body line number {i:02} reads top to bottom in a single column."))
        .collect();
    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        place_column(&mut page, MARGIN, PAGE_H - MARGIN, &lines);
    });

    let mut last_pos = 0usize;
    for line in &lines {
        let pos = out[last_pos..].find(line.as_str()).unwrap_or_else(|| {
            panic!("line not found in expected order: {line:?}\n--- output ---\n{out}")
        }) + last_pos;
        last_pos = pos + line.len();
    }
}

/// `body_two_column.pdf` — clean two-column body. Left column must be
/// fully read before right column. No interleave like the legacy
/// `accompaally` / `correlaanonymous` artifacts.
///
/// 30 lines per column ensures the page passes `is_multi_column_page`'s
/// ≥15-spans-per-half threshold so XY-cut routing kicks in.
#[test]
fn fixture_body_two_column_no_interleave() {
    let left: Vec<String> = (1..=30)
        .map(|i| format!("Left{i:02} short body text"))
        .collect();
    let right: Vec<String> = (1..=30)
        .map(|i| format!("Right{i:02} other body text"))
        .collect();

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        place_column(&mut page, MARGIN, PAGE_H - MARGIN, &left);
        place_column(&mut page, 360.0, PAGE_H - MARGIN, &right);
    });

    let last_left = out
        .find(left.last().unwrap().as_str())
        .expect("last-left missing");
    let first_right = out
        .find(right.first().unwrap().as_str())
        .expect("first-right missing");
    assert!(
        last_left < first_right,
        "two-column reading order broken: last-left at {last_left}, first-right at {first_right}\n--- output ---\n{out}"
    );

    // No interleave: between two consecutive left lines, no right line.
    for pair in left.windows(2) {
        let a = out.find(pair[0].as_str()).unwrap();
        let b = out.find(pair[1].as_str()).unwrap();
        let between = &out[a..b];
        for r in &right {
            assert!(
                !between.contains(r.as_str()),
                "interleave detected: right line {r:?} appears between left {:?} and left {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}

/// `body_two_column_narrow_gutter.pdf` — 2 cols with only ~1.5 em gutter.
/// Edge case for valley detection in XY-cut. Should still split cleanly.
#[test]
fn fixture_body_two_column_narrow_gutter_still_splits() {
    // Short text so left column ends well before the right column starts.
    let left: Vec<String> = (1..=30).map(|i| format!("L{i:02} body")).collect();
    let right: Vec<String> = (1..=30).map(|i| format!("R{i:02} other")).collect();

    // Use realistic column widths (~220pt body, 18pt gutter ≈ 1.5em).
    // Each column has wider text so center spread of histogram still
    // shows two distinct peaks — the narrow-gutter case for XY-cut
    // valley detection.
    let left_wide: Vec<String> = (1..=30)
        .map(|i| format!("L{i:02} extended body text spanning whole column"))
        .collect();
    let right_wide: Vec<String> = (1..=30)
        .map(|i| format!("R{i:02} extended other text spanning whole column"))
        .collect();

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        place_column(&mut page, MARGIN, PAGE_H - MARGIN, &left_wide);
        // Left text is ~250pt wide (42 chars × 6pt) → ends at ~322.
        // Right at 340 → 18pt gutter (~1.5 em at 12pt body).
        place_column(&mut page, 340.0, PAGE_H - MARGIN, &right_wide);
    });

    let last_left = out
        .find(left_wide.last().unwrap().as_str())
        .expect("last-left missing");
    let first_right = out
        .find(right_wide.first().unwrap().as_str())
        .expect("first-right missing");
    assert!(
        last_left < first_right,
        "narrow-gutter two-column reading order broken\n--- output ---\n{out}"
    );
    let _ = (left, right); // keep the original short Vecs alive for future debug tweaks
}

/// `mixed_size_columns.pdf` — left col 12pt, right col 10pt. Dominant
/// font (mode by char count) should resolve to 12pt and the resulting
/// thresholds should still split the page correctly.
// TODO(#405): Re-enable once the table-detector density gate
// tightens. Cross-PR: accurate standard-14 font widths expose a
// pre-existing false-positive in the spatial table detector on
// mixed-size two-column body text — the detector fires because
// widths are now accurate and the word-grid looks table-shaped.
#[ignore]
#[test]
fn fixture_mixed_size_columns_dominant_em_picks_mode() {
    let left: Vec<String> = (1..=30)
        .map(|i| format!("Left12pt{i:02} body text content"))
        .collect();
    let right: Vec<String> = (1..=22)
        .map(|i| format!("Right10pt{i:02} smaller body text"))
        .collect();

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        let mut y = PAGE_H - MARGIN;
        for line in &left {
            put(&mut page, line, MARGIN, y, "Helvetica", 12.0);
            y -= 14.4;
        }
        let mut y = PAGE_H - MARGIN;
        for line in &right {
            put(&mut page, line, 360.0, y, "Helvetica", 10.0);
            y -= 12.0;
        }
    });

    let last_left = out
        .find(left.last().unwrap().as_str())
        .expect("last-left missing");
    let first_right = out
        .find(right.first().unwrap().as_str())
        .expect("first-right missing");
    assert!(
        last_left < first_right,
        "mixed-size two-column reading order broken\n--- output ---\n{out}"
    );
}

// =====================================================================
// Multi-column interleave invariants (issue #319).
//
// On the prior release the row-aware sort that ran after column
// detection re-interleaved the left- and right-column lines, producing
// garbled tokens like `accompaally` (= `accompa` from the left column
// glued to `ally` from the right). These fixtures pin the fix in place
// so the regression cannot recur silently.
// =====================================================================

/// Two-column body with hyphenation in the left column and unrelated
/// text on the right at the interleave Y. The hyphenated word must
/// rejoin cleanly within the left column without any right-column
/// tokens leaking into it.
#[test]
fn fixture_two_column_accompa_nying_no_interleave() {
    // 30 lines per column to trigger multi-column detection.
    let mut left: Vec<String> = (1..=12)
        .map(|i| format!("LeftPad{i:02} body text"))
        .collect();
    left.push("We refer to the accompa-".to_string());
    left.push("nying table for details.".to_string());
    left.extend((15..=30).map(|i| format!("LeftPad{i:02} body text")));

    let mut right: Vec<String> = (1..=12)
        .map(|i| format!("RightPad{i:02} other text"))
        .collect();
    right.push("This line reads really clearly.".to_string());
    right.push("This line is independent text.".to_string());
    right.extend((15..=30).map(|i| format!("RightPad{i:02} other text")));

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        place_column(&mut page, MARGIN, PAGE_H - MARGIN, &left);
        place_column(&mut page, 360.0, PAGE_H - MARGIN, &right);
    });

    for bad in &[
        "accompaally",
        "accompareally",
        "accompathis",
        "accompanyingreally",
    ] {
        assert!(
            !out.contains(bad),
            "Multi-column row-interleave artifact `{bad}` re-appeared:\n--- output ---\n{out}"
        );
    }
    assert!(
        out.contains("accompanying") || out.contains("accompa-nying"),
        "Hyphenated word `accompa-`/`nying` not rejoined cleanly:\n--- output ---\n{out}"
    );
}

/// `two_column_correla_tion_no_interleave.pdf` — second canonical
/// #319 case (`correlaanonymous`). `correla-` / `tion` hyphenation
/// in the left column, `anonymous` on the right side at interleave Y.
#[test]
fn fixture_two_column_correla_tion_no_interleave() {
    let mut left: Vec<String> = (1..=10)
        .map(|i| format!("LeftPad{i:02} body text"))
        .collect();
    left.push("We compute pairwise correla-".to_string());
    left.push("tion across all sample pairs.".to_string());
    left.extend((13..=30).map(|i| format!("LeftPad{i:02} body text")));

    let mut right: Vec<String> = (1..=10)
        .map(|i| format!("RightPad{i:02} other text"))
        .collect();
    right.push("All datasets are anonymous and labelled.".to_string());
    right.push("This line is independent text.".to_string());
    right.extend((13..=30).map(|i| format!("RightPad{i:02} other text")));

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        place_column(&mut page, MARGIN, PAGE_H - MARGIN, &left);
        place_column(&mut page, 360.0, PAGE_H - MARGIN, &right);
    });

    for bad in &["correlaanonymous", "correlathis", "correlaall"] {
        assert!(
            !out.contains(bad),
            "Multi-column row-interleave artifact `{bad}` re-appeared:\n--- output ---\n{out}"
        );
    }
    assert!(
        out.contains("correlation") || out.contains("correla-tion"),
        "Hyphenated word `correla-`/`tion` not rejoined cleanly:\n--- output ---\n{out}"
    );
}

/// `legal_style_two_column.pdf` — synthetic dense two-column layout
/// with hyphenated word breaks in both columns and an indented marker
/// (mimics CFR-style § section IDs without quoting any real regulation).
/// Validates that markers from the right column don't appear mid-sentence
/// in left-column text — the failure pattern that #319 fixed on real
/// CFR PDFs.
#[test]
fn fixture_legal_style_columns_no_section_marker_interleave() {
    let mut left: Vec<String> = vec![
        "Based upon the assessment by the certi-".to_string(),
        "fied reviewer, the Office shall re-".to_string(),
        "tain the right to request supplemen-".to_string(),
        "tary evaluations from external experts".to_string(),
        "selected by the Office. Such requests".to_string(),
        "must be approved in writing by the Re-".to_string(),
        "gional Director of Reviews before any".to_string(),
        "additional materials may be requested.".to_string(),
    ];
    left.extend((9..=32).map(|i| format!("LeftPad{i:02} body text")));

    let mut right: Vec<String> = vec![
        "A.1 Petition by an applicant to re-".to_string(),
        "move conditional restrictions on stand-".to_string(),
        "ing under this synthetic regulation.".to_string(),
        "(a) Filing rules. (1) General proce-".to_string(),
        "dures. A petition to remove the condi-".to_string(),
        "tional restriction shall be filed by".to_string(),
        "the applicant on the prescribed form".to_string(),
        "within the window described in (b).".to_string(),
    ];
    right.extend((9..=32).map(|i| format!("RightPad{i:02} body text")));

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        place_column(&mut page, MARGIN, PAGE_H - MARGIN, &left);
        place_column(&mut page, 360.0, PAGE_H - MARGIN, &right);
    });

    for bad in &[
        "certi-A.1",
        "certified A.1",
        "Office shall re- move",
        "Officemove",
    ] {
        assert!(
            !out.contains(bad),
            "Right-column text leaked into left-column sentence (`{bad}`):\n--- output ---\n{out}"
        );
    }

    let certified_pos = out
        .find("certified")
        .or_else(|| out.find("certi-fied"))
        .expect("`certified` (or hyphenated variant) missing from left column");
    let office_pos = out
        .find("Office")
        .expect("`Office` missing from left column");
    assert!(
        certified_pos < office_pos,
        "Left column reading order broken: certified at {certified_pos}, Office at {office_pos}\n--- output ---\n{out}"
    );
}

// =====================================================================
// Scattered-fragment layout invariants.
//
// Pages whose text is rendered as many positioned word-fragments
// (typical of older fax-style PDFs) used to look "multi-column" to the
// X-center histogram and routed through XY-cut, which then reversed
// fragments within each row or read the page column-major instead of
// row-major. The font-aware column-shape gate rejects these layouts.
// =====================================================================

/// Fax-style page: each row contains several separately positioned
/// word-fragments along the same baseline. The extractor must read
/// each row left-to-right; column-major or reversed output would mean
/// the multi-column gate let a sparse layout through.
#[test]
fn fixture_fax_scattered_fragments_no_reversal() {
    // 12 rows × 6 fragments = 72 spans, weighted toward the right half
    // so `is_multi_column_page` clears the ≥15-spans-per-side check and
    // the font-aware column-shape gate is the only thing keeping XY-cut
    // from mis-routing this page as multi-column.
    let lines: &[&[(f32, &str)]] = &[
        &[
            (72.0, "WORDONE"),
            (140.0, "WORDTWO"),
            (200.0, "WORDTHREE"),
            (260.0, "WORDFOUR"),
            (320.0, "WORDFIVE"),
            (390.0, "WORDSIX"),
        ],
        &[
            (72.0, "ALPHA"),
            (130.0, "BETA"),
            (190.0, "GAMMA"),
            (260.0, "DELTA"),
            (320.0, "EPSILON"),
            (400.0, "ZETA"),
        ],
        &[
            (72.0, "First"),
            (130.0, "Second"),
            (200.0, "Third"),
            (260.0, "Fourth"),
            (320.0, "Fifth"),
            (390.0, "Sixth"),
        ],
        &[
            (72.0, "PartA"),
            (130.0, "PartB"),
            (200.0, "PartC"),
            (260.0, "PartD"),
            (320.0, "PartE"),
            (390.0, "PartF"),
        ],
        &[
            (72.0, "ItemX"),
            (130.0, "ItemY"),
            (190.0, "ItemZ"),
            (260.0, "ItemW"),
            (320.0, "ItemV"),
            (400.0, "ItemU"),
        ],
        &[
            (72.0, "Quux"),
            (130.0, "Garply"),
            (200.0, "Waldo"),
            (260.0, "Fred"),
            (320.0, "Plugh"),
            (390.0, "Xyzzy"),
        ],
        &[
            (72.0, "Foo"),
            (130.0, "Bar"),
            (190.0, "Baz"),
            (260.0, "Qux"),
            (320.0, "Corge"),
            (400.0, "Grault"),
        ],
        &[
            (72.0, "Apple"),
            (130.0, "Banana"),
            (200.0, "Cherry"),
            (260.0, "Date"),
            (320.0, "Elder"),
            (390.0, "Fig"),
        ],
        &[
            (72.0, "Red"),
            (130.0, "Green"),
            (200.0, "Blue"),
            (260.0, "Cyan"),
            (320.0, "Magenta"),
            (390.0, "Yellow"),
        ],
        &[
            (72.0, "North"),
            (130.0, "South"),
            (200.0, "East"),
            (260.0, "West"),
            (320.0, "Up"),
            (390.0, "Down"),
        ],
        &[
            (72.0, "Mon"),
            (130.0, "Tue"),
            (200.0, "Wed"),
            (260.0, "Thu"),
            (320.0, "Fri"),
            (390.0, "Sat"),
        ],
        &[
            (72.0, "Spring"),
            (130.0, "Summer"),
            (200.0, "Autumn"),
            (260.0, "Winter"),
            (320.0, "Solstice"),
            (390.0, "Equinox"),
        ],
    ];

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        let mut y = PAGE_H - MARGIN;
        for line_frags in lines {
            for (x, word) in line_frags.iter() {
                put(&mut page, word, *x, y, "Helvetica", 8.0);
            }
            y -= 11.0;
        }
    });

    for bad in &[
        "WORDFIVEWORDFOURWORDTHREE",
        "WORDFOURWORDTHREEWORDTWOWORDONE",
        "EPSILONDELTAGAMMABETAALPHA",
        "FifthFourthThirdSecondFirst",
    ] {
        assert!(!out.contains(bad), "Reversal artifact `{bad}` detected in output:\n{out}");
    }

    // Within-row order: every fragment of row 1 must precede every
    // fragment of row 2, otherwise the page was read column-major —
    // the failure mode where XY-cut treats a scattered-fragment page
    // as multi-column and reads all left-column words before any
    // right-column word.
    let pos = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle:?} missing in output:\n{out}"))
    };
    let row1 = [
        "WORDONE",
        "WORDTWO",
        "WORDTHREE",
        "WORDFOUR",
        "WORDFIVE",
        "WORDSIX",
    ];
    let row2 = ["ALPHA", "BETA", "GAMMA", "DELTA", "EPSILON", "ZETA"];

    // Row 1 in left-to-right order.
    for pair in row1.windows(2) {
        assert!(
            pos(pair[0]) < pos(pair[1]),
            "Row-1 reading order reversed: {} at {} > {} at {}\n--- output ---\n{out}",
            pair[0],
            pos(pair[0]),
            pair[1],
            pos(pair[1])
        );
    }
    // Row 2 in left-to-right order.
    for pair in row2.windows(2) {
        assert!(
            pos(pair[0]) < pos(pair[1]),
            "Row-2 reading order reversed: {} at {} > {} at {}\n--- output ---\n{out}",
            pair[0],
            pos(pair[0]),
            pair[1],
            pos(pair[1])
        );
    }
    // The last fragment of row 1 must come BEFORE the first fragment
    // of row 2 — proves we're reading row-major, not column-major.
    let last_row1 = pos("WORDSIX");
    let first_row2 = pos("ALPHA");
    assert!(
        last_row1 < first_row2,
        "Column-major output detected: WORDSIX at {last_row1} > ALPHA at {first_row2} — XY-cut wrongly read this scattered page as multi-column\n--- output ---\n{out}"
    );
}

/// 3×3 table where the same word stacks in three vertical cells. The
/// span emitter must insert a separator between cells whose vertical
/// gap exceeds the line-height threshold; concatenation without one
/// produces tokens like `instancesinstancesinstances`.
#[test]
fn fixture_table_3x3_cells_not_concatenated() {
    let cell_text = "instances";
    let cols = [120.0_f32, 250.0, 380.0];
    let rows = [
        PAGE_H - MARGIN,
        PAGE_H - MARGIN - 30.0,
        PAGE_H - MARGIN - 60.0,
    ];

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        for &x in &cols {
            for &y in &rows {
                put(&mut page, cell_text, x, y, "Helvetica", BODY);
            }
        }
    });

    assert!(
        !out.contains("instancesinstancesinstances"),
        "Adjacent cells concatenated without separator:\n{out}"
    );
    let occurrences = out.matches("instances").count();
    assert!(
        occurrences >= 9,
        "Expected ≥9 occurrences of cell text, found {occurrences}\n--- output ---\n{out}"
    );
}

/// Two columns where some lines end with `-` to mark a word continuing
/// on the next line. The dehyphenation pass must strip the hyphen and
/// rejoin the continuation; an unjoined output (`compre-` followed by
/// a literal newline before `hensive`) leaves the prose unreadable.
#[test]
fn fixture_two_column_hyphen_rejoin() {
    let mut left: Vec<String> = (1..=10)
        .map(|i| format!("LeftPad{i:02} body text"))
        .collect();
    left.push("Continuation requires compre-".to_string());
    left.push("hensive understanding here.".to_string());
    left.extend((13..=30).map(|i| format!("LeftPad{i:02} body text")));

    let mut right: Vec<String> = (1..=10)
        .map(|i| format!("RightPad{i:02} other text"))
        .collect();
    right.push("Some financial terms cross-".to_string());
    right.push("collateralized in detail.".to_string());
    right.extend((13..=30).map(|i| format!("RightPad{i:02} other text")));

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        place_column(&mut page, MARGIN, PAGE_H - MARGIN, &left);
        place_column(&mut page, 360.0, PAGE_H - MARGIN, &right);
    });

    let comp_joined = out.contains("comprehensive") || out.contains("compre-hensive");
    let cross_joined = out.contains("crosscollateralized") || out.contains("cross-collateralized");
    assert!(
        comp_joined,
        "End-of-line hyphen for `compre-`/`hensive` not rejoined\n--- output ---\n{out}"
    );
    assert!(
        cross_joined,
        "End-of-line hyphen for `cross-`/`collateralized` not rejoined\n--- output ---\n{out}"
    );
}

/// Body text in 12 pt with a 9 pt caption embedded mid-column. The
/// caption is a distinct typographic region; concatenating its first
/// word onto the body sentence (`conceptFigure`) would corrupt the
/// surrounding prose.
#[test]
fn fixture_figure_caption_not_merged_into_body() {
    let body_lines_pre = [
        "We describe the two dimensions through the concept",
        "outlined in the diagram presented further below,",
    ];
    let caption_line = "Figure 1: Synthetic illustration accompanying this fixture only.";
    let body_lines_post = [
        "which captures the relevant scaling properties and",
        "demonstrates the expected behaviour in this setting.",
    ];

    let out = build_and_extract(|w| {
        let mut page = w.add_letter_page();
        let mut y = PAGE_H - MARGIN;
        for line in &body_lines_pre {
            put(&mut page, line, MARGIN, y, "Helvetica", 12.0);
            y -= 14.4;
        }
        y -= 8.0;
        put(&mut page, caption_line, MARGIN, y, "Helvetica-Oblique", 9.0);
        y -= 14.0;
        for line in &body_lines_post {
            put(&mut page, line, MARGIN, y, "Helvetica", 12.0);
            y -= 14.4;
        }
    });

    for bad in &[
        "conceptFigure",
        "conceptSynthetic",
        "belowFigure",
        "belowSynthetic",
        "conceptIllustration",
    ] {
        assert!(!out.contains(bad), "Body word collided with caption word (`{bad}`):\n{out}");
    }

    let concept = out.find("concept").expect("concept missing");
    let scaling = out.find("scaling").expect("scaling missing");
    assert!(
        concept < scaling,
        "Body sentence broken: concept at {concept}, scaling at {scaling}\n--- output ---\n{out}"
    );
}
