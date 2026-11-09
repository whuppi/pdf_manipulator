//! Table detection holds the rulings of a page, never its paths.
//!
//! A page of ruled tables is built here in two forms: as a document, for the
//! public path (`extract_paths`, `extract_tables`), and as paths handed
//! straight to a `RulingGrid`, for sizes no document test would read in time.

use super::support::{resolution_secs, thread_cpu_secs};
use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::elements::PathContent;
use crate::geometry::Rect;
use crate::host::ruling_grid::RulingGrid;
use crate::layout::TextSpan;
use crate::structure::spatial_table_detector::{detect_tables, TableDetectionConfig, TableStrategy};

/// One-page document around a content stream, with a large page and Helvetica as F1.
fn page_with(content: &str, size: u32) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {size} {size}] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n"
        )
        .into_bytes(),
        stream_object(4, "", content.as_bytes()),
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ])
}

/// Rows and columns of a drawn table, and the size of its cells, in points.
const TABLE_ROWS: usize = 14;
const TABLE_COLS: usize = 14;
const CELL_W: f32 = 24.0;
const CELL_H: f32 = 12.0;

/// The content of one table with its lower left corner at (x, y): a ruling
/// for every row and column edge (15 + 15 = 30 primitives) and a word in each cell.
fn table_content(x: f32, y: f32, out: &mut String) {
    for r in 0..=TABLE_ROWS {
        let yy = y + r as f32 * CELL_H;
        out.push_str(&format!("{x} {yy} m {} {yy} l S\n", x + TABLE_COLS as f32 * CELL_W));
    }
    for c in 0..=TABLE_COLS {
        let xx = x + c as f32 * CELL_W;
        out.push_str(&format!("{xx} {y} m {xx} {} l S\n", y + TABLE_ROWS as f32 * CELL_H));
    }
    for r in 0..TABLE_ROWS {
        for c in 0..TABLE_COLS {
            out.push_str(&format!(
                "BT /F1 6 Tf {} {} Td ({}.5) Tj ET\n",
                x + c as f32 * CELL_W + 3.0,
                y + r as f32 * CELL_H + 3.0,
                r * TABLE_COLS + c
            ));
        }
    }
}

/// Sixty 14 x 14 tables, 30 primitives each: 1,800 in all, on one page.
fn sixty_tables() -> Vec<u8> {
    let mut content = String::from("0.5 w\n");
    for n in 0..60 {
        table_content(60.0 + (n % 6) as f32 * 400.0, 60.0 + (n / 6) as f32 * 220.0, &mut content);
    }
    page_with(&content, 2500)
}

/// No Dart battery sees it: a battery asserts what one small page returns,
/// and the cap this replaces counted primitives, so only a page with more of
/// them than the cap tells a detector that reads them all from one that reads
/// the first 1,500.
#[test]
fn ruled_page_over_fifteen_hundred_primitives_detects_every_table() {
    let doc = PdfDocument::from_bytes(sixty_tables()).unwrap();
    assert_eq!(doc.extract_paths(0).unwrap().len(), 1_800, "the page holds 1,800 primitives");
    let tables = doc.extract_tables(0).unwrap();
    assert_eq!(tables.len(), 60, "every table is found, not the ones among the first 1,500 paths");
    assert!(tables.iter().all(|t| t.rows.len() == TABLE_ROWS && t.col_count == TABLE_COLS));

    // The text and markdown readers took the first 1,500 paths for the page's
    // rulings and found no table at all when the page held more.
    let markdown = doc.to_markdown(0, &crate::converters::ConversionOptions::default()).unwrap();
    let separators = markdown
        .lines()
        .filter(|l| l.contains("---") && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')))
        .count();
    assert_eq!(separators, 60, "every table is a table in the markdown too");
}

/// No Dart battery sees it: a battery cannot tell the paths a sink was handed
/// from the ones a `Vec` keeps, and the whole point of the sink is that the
/// caller decides what is kept.
#[test]
fn a_sink_sees_the_paths_extract_paths_returns_in_order() {
    let doc = PdfDocument::from_bytes(sixty_tables()).unwrap();
    let kept = doc.extract_paths(0).unwrap();
    let mut seen: Vec<PathContent> = Vec::new();
    doc.extract_paths_into(0, &mut |p| seen.push(p)).unwrap();
    assert_eq!(kept.len(), seen.len());
    assert!(kept.iter().zip(&seen).all(|(a, b)| a.bbox == b.bbox && a.operations == b.operations));
    // A counting sink keeps nothing and sees every path.
    let mut count = 0usize;
    doc.extract_paths_into(0, &mut |_| count += 1).unwrap();
    assert_eq!(count, kept.len());
}

/// A page of `segments` random short strokes, 6 to 14 pt long at any angle, and
/// `words` numbered words, from `seed`. Some strokes lie within 2 pt of an
/// axis, so they are rules, and the page is dense with them: noise a grid can
/// be read into by chance.
fn random_strokes(seed: u64, segments: usize, words: usize) -> Vec<u8> {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15 ^ seed.wrapping_mul(0x1234_5678_9ABC_DEF1);
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f32 / (1u64 << 53) as f32
    };
    let mut content = String::from("0.5 w\n");
    for _ in 0..segments {
        let (x, y) = (50.0 + next() * 900.0, 50.0 + next() * 900.0);
        let (len, angle) = (6.0 + next() * 8.0, next() * std::f32::consts::TAU);
        content.push_str(&format!("{x} {y} m {} {} l S\n", x + len * angle.cos(), y + len * angle.sin()));
    }
    for n in 0..words {
        content.push_str(&format!(
            "BT /F1 8 Tf {} {} Td (w{n}) Tj ET\n",
            50.0 + next() * 900.0,
            50.0 + next() * 900.0
        ));
    }
    page_with(&content, 1000)
}

