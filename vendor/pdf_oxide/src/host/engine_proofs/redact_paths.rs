//! Path redaction at the operator level: a painted path inside a region is
//! gone, one outside is written as it was, and one that straddles is cut to
//! the complement of the region, its rule and its clip kept. The page's
//! pixels are not drawn here, so these tests read the operators back: a
//! battery over a saved file cannot see a single point of a path that the
//! pass cut.

use crate::content::operators::Operator;
use crate::error::Result;
use crate::geometry::Rect;
use crate::host::content::{Mode, Operators};
use crate::redaction::text_engine::FontMetrics;
use crate::redaction::transform::{ImageResources, LineStyle, RedactPass};
use crate::redaction::{RedactionOptions, RedactionRegion, RedactionReport, RegionSet};

/// Tolerance for a point that lies on the cut, after the writer's six decimals.
const EPS: f32 = 1e-3;

/// No glyph is shown in these tests, so no font metric is read.
struct NoFont;

impl FontMetrics for NoFont {
    fn width(&self, _font: &str, _code: u32) -> f32 {
        0.0
    }
}

fn ops_of(bytes: &[u8]) -> Vec<Operator> {
    Operators::new(bytes, Mode::Full)
        .map(|item| item.expect("the writer's output parses"))
        .collect()
}

fn redact(content: &[u8], x0: f32, y0: f32, x1: f32, y1: f32) -> Result<(Vec<u8>, RedactionReport)> {
    redact_with(content, x0, y0, x1, y1, &ImageResources::none())
}

fn redact_with(
    content: &[u8],
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    images: &ImageResources,
) -> Result<(Vec<u8>, RedactionReport)> {
    let mut set = RegionSet::new(0);
    set.push(RedactionRegion::from_rect(x0, y0, x1, y1, Some([0.0, 0.0, 0.0])));
    let opts = RedactionOptions::default();
    let mut out = Vec::new();
    let outcome = RedactPass::new(&set, &opts, &NoFont, images)
        .run(Operators::new(content, Mode::Full), &mut out)?;
    Ok((out, outcome.report))
}

/// The region's padded box: what the pass removes under, and the box the
/// operators must keep out of.
fn hole(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    RedactionRegion::from_rect(x0, y0, x1, y1, None).padded_rect(RedactionOptions::default().edge_padding)
}

/// The page's own operators: those between the outer `q` and its `Q`. The
/// overlay follows the `Q`, so it is left out.
fn page_ops(out: &[u8]) -> Vec<Operator> {
    ops_of(out)
        .into_iter()
        .skip(1)
        .take_while(|op| !matches!(op, Operator::RestoreState))
        .collect()
}

/// Every point a path operator names, control points included.
fn points(ops: &[Operator]) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    for op in ops {
        match *op {
            Operator::MoveTo { x, y } | Operator::LineTo { x, y } => out.push((x, y)),
            Operator::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                out.extend([(x1, y1), (x2, y2), (x3, y3)]);
            },
            Operator::CurveToV { x2, y2, x3, y3 } => out.extend([(x2, y2), (x3, y3)]),
            Operator::CurveToY { x1, y1, x3, y3 } => out.extend([(x1, y1), (x3, y3)]),
            _ => {},
        }
    }
    out
}

fn strictly_inside(p: (f32, f32), hole: &Rect) -> bool {
    p.0 > hole.left() + EPS
        && p.0 < hole.right() - EPS
        && p.1 > hole.top() + EPS
        && p.1 < hole.bottom() - EPS
}

fn any_inside(ops: &[Operator], hole: &Rect) -> bool {
    points(ops).into_iter().any(|p| strictly_inside(p, hole))
}

/// A filled square fully under the region is removed, and nothing of it is
/// written: the proof that a path under a region leaves no point behind.
#[test]
fn filled_path_inside_region_is_removed() {
    let (out, report) = redact(b"10 10 m 20 10 l 20 20 l h f\n", 0.0, 0.0, 100.0, 100.0).unwrap();
    assert_eq!(report.paths_pruned, 1);
    assert!(!ops_of(&out).iter().any(|op| matches!(op, Operator::MoveTo { .. })));
}

/// A path outside every region is written as the source wrote it: the
/// proof that the pass leaves untouched geometry alone.
#[test]
fn filled_path_outside_region_is_written_unchanged() {
    let content = b"10 10 m 20 10 l 20 20 l h f\n";
    let (out, report) = redact(content, 500.0, 500.0, 600.0, 600.0).unwrap();
    assert_eq!(report.paths_pruned, 0);
    let source: Vec<Operator> = ops_of(content)
        .into_iter()
        .filter(|op| !matches!(op, Operator::SaveState | Operator::RestoreState))
        .collect();
    let written: Vec<Operator> = page_ops(&out);
    assert_eq!(written, source);
}

/// A filled rectangle straddling the region keeps only its outside: no
/// point of the cut path is under the region, and the fill itself stays.
#[test]
fn straddling_fill_keeps_only_the_outside() {
    let content = b"0 0 m 100 0 l 100 10 l 0 10 l h f\n";
    let (out, report) = redact(content, 50.0, -10.0, 150.0, 20.0).unwrap();
    let ops = page_ops(&out);
    assert_eq!(report.paths_pruned, 1);
    assert!(!any_inside(&ops, &hole(50.0, -10.0, 150.0, 20.0)));
    assert!(ops.iter().any(|op| matches!(op, Operator::Fill)));
    assert!(points(&ops).iter().any(|p| p.0 < 50.0));
}

