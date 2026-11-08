//! A stroke styled through an ExtGState is drawn as the same stroke styled
//! through the line operators.
//!
//! `/LW`, `/LC`, `/LJ`, `/ML` and `/D` of an ExtGState set the width, cap,
//! join, miter limit and dash that `w`, `J`, `j`, `M` and `d` set. Each proof
//! renders a page at 150 dpi twice, once styled by `gs` and once by the
//! operator, and compares every pixel; the unstyled page is rendered too, so
//! a pair that agrees only because neither style shows cannot pass.
//!
//! No Dart battery can observe this: the batteries see the pixels of one
//! page, and only the operator twin of the same page, written here, tells
//! which style the pixels should carry.

use super::band_proof::Fixture;
use crate::document::PdfDocument;
use crate::rendering::{render_page, RenderOptions};

/// A path with a sharp corner and a long end, thick enough that a cap, a
/// join, a miter limit or a dash changes its pixels.
const PATH: &str = "40 40 m 100 170 l 160 40 l";

fn render(pdf: Vec<u8>) -> Vec<u8> {
    let doc = PdfDocument::from_bytes(pdf).expect("fixture parses");
    render_page(&doc, 0, &RenderOptions::with_dpi(150)).expect("renders").data
}

/// A page whose ExtGState `/GS1` is `entries` and whose content is `content`.
fn gs_page(entries: &str, content: &str) -> Vec<u8> {
    let mut fixture = Fixture::new("ext_gstate_lines", content);
    fixture.resources = "/ExtGState << /GS1 << /Type /ExtGState ".to_string() + entries + " >> >>";
    fixture.pdf()
}

fn plain_page(content: &str) -> Vec<u8> {
    Fixture::new("ext_gstate_lines", content).pdf()
}

/// The page styled by `entries` through `gs`, the same page styled by
/// `operators`, and the page with neither.
fn prove(entries: &str, operators: &str, base: &str) {
    let via_gs = render(gs_page(entries, &format!("{base} /GS1 gs {PATH} S\n")));
    let via_operators = render(plain_page(&format!("{base} {operators} {PATH} S\n")));
    let unstyled = render(plain_page(&format!("{base} {PATH} S\n")));
    assert!(via_gs == via_operators, "{entries}: gs renders differently from `{operators}`");
    assert!(via_gs != unstyled, "{entries}: the style shows nowhere, the proof is empty");
}

/// `/LW 6` draws the stroke `6 w` draws. Not visible to a Dart battery: see the
/// module note, the operator twin is the oracle.
#[test]
fn gs_line_width_is_the_w_operator() {
    prove("/LW 6", "6 w", "0 0 0 RG");
}

/// `/LC 1` draws what `1 J` draws (round caps on a wide stroke). Not visible
/// to a Dart battery: see the module note.
#[test]
fn gs_line_cap_is_the_j_operator() {
    prove("/LC 1", "1 J", "0 0 0 RG 14 w");
}

/// `/LJ 1` draws what `1 j` draws (round join on a wide stroke). Not visible
/// to a Dart battery: see the module note.
#[test]
fn gs_line_join_is_the_lower_j_operator() {
    prove("/LJ 1", "1 j", "0 0 0 RG 14 w");
}

/// `/ML 2` draws what `2 M` draws (a sharp corner beveled below the default
/// limit of 10). Not visible to a Dart battery: see the module note.
#[test]
fn gs_miter_limit_is_the_m_operator() {
    prove("/ML 2", "2 M", "0 0 0 RG 14 w");
}

/// `/D [[6 3] 0]` draws what `[6 3] 0 d` draws. Not visible to a Dart battery:
/// see the module note.
#[test]
fn gs_dash_is_the_d_operator() {
    prove("/D [[6 3] 0]", "[6 3] 0 d", "0 0 0 RG 4 w");
}

/// A width set inside `q ... Q` through `gs` is gone after `Q`: the stroke
/// after it is 1 pt wide, as it is when `6 w` sets the width. Not visible to
/// a Dart battery: see the module note.
#[test]
fn gs_line_width_is_restored_by_q() {
    let later = "20 20 m 180 20 l S";
    let via_gs = render(gs_page("/LW 6", &format!("0 0 0 RG q /GS1 gs {PATH} S Q {later}\n")));
    let via_operator = render(plain_page(&format!("0 0 0 RG q 6 w {PATH} S Q {later}\n")));
    let leaked = render(plain_page(&format!("0 0 0 RG 6 w {PATH} S {later}\n")));
    assert!(via_gs == via_operator, "gs and `6 w` disagree around q/Q");
    assert!(via_gs != leaked, "the later stroke is as wide as the styled one");
}

/// An ExtGState given by reference, with its values given by reference, works
/// (ISO 32000-1 §7.3.10). Not visible to a Dart battery: see the module note.
#[test]
fn gs_line_values_by_reference() {
    let mut fixture = Fixture::new("ext_gstate_lines", &format!("0 0 0 RG /GS1 gs {PATH} S\n"));
    fixture.resources = "/ExtGState << /GS1 5 0 R >>".to_string();
    fixture.extra = vec![
        b"5 0 obj\n<< /Type /ExtGState /LW 6 0 R /LC 7 0 R /LJ 7 0 R /ML 8 0 R /D [9 0 R 10 0 R] >>\nendobj\n"
            .to_vec(),
        b"6 0 obj\n6\nendobj\n".to_vec(),
        b"7 0 obj\n1\nendobj\n".to_vec(),
        b"8 0 obj\n2\nendobj\n".to_vec(),
        b"9 0 obj\n[11 0 R 3]\nendobj\n".to_vec(),
        b"10 0 obj\n0\nendobj\n".to_vec(),
        b"11 0 obj\n6\nendobj\n".to_vec(),
    ];
    let via_gs = render(fixture.pdf());
    let via_operators =
        render(plain_page(&format!("0 0 0 RG 6 w 1 J 1 j 2 M [6 3] 0 d {PATH} S\n")));
    assert!(via_gs == via_operators, "references in the ExtGState were not followed");
}

/// `/LW -1` is taken as `-1 w` is taken, whatever that draws. Not visible to
/// a Dart battery: see the module note.
#[test]
fn gs_negative_line_width_is_what_w_does() {
    let via_gs = render(gs_page("/LW -1", &format!("0 0 0 RG 4 w /GS1 gs {PATH} S\n")));
    let via_operator = render(plain_page(&format!("0 0 0 RG 4 w -1 w {PATH} S\n")));
    assert!(via_gs == via_operator, "`/LW -1` and `-1 w` disagree");
}