/// The detector set to read rulings only: the words alone may form a table by
/// their alignment, and these proofs are about what the strokes make of one.
fn rulings_only() -> TableDetectionConfig {
    TableDetectionConfig {
        horizontal_strategy: TableStrategy::Lines,
        vertical_strategy: TableStrategy::Lines,
        text_fallback: false,
        ..TableDetectionConfig::default()
    }
}

/// No Dart battery sees it: it takes a page of ten thousand random strokes,
/// more than the cap this replaces let through, and a battery asserts on what
/// a real document shows. Six seeds, with 400 words and with 2,000: before the
/// grid had to hold text, the 2,000-word pages made 1 to 5 tables each.
#[test]
fn ten_thousand_random_short_segments_detect_no_table() {
    for words in [400, 2_000] {
        for seed in 0..6 {
            let doc = PdfDocument::from_bytes(random_strokes(seed, 10_000, words)).unwrap();
            let tables = doc.extract_tables_with_config(0, rulings_only()).unwrap();
            let found: Vec<_> = tables.iter().map(|t| (t.rows.len(), t.col_count, t.bbox)).collect();
            assert!(
                tables.is_empty(),
                "seed {seed}, {words} words: tables found among random strokes: {found:?}"
            );
        }
    }
}

/// A span at a point, for a detector that reads a word per cell.
fn word_at(x: f32, y: f32) -> TextSpan {
    TextSpan { text: "w".to_string(), bbox: Rect::new(x, y, 6.0, 8.0), font_size: 8.0, ..Default::default() }
}

/// The rulings and words of `tables` tables of 3 x 3 cells in a repeating
/// lattice of pitch 120: each 8 rulings and 9 numbers.
fn lattice(tables: usize) -> (RulingGrid, Vec<TextSpan>) {
    let per_row = 300usize;
    let bounds = Rect::new(0.0, 0.0, 120.0 * per_row as f32, 120.0 * (tables / per_row + 1) as f32);
    let mut grid = RulingGrid::new(bounds);
    let mut spans = Vec::with_capacity(tables * 9);
    for n in 0..tables {
        let (x, y) = ((n % per_row) as f32 * 120.0 + 10.0, (n / per_row) as f32 * 120.0 + 10.0);
        for i in 0..=3 {
            let d = i as f32 * 30.0;
            grid.add(&PathContent::line(x, y + d, x + 90.0, y + d));
            grid.add(&PathContent::line(x + d, y, x + d, y + 90.0));
        }
        for r in 0..3 {
            for c in 0..3 {
                spans.push(word_at(x + c as f32 * 30.0 + 8.0, y + r as f32 * 30.0 + 10.0));
            }
        }
    }
    assert!(!grid.refused());
    (grid, spans)
}