/// A thick stroke straddling the region is cut into open polylines and never
/// closed, so the cut edge is not stroked on the box.
#[test]
fn straddling_stroke_is_cut_into_open_lines() {
    let content = b"4 w 0 50 m 100 50 l S\n";
    let (out, report) = redact(content, 40.0, 40.0, 60.0, 60.0).unwrap();
    let ops = page_ops(&out);
    assert_eq!(report.paths_pruned, 1);
    assert!(!any_inside(&ops, &hole(40.0, 40.0, 60.0, 60.0)));
    assert!(ops.iter().any(|op| matches!(op, Operator::Stroke)));
    assert!(!ops.iter().any(|op| matches!(op, Operator::ClosePath)));
    let xs: Vec<f32> = points(&ops).into_iter().map(|p| p.0).collect();
    assert!(xs.iter().any(|x| *x < 40.0) && xs.iter().any(|x| *x > 60.0));
}

/// A stroke whose width reaches into a region that its centre line misses is
/// cut: the line width is part of the bound, never the centre line alone.
#[test]
fn stroke_reaching_a_region_by_its_width_is_cut() {
    let content = b"40 w 0 50 m 100 50 l S\n";
    let (_, report) = redact(content, 0.0, 66.0, 100.0, 100.0).unwrap();
    assert_eq!(report.paths_pruned, 1);
}

/// A curve straddling the region comes out as polylines with no curve
/// operator left, so the flattening is the only change at the box.
#[test]
fn straddling_curve_is_flattened_outside_the_box() {
    let content = b"0 0 m 0 100 100 100 100 0 c f\n";
    let (out, report) = redact(content, 40.0, -10.0, 60.0, 60.0).unwrap();
    let ops = page_ops(&out);
    assert_eq!(report.paths_pruned, 1);
    assert!(!ops.iter().any(|op| matches!(op, Operator::CurveTo { .. })));
    assert!(!any_inside(&ops, &hole(40.0, -10.0, 60.0, 60.0)));
}

/// An even-odd donut straddling the region keeps its even-odd rule, so the
/// hole of the donut is still a hole in what remains.
#[test]
fn even_odd_donut_straddling_keeps_its_rule() {
    let content = b"0 0 100 100 re 30 30 40 40 re f*\n";
    let (out, report) = redact(content, 90.0, 40.0, 120.0, 60.0).unwrap();
    let ops = page_ops(&out);
    assert_eq!(report.paths_pruned, 1);
    assert!(ops.iter().any(|op| matches!(op, Operator::FillEvenOdd)));
    assert!(!any_inside(&ops, &hole(90.0, 40.0, 120.0, 60.0)));
}

/// A clip inside the region becomes a zero-area clip under its own rule, so
/// nothing painted after it shows inside the region.
#[test]
fn clip_inside_region_becomes_empty() {
    let content = b"10 10 20 20 re W n\n0 0 200 200 re f\n";
    let (out, report) = redact(content, 0.0, 0.0, 100.0, 100.0).unwrap();
    let ops = page_ops(&out);
    assert!(report.paths_pruned >= 1);
    assert!(ops.contains(&Operator::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }));
    assert!(ops.contains(&Operator::ClipNonZero));
    assert!(!any_inside(&ops, &hole(0.0, 0.0, 100.0, 100.0)));
}

/// A clip straddling the region is cut like a fill, so the clip itself
/// carries no area under the region.
#[test]
fn clip_straddling_region_is_cut() {
    let content = b"0 0 200 200 re W n\n0 0 10 10 re f\n";
    let (out, report) = redact(content, 50.0, 50.0, 150.0, 150.0).unwrap();
    let ops = page_ops(&out);
    assert_eq!(report.paths_pruned, 1);
    assert!(ops.contains(&Operator::ClipNonZero));
    assert!(!any_inside(&ops, &hole(50.0, 50.0, 150.0, 150.0)));
}

/// The stroke's reach is the width its ExtGState sets: a `gs` with `/LW 4`
/// cuts a single straight line at the padded edge less half that width, so
/// its ink stops where the box's padding plus two points begins.
#[test]
fn ext_gstate_line_width_sets_the_stroke_reach() {
    let mut images = ImageResources::none();
    images.line_styles.insert(
        "GS1".to_string(),
        Some(LineStyle {
            lw: Some(4.0),
            lj: None,
            ml: None,
            lc: None,
        }),
    );
    let content = b"/GS1 gs 0 161 m 300 161 l S\n";
    let (out, report) = redact_with(content, 100.0, 140.0, 200.0, 160.0, &images).unwrap();
    assert_eq!(report.paths_pruned, 1);
    let left = points(&page_ops(&out))
        .into_iter()
        .map(|p| p.0)
        .filter(|x| *x < 100.0)
        .fold(f32::MIN, f32::max);
    let padded = RedactionRegion::from_rect(100.0, 140.0, 200.0, 160.0, None)
        .padded_rect(RedactionOptions::default().edge_padding)
        .left();
    assert!(
        (left - (padded - 2.0)).abs() < 0.01,
        "the kept line ends at {left}, not {}",
        padded - 2.0
    );
}

/// A `gs` whose ExtGState cannot be read, or is not in the page's resources,
/// refuses the redaction: the line style it sets is unknown.
#[test]
fn unreadable_ext_gstate_refuses() {
    let mut images = ImageResources::none();
    images.line_styles.insert("GS1".to_string(), None);
    assert!(redact_with(b"/GS1 gs 0 0 m 1 1 l S\n", 0.0, 0.0, 5.0, 5.0, &images).is_err());
    assert!(redact(b"/GS2 gs 0 0 m 1 1 l S\n", 0.0, 0.0, 5.0, 5.0).is_err());
}

/// A path the content never paints is refused, not passed through: a path
/// that cannot be read cannot be shown to be outside the region.
#[test]
fn path_without_a_paint_operator_is_refused() {
    assert!(redact(b"0 0 m 10 10 l\n", 0.0, 0.0, 5.0, 5.0).is_err());
}