/// The tables found in a lattice of `tables` and the CPU milliseconds the grid
/// and the detector took together on this thread, per 10,000 primitives.
fn milliseconds_per_ten_thousand(tables: usize) -> (usize, f64) {
    let started = thread_cpu_secs();
    let (grid, spans) = lattice(tables);
    let primitives = grid.primitives();
    let found = detect_tables(
        &spans,
        &grid,
        &TableDetectionConfig::default(),
        &crate::cancel::CancelCheck::none(),
    )
    .unwrap();
    let count = found.len();
    std::hint::black_box(&found);
    let ms = (thread_cpu_secs() - started) * 1000.0;
    (count, ms / (primitives as f64 / 10_000.0))
}

/// Milliseconds per 10,000 primitives at `tables` tables: the quicker of
/// `runs` runs, so a slow start does not stand for the size. Every table must be found.
fn best_milliseconds(tables: usize, runs: usize) -> f64 {
    (0..runs)
        .map(|_| {
            let (found, ms) = milliseconds_per_ten_thousand(tables);
            assert_eq!(found, tables, "every table of the lattice is found");
            ms
        })
        .fold(f64::MAX, f64::min)
}

#[cfg(not(debug_assertions))]
/// No Dart battery sees it: only a comparison of two sizes shows growth, and
/// a million rulings is too many for a document test. Compiled for release
/// only, since the debug profile runs it for minutes.
#[test]
fn a_hundred_thousand_and_a_million_rulings_cost_the_same_per_primitive() {
    let small = best_milliseconds(12_500, 2);
    let large = best_milliseconds(125_000, 1);
    eprintln!("per 10k primitives: 100k rulings {small:.1} ms, 1M rulings {large:.1} ms");
    assert!(large / small < 1.3, "{small:.1} ms at 100k and {large:.1} ms at 1M per 10k primitives");
}

#[cfg(debug_assertions)]
/// No Dart battery sees it, as above. The debug profile reads a smaller pair.
#[test]
fn ten_thousand_and_a_hundred_thousand_rulings_cost_about_the_same_per_primitive() {
    let small = best_milliseconds(1_250, 2);
    let large = best_milliseconds(12_500, 1);
    eprintln!("per 10k primitives: 10k rulings {small:.1} ms, 100k rulings {large:.1} ms");
    assert!(large / small < 1.6, "{small:.1} ms at 10k and {large:.1} ms at 100k per 10k primitives");
}

/// The ledger's columns, left to right.
const LEDGER_HEADER: [&str; 20] = [
    "Account", "Code", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
    "Nov", "Dec", "Q1", "Q2", "Q3", "Q4", "Total", "Avg",
];

/// A monthly ledger: an account and a code, twelve months, four quarters, a
/// total and an average (20 columns), thirteen rows under a header row, every
/// line of the grid ruled and a number in every cell.
fn ledger() -> Vec<u8> {
    let widths: Vec<f32> = [62.0, 32.0].into_iter().chain([34.0; 12]).chain([40.0; 4]).chain([46.0, 38.0]).collect();
    let (x0, y0, row_h) = (20.0f32, 40.0f32, 16.0f32);
    let mut rows: Vec<Vec<String>> = vec![LEDGER_HEADER.iter().map(|s| s.to_string()).collect()];
    for r in 0..12usize {
        let months: Vec<usize> = (0..12).map(|m| 1_000 + (r * 977 + m * 389) % 9_000).collect();
        let quarters: Vec<usize> = (0..4).map(|q| months[q * 3..q * 3 + 3].iter().sum()).collect();
        let total: usize = months.iter().sum();
        let mut row = vec![format!("Account{r}"), format!("{}", 100 + r)];
        row.extend(months.iter().map(|v| v.to_string()));
        row.extend(quarters.iter().map(|v| v.to_string()));
        row.extend([total.to_string(), (total / 12).to_string()]);
        rows.push(row);
    }
    let mut net = vec!["Net".to_string(), "999".to_string()];
    for c in 2..LEDGER_HEADER.len() {
        net.push(rows[1..].iter().map(|r| r[c].parse::<usize>().unwrap()).sum::<usize>().to_string());
    }
    rows.push(net);
    let table_w: f32 = widths.iter().sum();
    let table_h = rows.len() as f32 * row_h;
    let mut content = String::from("0.5 w\n");
    for r in 0..=rows.len() {
        let y = y0 + r as f32 * row_h;
        content.push_str(&format!("{x0} {y} m {} {y} l S\n", x0 + table_w));
    }
    let mut x = x0;
    for c in 0..=widths.len() {
        content.push_str(&format!("{x} {y0} m {x} {} l S\n", y0 + table_h));
        if c < widths.len() {
            x += widths[c];
        }
    }
    for (r, row) in rows.iter().enumerate() {
        let mut x = x0;
        for (c, cell) in row.iter().enumerate() {
            let y = y0 + (rows.len() - 1 - r) as f32 * row_h + 5.0;
            content.push_str(&format!("BT /F1 6 Tf {} {y} Td ({cell}) Tj ET\n", x + 2.0));
            x += widths[c];
        }
    }
    page_with(&content, 1_000)
}

/// No Dart battery sees it: the column limit dropped such a table without a
/// word, and a battery has no 20-column page to ask about.
#[test]
fn a_ruled_ledger_of_twenty_columns_is_one_table() {
    let doc = PdfDocument::from_bytes(ledger()).unwrap();
    let tables = doc.extract_tables(0).unwrap();
    assert_eq!(tables.len(), 1, "the ledger is one table");
    let table = &tables[0];
    assert_eq!((table.rows.len(), table.col_count), (14, 20));
    let cells: Vec<&str> = table.rows.iter().flat_map(|r| r.cells.iter().map(|c| c.text.trim())).collect();
    for word in ["Account", "Dec", "Avg", "Net", "Account11"] {
        assert!(cells.contains(&word), "{word} is a cell of the ledger: {cells:?}");
    }
}

/// An engineering drawing: a sheet border with zone ticks, three views with
/// centre lines, a hatched section, two crosshatched details with leader
/// numbers, dimension lines with arrowheads, scattered labels, a bill of
/// materials and a title block. The last two are the drawing's tables.
fn engineering_drawing() -> Vec<u8> {
    let mut content = String::new();
    let line = |c: &mut String, x1: f32, y1: f32, x2: f32, y2: f32, w: f32| {
        c.push_str(&format!("{w} w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
    };
    let rect = |c: &mut String, x: f32, y: f32, w: f32, h: f32, lw: f32| {
        c.push_str(&format!("{lw} w {x:.2} {y:.2} {w:.2} {h:.2} re S\n"));
    };
    let text = |c: &mut String, x: f32, y: f32, s: &str, size: f32| {
        c.push_str(&format!("BT /F1 {size} Tf {x:.2} {y:.2} Td ({s}) Tj ET\n"));
    };
    let (w, h) = (1_190.0f32, 842.0f32);
    rect(&mut content, 20.0, 20.0, w - 40.0, h - 40.0, 1.0);
    rect(&mut content, 35.0, 35.0, w - 70.0, h - 70.0, 0.7);
    for i in 1..8 {
        let x = 35.0 + (w - 70.0) * i as f32 / 8.0;
        line(&mut content, x, 20.0, x, 35.0, 0.5);
        line(&mut content, x, h - 35.0, x, h - 20.0, 0.5);
    }
    for i in 1..6 {
        let y = 35.0 + (h - 70.0) * i as f32 / 6.0;
        line(&mut content, 20.0, y, 35.0, y, 0.5);
        line(&mut content, w - 35.0, y, w - 20.0, y, 0.5);
    }
    let view = |c: &mut String, x: f32, y: f32, vw: f32, vh: f32, name: &str| {
        rect(c, x, y, vw, vh, 0.9);
        for f in [0.25, 0.75] {
            line(c, x + vw * f, y, x + vw * f, y + vh, 0.5);
        }
        for f in [0.3, 0.7] {
            line(c, x, y + vh * f, x + vw, y + vh * f, 0.5);
        }
        let mut k = 0.0;
        while k < vw {
            line(c, x + k, y + vh / 2.0, x + (k + 9.0).min(vw), y + vh / 2.0, 0.3);
            k += 14.0;
        }
        let mut k = 0.0;
        while k < vh {
            line(c, x + vw / 2.0, y + k, x + vw / 2.0, y + (k + 9.0).min(vh), 0.3);
            k += 14.0;
        }
        text(c, x, y - 12.0, name, 9.0);
    };
    view(&mut content, 80.0, 420.0, 260.0, 200.0, "FRONT VIEW");
    view(&mut content, 400.0, 420.0, 200.0, 200.0, "SIDE VIEW");
    view(&mut content, 80.0, 150.0, 260.0, 180.0, "TOP VIEW");
    // The section, hatched at 45 degrees.
    let (sx, sy, sw, sh) = (680.0f32, 500.0f32, 300.0f32, 200.0f32);
    rect(&mut content, sx, sy, sw, sh, 0.9);
    let mut d = -sh;
    while d < sw {
        let (x1, y1) = (sx + d.max(0.0), sy + (-d).max(0.0));
        let (x2, y2) = (sx + (d + sh).min(sw), sy + sh.min(sw - d));
        if x2 > x1 {
            line(&mut content, x1, y1, x2, y2, 0.25);
        }
        d += 6.0;
    }
    text(&mut content, sx, sy - 12.0, "SECTION A-A", 9.0);
    // Two crosshatched details, with leader numbers inside.
    rect(&mut content, 700.0, 210.0, 120.0, 90.0, 0.7);
    for k in 1..24 {
        line(&mut content, 700.0, 210.0 + k as f32 * 90.0 / 24.0, 820.0, 210.0 + k as f32 * 90.0 / 24.0, 0.2);
    }
    for k in 1..30 {
        line(&mut content, 700.0 + k as f32 * 120.0 / 30.0, 210.0, 700.0 + k as f32 * 120.0 / 30.0, 300.0, 0.2);
    }
    rect(&mut content, 860.0, 210.0, 117.0, 81.0, 0.7);
    for k in 1..9 {
        line(&mut content, 860.0, 210.0 + k as f32 * 9.0, 977.0, 210.0 + k as f32 * 9.0, 0.25);
    }
    for k in 1..13 {
        line(&mut content, 860.0 + k as f32 * 9.0, 210.0, 860.0 + k as f32 * 9.0, 291.0, 0.25);
    }
    for i in 0..30 {
        let (x, y) = (702.0 + (i * 37 % 112) as f32, 213.0 + (i * 53 % 80) as f32);
        text(&mut content, x, y, ["3", "7", "12", "A", "B2", "0.5"][i % 6], 5.0);
    }
    for i in 0..40 {
        let (x, y) = (862.0 + (i * 41 % 110) as f32, 212.0 + (i * 29 % 74) as f32);
        text(&mut content, x, y, ["1", "2", "3", "5", "8", "13", "21", "34"][i % 8], 5.0);
    }
    text(&mut content, 700.0, 306.0, "DETAIL B (CROSSHATCH)", 8.0);
    // Dimension lines: arrowheads, extension ticks and a figure each.
    let hdim = |c: &mut String, x1: f32, x2: f32, y: f32, label: &str| {
        line(c, x1, y, x2, y, 0.4);
        line(c, x1, y - 6.0, x1, y + 6.0, 0.4);
        line(c, x2, y - 6.0, x2, y + 6.0, 0.4);
        for (ax, bx) in [(x1, x1 + 6.0), (x2, x2 - 6.0)] {
            line(c, ax, y, bx, y + 2.0, 0.4);
            line(c, ax, y, bx, y - 2.0, 0.4);
        }
        text(c, (x1 + x2) / 2.0 - 10.0, y + 3.0, label, 7.0);
    };
    let vdim = |c: &mut String, y1: f32, y2: f32, x: f32, label: &str| {
        line(c, x, y1, x, y2, 0.4);
        line(c, x - 6.0, y1, x + 6.0, y1, 0.4);
        line(c, x - 6.0, y2, x + 6.0, y2, 0.4);
        for (ay, by) in [(y1, y1 + 6.0), (y2, y2 - 6.0)] {
            line(c, x, ay, x + 2.0, by, 0.4);
            line(c, x, ay, x - 2.0, by, 0.4);
        }
        text(c, x + 4.0, (y1 + y2) / 2.0, label, 7.0);
    };
    hdim(&mut content, 80.0, 340.0, 640.0, "260.00");
    hdim(&mut content, 400.0, 600.0, 640.0, "200.00");
    hdim(&mut content, 680.0, 980.0, 720.0, "300.00");
    vdim(&mut content, 420.0, 620.0, 60.0, "200.00");
    vdim(&mut content, 500.0, 700.0, 1000.0, "200.00");
    vdim(&mut content, 150.0, 330.0, 60.0, "180.00");
    hdim(&mut content, 80.0, 160.0, 350.0, "80.00");
    hdim(&mut content, 160.0, 340.0, 350.0, "180.00");
    // Scattered labels, kept off the two tables.
    let mut state: u64 = 0x7E57_ED17_C0DE;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f32 / (1u64 << 53) as f32
    };
    let notes = ["R5", "R12", "M8", "1/4-20", "45", "30", "DIA 6", "TYP", "SEE NOTE 3", "0.05", "REF", "2X", "C1", "FIG 2"];
    let mut placed = 0;
    while placed < 60 {
        let (x, y) = (60.0 + next() * 1_040.0, 60.0 + next() * 720.0);
        let near_table = (x > 670.0 && y < 470.0 && y > 30.0) || (x > 670.0 && y > 470.0 && y < 720.0);
        if near_table {
            continue;
        }
        text(&mut content, x, y, notes[placed % notes.len()], 6.0);
        placed += 1;
    }
    // The bill of materials: 5 columns, a header and 7 parts.
    let bom = [
        ["ITEM", "QTY", "PART NO", "DESCRIPTION", "MATERIAL"],
        ["1", "1", "AS-1001", "BASE PLATE", "STEEL"],
        ["2", "2", "AS-1002", "SIDE BRACKET", "ALUMINUM"],
        ["3", "4", "FS-0420", "BOLT M8x30", "STEEL"],
        ["4", "4", "FS-0421", "WASHER M8", "STEEL"],
        ["5", "1", "AS-1003", "COVER", "ABS"],
        ["6", "2", "GS-0110", "GASKET", "RUBBER"],
        ["7", "1", "AS-1004", "SHAFT", "STEEL"],
    ];
    let title = [
        ["TITLE", "BRACKET ASSEMBLY", "DWG NO", "AS-1000", "REV", "B"],
        ["DRAWN", "J. SMITH", "DATE", "2026-10-31", "SCALE", "1:2"],
        ["CHECKED", "A. KUMAR", "DATE", "2026-11-02", "SHEET", "1 OF 1"],
        ["APPROVED", "R. LEE", "DATE", "2026-11-03", "MASS", "2.4 KG"],
        ["COMPANY", "ACME TOOLING", "MATERIAL", "STEEL", "FINISH", "ZINC"],
    ];
    let ruled = |c: &mut String, x0: f32, y0: f32, widths: &[f32], row_h: f32, rows: &[&[&str]]| {
        let table_w: f32 = widths.iter().sum();
        for r in 0..=rows.len() {
            line(c, x0, y0 + r as f32 * row_h, x0 + table_w, y0 + r as f32 * row_h, 0.8);
        }
        let mut x = x0;
        for k in 0..=widths.len() {
            line(c, x, y0, x, y0 + rows.len() as f32 * row_h, 0.8);
            if k < widths.len() {
                x += widths[k];
            }
        }
        for (r, row) in rows.iter().enumerate() {
            let mut x = x0;
            for (k, cell) in row.iter().enumerate() {
                text(c, x + 3.0, y0 + (rows.len() - 1 - r) as f32 * row_h + 5.0, cell, 7.0);
                x += widths[k];
            }
        }
    };
    let bom_rows: Vec<&[&str]> = bom.iter().map(|r| &r[..]).collect();
    let title_rows: Vec<&[&str]> = title.iter().map(|r| &r[..]).collect();
    ruled(&mut content, 700.0, 330.0, &[40.0, 70.0, 150.0, 120.0, 90.0], 16.0, &bom_rows);
    ruled(&mut content, 690.0, 45.0, &[70.0, 110.0, 70.0, 110.0, 60.0, 60.0], 18.0, &title_rows);
    page_with(&content, 1_200)
}

/// No Dart battery sees it: the page is the shape dense line art takes in a
/// CAD export, and what it asserts is that the two real tables on it come out
/// whole while the hatching, crosshatch, dimension lines and scattered figures
/// make none.
#[test]
fn a_drawing_keeps_its_bill_of_materials_and_title_block_and_nothing_else() {
    let doc = PdfDocument::from_bytes(engineering_drawing()).unwrap();
    let tables = doc.extract_tables(0).unwrap();
    let shape: Vec<_> = tables.iter().map(|t| (t.rows.len(), t.col_count)).collect();
    assert_eq!(tables.len(), 2, "the bill of materials and the title block, nothing else: {shape:?}");
    let texts = |t: &crate::structure::Table| -> Vec<String> {
        t.rows.iter().flat_map(|r| r.cells.iter().map(|c| c.text.trim().to_string())).collect()
    };
    let bom = tables.iter().find(|t| texts(t).contains(&"PART NO".to_string())).expect("the bill of materials");
    let title = tables.iter().find(|t| texts(t).contains(&"BRACKET ASSEMBLY".to_string())).expect("the title block");
    assert_eq!((bom.rows.len(), bom.col_count), (8, 5), "{:?}", texts(bom));
    assert_eq!((title.rows.len(), title.col_count), (5, 6), "{:?}", texts(title));
    for word in ["SHAFT", "ALUMINUM", "AS-1004"] {
        assert!(texts(bom).contains(&word.to_string()), "{word}");
    }
    for word in ["ACME TOOLING", "2.4 KG", "1 OF 1"] {
        assert!(texts(title).contains(&word.to_string()), "{word}");
    }
}

/// A table with a header cell merged across all 14 columns and 7 rows of 14
/// cells, of which one holds text, as colspan merging leaves it: every row
/// after the header carries one cell.
fn ragged(rows_filled: usize) -> crate::structure::Table {
    use crate::structure::{Table, TableCell, TableRow};
    let cell = |text: &str, colspan: u32| TableCell {
        text: text.to_string(),
        spans: Vec::new(),
        colspan,
        rowspan: 1,
        mcids: Vec::new(),
        bbox: None,
        is_header: false,
    };
    let mut rows = Vec::new();
    for r in 0..8 {
        let mut row = TableRow::new(false);
        row.cells.push(cell(if r < rows_filled { "text" } else { "" }, 14));
        rows.push(row);
    }
    Table { rows, has_header: false, col_count: 14, bbox: None }
}

/// The empty share is taken over the grid, `rows * col_count`, not over the
/// cells a row happens to carry. Merged cells left 8 rows of 14 with one cell
/// each; with one filled that is 111 empty of 112, and it scored 0.04.
#[test]
fn a_grid_of_merged_cells_is_empty_by_the_grid_not_by_its_present_cells() {
    use crate::structure::spatial_table_detector::is_valid_table;
    assert!(!is_valid_table(&ragged(1)), "1 filled cell in 8 x 14 is empty");
    assert!(!is_valid_table(&ragged(4)), "4 filled of 112 is empty");
    // A full grid of 2 x 5 with `filled` cells holding text: exactly 60% empty
    // is on the keep side of the line, and one cell fewer is not.
    let full = |filled: usize| {
        let mut table = ragged(0);
        table.col_count = 5;
        table.rows.truncate(2);
        for (r, row) in table.rows.iter_mut().enumerate() {
            row.cells.clear();
            for c in 0..5 {
                let mut cell = ragged(0).rows[0].cells[0].clone();
                cell.colspan = 1;
                if r * 5 + c < filled {
                    cell.text = "a".to_string();
                }
                row.cells.push(cell);
            }
        }
        table
    };
    assert!(is_valid_table(&full(4)), "4 of 10 filled is 60% empty");
    assert!(!is_valid_table(&full(3)), "3 of 10 filled is 70% empty");
    // A title merged across every column over full rows is not empty.
    let mut titled = full(10);
    titled.rows[0].cells.truncate(1);
    titled.rows[0].cells[0].colspan = 5;
    assert!(is_valid_table(&titled));
}

/// A cancel check on a job flag, and the flag.
fn job_cancel() -> (std::sync::Arc<std::sync::atomic::AtomicBool>, crate::cancel::CancelCheck) {
    use std::sync::{atomic::AtomicBool, Arc};
    let job = Arc::new(AtomicBool::new(false));
    let check = crate::cancel::CancelCheck::from_flags(Arc::new(AtomicBool::new(false)), job.clone());
    (job, check)
}

/// Tables in the lattice the cancel proofs run on: ten thousand primitives
/// under the debug profile, a hundred thousand in release.
#[cfg(debug_assertions)]
const CANCEL_TABLES: usize = 1_250;
#[cfg(not(debug_assertions))]
const CANCEL_TABLES: usize = 12_500;

/// No Dart battery sees it: a cancel arrives from the host while a lane is
/// busy, and a battery only sees a job end; the proof is how soon.
#[test]
fn a_cancel_set_before_detection_ends_it_at_once() {
    use std::sync::atomic::Ordering;
    let (grid, spans) = lattice(CANCEL_TABLES);
    let config = TableDetectionConfig::default();

    let started = thread_cpu_secs();
    let found = detect_tables(&spans, &grid, &config, &crate::cancel::CancelCheck::none()).unwrap();
    let full = thread_cpu_secs() - started;
    assert_eq!(found.len(), CANCEL_TABLES);

    let (job, cancel) = job_cancel();
    job.store(true, Ordering::Relaxed);
    let started = thread_cpu_secs();
    let result = detect_tables(&spans, &grid, &config, &cancel);
    let stopped = thread_cpu_secs() - started;
    let message = result.expect_err("a cancelled detection is an error").to_string();
    eprintln!("cancel set first: ended in {stopped:.4} s, a whole run is {full:.4} s");
    assert!(message.contains("cancelled"), "{message}");
    // One clock tick on each reading: Windows charges CPU time a tick at a time.
    let tick = resolution_secs();
    assert!(
        stopped < full * 0.05 + tick,
        "a cancel set first took {stopped:.4} s against {full:.4} s for the whole run (a clock tick is {tick:.4} s)"
    );
}

/// No Dart battery sees it, as above. A cancel lands a quarter of the way
/// into a run, and the run must end soon after: the work between that poll
/// and the next, read as the difference of two runs cancelled one poll apart,
/// is one cluster, never a large share of the run.
#[test]
fn a_cancel_in_the_middle_of_detection_ends_it_within_a_cluster() {
    use crate::cancel::CancelCheck;
    let (grid, spans) = lattice(CANCEL_TABLES);
    let config = TableDetectionConfig::default();
    let run = |check: &CancelCheck| {
        let started = thread_cpu_secs();
        let result = detect_tables(&spans, &grid, &config, check);
        (result, thread_cpu_secs() - started)
    };

    let counting = CancelCheck::tripping_at(usize::MAX);
    let (found, full) = run(&counting);
    assert_eq!(found.unwrap().len(), CANCEL_TABLES);
    let quarter = counting.polls() / 4;
    assert!(quarter > 0, "the detection polls for a cancel");

    let (at, ended) = run(&CancelCheck::tripping_at(quarter));
    let (next, ended_next) = run(&CancelCheck::tripping_at(quarter + 1));
    assert!(at.is_err() && next.is_err(), "the run finished before the cancel landed: raise the lattice");
    let cluster = ended_next - ended;
    eprintln!("cancel a quarter in: ended after {ended:.4} s, one cluster is {cluster:.4} s, a whole run is {full:.4} s");
    // A difference of two readings is off by up to two ticks.
    let tick = resolution_secs();
    assert!(
        cluster < 0.25_f64.min(full * 0.1) + 2.0 * tick,
        "the work between two polls is {cluster:.4} s, a whole run is {full:.4} s (a clock tick is {tick:.4} s)"
    );
}

/// No Dart battery sees it: the host calls these entry points and a battery
/// cannot set the flag between its call and the engine's first poll.
#[test]
fn the_host_calls_stop_on_a_cancel_set_first() {
    use std::sync::atomic::Ordering;
    let doc = PdfDocument::from_bytes(sixty_tables()).unwrap();
    let (job, cancel) = job_cancel();
    job.store(true, Ordering::Relaxed);
    let options = crate::converters::ConversionOptions::default();
    for (name, result) in [
        ("extract_tables", doc.extract_tables_with_config_cancellable(0, TableDetectionConfig::default(), &cancel).map(|_| ())),
        ("to_markdown", doc.to_markdown_cancellable(0, &options, &cancel).map(|_| ())),
        ("to_html", doc.to_html_cancellable(0, &options, &cancel).map(|_| ())),
        ("extract_text", doc.extract_text_cancellable(0, &cancel).map(|_| ())),
    ] {
        let message = result.expect_err(name).to_string();
        assert!(message.contains("cancelled"), "{name}: {message}");
    }
}
