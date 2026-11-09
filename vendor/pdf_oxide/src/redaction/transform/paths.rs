//! The path walker of the redaction pass. A path's construction and clip
//! operators are held until its paint operator decides them: the path is
//! kept as it was, dropped, or cut to the complement of the regions. A cut
//! path is written as the same geometry in user space, its curves flattened
//! to polylines in page space, so a surviving curve is a polyline at the
//! box edge and nowhere else.

use crate::content::graphics_state::{GraphicsState, Matrix};
use crate::content::operators::Operator;
use crate::error::{Error, Result};
use crate::geometry::{Point, Rect};
use crate::redaction::classify::{classify, transform_bbox, Classification};
use crate::redaction::image_prune::invert_affine;
use crate::redaction::path_prune::clip_polygon_to_rect;
use crate::redaction::region::RegionSet;

/// Flattened curve segments are about this long in page units.
const CURVE_STEP: f32 = 2.0;
const CURVE_MIN_STEPS: usize = 4;
const CURVE_MAX_STEPS: usize = 512;

/// The operators that build a path or set its clip. They are held until the
/// paint operator that ends the path.
pub(super) fn is_path_op(op: &Operator) -> bool {
    matches!(
        op,
        Operator::MoveTo { .. }
            | Operator::LineTo { .. }
            | Operator::CurveTo { .. }
            | Operator::CurveToV { .. }
            | Operator::CurveToY { .. }
            | Operator::ClosePath
            | Operator::Rectangle { .. }
            | Operator::ClipNonZero
            | Operator::ClipEvenOdd
    )
}

/// What a paint operator does to the path it ends.
#[derive(Clone, Copy)]
pub(super) struct Paint {
    /// `Some(even_odd)` when the path is filled.
    fill: Option<bool>,
    /// The path is stroked.
    stroke: bool,
    /// The operator closes the current subpath first (`s`, `b`, `b*`).
    close: bool,
}

/// The paint a paint operator makes, or `None` when `op` is not one.
pub(super) fn paint_of(op: &Operator) -> Option<Paint> {
    let paint = |fill, stroke, close| Some(Paint { fill, stroke, close });
    match op {
        Operator::Fill => paint(Some(false), false, false),
        Operator::FillEvenOdd => paint(Some(true), false, false),
        Operator::Stroke => paint(None, true, false),
        Operator::CloseAndStroke => paint(None, true, true),
        Operator::CloseFillStroke => paint(Some(false), true, true),
        Operator::CloseFillStrokeEvenOdd => paint(Some(true), true, true),
        Operator::FillStroke => paint(Some(false), true, false),
        Operator::FillStrokeEvenOdd => paint(Some(true), true, false),
        Operator::EndPath => paint(None, false, false),
        _ => None,
    }
}

/// What the walker writes in place of a painted path.
pub(super) enum PathRedaction {
    /// The held operators and the paint go out as read.
    Keep,
    /// These operators go out instead; none of them is a source operator.
    Rewritten(Vec<Operator>),
}

enum Seg {
    Line(Point),
    Curve(Point, Point, Point),
}

struct Sub {
    start: Point,
    segs: Vec<Seg>,
    closed: bool,
}

/// The subpaths a path builds, in user space, and the rule of its clip.
struct Geometry {
    subs: Vec<Sub>,
    clip: Option<bool>,
}

fn unreadable() -> Error {
    Error::Unsupported(
        "a path operator follows no moveto; refusing to redact a path that cannot be read"
            .to_string(),
    )
}

/// The subpath a segment goes into: the last one when it is open, a new one
/// starting at `at` when it is closed.
fn open(subs: &mut Vec<Sub>, at: Point) -> Result<&mut Sub> {
    let reopen = match subs.last() {
        None => return Err(unreadable()),
        Some(last) => last.closed,
    };
    if reopen {
        subs.push(Sub {
            start: at,
            segs: Vec::new(),
            closed: false,
        });
    }
    subs.last_mut().ok_or_else(unreadable)
}

fn parse(ops: &[Operator]) -> Result<Geometry> {
    let mut subs: Vec<Sub> = Vec::new();
    let mut clip = None;
    let mut cur = Point::default();
    for op in ops {
        match *op {
            Operator::MoveTo { x, y } => {
                cur = Point::new(x, y);
                subs.push(Sub {
                    start: cur,
                    segs: Vec::new(),
                    closed: false,
                });
            },
            Operator::LineTo { x, y } => {
                let p = Point::new(x, y);
                open(&mut subs, cur)?.segs.push(Seg::Line(p));
                cur = p;
            },
            Operator::CurveTo { x1, y1, x2, y2, x3, y3 } => {
                let p = Point::new(x3, y3);
                let seg = Seg::Curve(Point::new(x1, y1), Point::new(x2, y2), p);
                open(&mut subs, cur)?.segs.push(seg);
                cur = p;
            },
            Operator::CurveToV { x2, y2, x3, y3 } => {
                let p = Point::new(x3, y3);
                let seg = Seg::Curve(cur, Point::new(x2, y2), p);
                open(&mut subs, cur)?.segs.push(seg);
                cur = p;
            },
            Operator::CurveToY { x1, y1, x3, y3 } => {
                let p = Point::new(x3, y3);
                let seg = Seg::Curve(Point::new(x1, y1), p, p);
                open(&mut subs, cur)?.segs.push(seg);
                cur = p;
            },
            Operator::ClosePath => {
                if let Some(last) = subs.last_mut() {
                    last.closed = true;
                    cur = last.start;
                }
            },
            Operator::Rectangle {
                x,
                y,
                width,
                height,
            } => {
                let corner = Point::new(x + width, y);
                let far = Point::new(x + width, y + height);
                let near = Point::new(x, y + height);
                subs.push(Sub {
                    start: Point::new(x, y),
                    segs: vec![Seg::Line(corner), Seg::Line(far), Seg::Line(near)],
                    closed: true,
                });
                cur = Point::new(x, y);
            },
            Operator::ClipNonZero => clip = Some(false),
            Operator::ClipEvenOdd => clip = Some(true),
            _ => {},
        }
    }
    Ok(Geometry { subs, clip })
}

/// Every point a subpath names, its control points included: the hull of a
/// curve lies inside the hull of its control points, so their box bounds it.
fn for_each_point(sub: &Sub, mut f: impl FnMut(Point)) {
    f(sub.start);
    for seg in &sub.segs {
        match *seg {
            Seg::Line(p) => f(p),
            Seg::Curve(a, b, c) => {
                f(a);
                f(b);
                f(c);
            },
        }
    }
}

/// The box of every point of `subs`; `None` when there is no point. A point
/// that is not finite is refused: no box can bound it.
fn bounds(subs: &[Sub]) -> Result<Option<Rect>> {
    let mut lo = Point::new(f32::INFINITY, f32::INFINITY);
    let mut hi = Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;
    let mut finite = true;
    for sub in subs {
        for_each_point(sub, |p| {
            finite &= p.x.is_finite() && p.y.is_finite();
            any = true;
            lo.x = lo.x.min(p.x);
            lo.y = lo.y.min(p.y);
            hi.x = hi.x.max(p.x);
            hi.y = hi.y.max(p.y);
        });
    }
    if !finite {
        return Err(Error::Unsupported(
            "a path has a point that is not finite; refusing to redact it".to_string(),
        ));
    }
    Ok(any.then(|| Rect::from_points(lo.x, lo.y, hi.x, hi.y)))
}

/// Half the widest a stroke reaches from its centre line, in user space.
/// A hairline is one device pixel wide, which is `1 / min_scale` user units.
/// A miter join reaches the miter limit times half its width, so the factor
/// applies only when the path has a join (`joined`): a single segment has none.
fn stroke_reach(gs: &GraphicsState, ctm: &Matrix, joined: bool) -> Result<f32> {
    let min_scale = (ctm.a * ctm.a + ctm.b * ctm.b)
        .sqrt()
        .min((ctm.c * ctm.c + ctm.d * ctm.d).sqrt());
    if !min_scale.is_finite() || min_scale <= 0.0 {
        return Err(Error::Unsupported(
            "a singular transform cannot bound a stroke; refusing to redact it".to_string(),
        ));
    }
    let width = gs.line_width.max(1.0 / min_scale);
    let miter = if gs.line_join == 0 && joined {
        gs.miter_limit.max(1.0)
    } else {
        1.0
    };
    Ok(width / 2.0 * miter)
}

fn bezier(p0: Point, c1: Point, c2: Point, p3: Point, t: f32) -> Point {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Point::new(
        a * p0.x + b * c1.x + c * c2.x + d * p3.x,
        a * p0.y + b * c1.y + c * c2.y + d * p3.y,
    )
}

fn distance(a: Point, b: Point) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// A subpath in page space as one polyline; each curve becomes the chords
/// of its flattening, its end point exact.
fn flatten(sub: &Sub, ctm: &Matrix) -> Vec<Point> {
    let to_page = |p: Point| ctm.transform_point(p.x, p.y);
    let mut out = vec![to_page(sub.start)];
    for seg in &sub.segs {
        match *seg {
            Seg::Line(p) => out.push(to_page(p)),
            Seg::Curve(c1, c2, p3) => {
                let Some(&p0) = out.last() else { continue };
                let (c1, c2, p3) = (to_page(c1), to_page(c2), to_page(p3));
                let len = distance(p0, c1) + distance(c1, c2) + distance(c2, p3);
                let steps = ((len / CURVE_STEP).ceil() as usize)
                    .clamp(CURVE_MIN_STEPS, CURVE_MAX_STEPS);
                for i in 1..steps {
                    out.push(bezier(p0, c1, c2, p3, i as f32 / steps as f32));
                }
                out.push(p3);
            },
        }
    }
    out
}

/// The rectangles of `bounds` outside `hole`: at most four, disjoint.
fn complement(bounds: &Rect, hole: &Rect) -> Vec<Rect> {
    if !bounds.intersects(hole) {
        return vec![*bounds];
    }
    let (bl, br, bt, bb) = (bounds.left(), bounds.right(), bounds.top(), bounds.bottom());
    let (middle_top, middle_bottom) = (hole.top().max(bt), hole.bottom().min(bb));
    let mut out = Vec::with_capacity(4);
    if hole.top() > bt {
        out.push(Rect::from_points(bl, bt, br, hole.top().min(bb)));
    }
    if hole.bottom() < bb {
        out.push(Rect::from_points(bl, hole.bottom().max(bt), br, bb));
    }
    if hole.left() > bl {
        out.push(Rect::from_points(bl, middle_top, hole.left().min(br), middle_bottom));
    }
    if hole.right() < br {
        out.push(Rect::from_points(hole.right().max(bl), middle_top, br, middle_bottom));
    }
    out
}

/// The pieces of `bounds` that no padded region covers, in page space.
fn pieces(bounds: Rect, regions: &RegionSet, padding: f32, grow: f32) -> Vec<Rect> {
    let mut out = vec![bounds];
    for region in &regions.regions {
        let padded = region.padded_rect(padding);
        let hole = Rect::from_points(
            padded.left() - grow,
            padded.top() - grow,
            padded.right() + grow,
            padded.bottom() + grow,
        );
        out = out.iter().flat_map(|piece| complement(piece, &hole)).collect();
    }
    out
}

/// The largest factor by which `ctm` stretches a length: a user-space
/// distance `d` is at most `d * max_scale` in page space.
fn max_scale(ctm: &Matrix) -> f32 {
    // The largest singular value of the linear part. The longer column alone
    // falls short under a shear, which would let a stroke's ink reach the box.
    let (a, b, c, d) = (f64::from(ctm.a), f64::from(ctm.b), f64::from(ctm.c), f64::from(ctm.d));
    let sum = a * a + b * b + c * c + d * d;
    let det = a * d - b * c;
    let root = (sum * sum - 4.0 * det * det).max(0.0).sqrt();
    ((sum + root) / 2.0).sqrt() as f32
}

/// Liang–Barsky: the part of the segment `a`–`b` inside `r`. An end the
/// segment keeps is returned as it was given, so a run of segments joins
/// exactly.
fn clip_segment(a: Point, b: Point, r: &Rect) -> Option<(Point, Point)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    let edges = [
        (-dx, a.x - r.left()),
        (dx, r.right() - a.x),
        (-dy, a.y - r.top()),
        (dy, r.bottom() - a.y),
    ];
    for (p, q) in edges {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else if p < 0.0 {
            let t = q / p;
            if t > t1 {
                return None;
            }
            t0 = t0.max(t);
        } else {
            let t = q / p;
            if t < t0 {
                return None;
            }
            t1 = t1.min(t);
        }
    }
    let start = if t0 == 0.0 {
        a
    } else {
        Point::new(a.x + t0 * dx, a.y + t0 * dy)
    };
    let end = if t1 == 1.0 {
        b
    } else {
        Point::new(a.x + t1 * dx, a.y + t1 * dy)
    };
    Some((start, end))
}

/// The open polylines of `pts` inside `r`. Segments that stay connected
/// stay one polyline; a segment that leaves starts a new one. A polyline is
/// never closed, so a cut edge is not stroked.
fn clip_polyline(pts: &[Point], r: &Rect) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    for w in pts.windows(2) {
        match clip_segment(w[0], w[1], r) {
            Some((p, q)) => {
                if cur.last() != Some(&p) {
                    if cur.len() >= 2 {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(p);
                }
                cur.push(q);
            },
            None => {
                if cur.len() >= 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
            },
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

fn clip_op(even_odd: bool) -> Operator {
    if even_odd {
        Operator::ClipEvenOdd
    } else {
        Operator::ClipNonZero
    }
}

/// A zero-area rectangle under the clip's own rule: the clip covers nothing,
/// so whatever the path would have painted inside the region is gone. A
/// path that is not clipped vanishes with no operators at all.
fn vanish(clip: Option<bool>) -> Vec<Operator> {
    match clip {
        None => Vec::new(),
        Some(even_odd) => vec![
            Operator::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            clip_op(even_odd),
            Operator::EndPath,
        ],
    }
}

/// Writes one subpath in user space: `pts` are page-space points, mapped
/// back through `inv`.
fn push_subpath(ops: &mut Vec<Operator>, pts: &[Point], close: bool, inv: &Matrix) {
    for (i, p) in pts.iter().enumerate() {
        let q = inv.transform_point(p.x, p.y);
        ops.push(if i == 0 {
            Operator::MoveTo { x: q.x, y: q.y }
        } else {
            Operator::LineTo { x: q.x, y: q.y }
        });
    }
    if close {
        ops.push(Operator::ClosePath);
    }
}

/// Cuts a straddling path to the complement of the regions. The fill is the
/// closed polygons of each subpath clipped to each piece; the stroke is the
/// open polylines. A clip is the polygons under the clip's own rule, and it
/// is applied before the stroke, so the stroke is clipped too.
fn cut(
    geo: &Geometry,
    paint: Paint,
    grown: &Rect,
    reach: f32,
    ctm: &Matrix,
    regions: &RegionSet,
    padding: f32,
) -> Result<Vec<Operator>> {
    let inv = invert_affine(ctm).ok_or_else(|| {
        Error::Unsupported("a singular transform cannot cut a path; refusing to redact it".into())
    })?;
    let bounds = transform_bbox(grown, ctm);
    let page: Vec<(Vec<Point>, bool)> = geo
        .subs
        .iter()
        .map(|sub| (flatten(sub, ctm), sub.closed))
        .collect();
    let needs_fill = paint.fill.is_some() || geo.clip.is_some();
    let mut fills: Vec<Vec<Point>> = Vec::new();
    if needs_fill {
        for piece in pieces(bounds, regions, padding, 0.0) {
            for (pts, _) in &page {
                let poly = clip_polygon_to_rect(pts, &piece);
                if poly.len() >= 3 {
                    fills.push(poly);
                }
            }
        }
    }
    // The ink of a kept centreline reaches `reach` past it, so the centreline
    // is cut at the holes grown by that reach: no ink lands in a region.
    let mut lines: Vec<Vec<Point>> = Vec::new();
    if paint.stroke {
        let page_reach = reach * max_scale(ctm);
        for piece in pieces(bounds, regions, padding, page_reach) {
            for (pts, closed) in &page {
                let mut path = pts.clone();
                if *closed {
                    if let Some(&first) = pts.first() {
                        path.push(first);
                    }
                }
                lines.extend(clip_polyline(&path, &piece));
            }
        }
    }
    let mut ops = Vec::new();
    if needs_fill {
        if fills.is_empty() {
            ops.extend(vanish(geo.clip));
        } else {
            for poly in &fills {
                push_subpath(&mut ops, poly, true, &inv);
            }
            if let Some(even_odd) = geo.clip {
                ops.push(clip_op(even_odd));
            }
            ops.push(match paint.fill {
                Some(false) => Operator::Fill,
                Some(true) => Operator::FillEvenOdd,
                None => Operator::EndPath,
            });
        }
    }
    if paint.stroke && !lines.is_empty() {
        for line in &lines {
            push_subpath(&mut ops, line, false, &inv);
        }
        ops.push(Operator::Stroke);
    }
    Ok(ops)
}

/// Decides one painted path: `pending` are the operators that built it,
/// `paint` the operator that ends it, `gs` the state it is painted in.
pub(super) fn redact_path(
    pending: &[Operator],
    paint: &Operator,
    gs: &GraphicsState,
    ctm: &Matrix,
    regions: &RegionSet,
    padding: f32,
) -> Result<PathRedaction> {
    let Some(paint) = paint_of(paint) else {
        return Ok(PathRedaction::Keep);
    };
    let mut geo = parse(pending)?;
    if paint.close {
        if let Some(last) = geo.subs.last_mut() {
            last.closed = true;
        }
    }
    let Some(user) = bounds(&geo.subs)? else {
        return Ok(PathRedaction::Keep);
    };
    let joined = geo.subs.iter().any(|sub| sub.segs.len() >= 2);
    let reach = if paint.stroke {
        stroke_reach(gs, ctm, joined)?
    } else {
        0.0
    };
    let grown = Rect::from_points(
        user.left() - reach,
        user.top() - reach,
        user.right() + reach,
        user.bottom() + reach,
    );
    Ok(match classify(&grown, ctm, regions, padding) {
        Classification::Outside => PathRedaction::Keep,
        Classification::Inside => PathRedaction::Rewritten(vanish(geo.clip)),
        Classification::Straddle => {
            PathRedaction::Rewritten(cut(&geo, paint, &grown, reach, ctm, regions, padding)?)
        },
    })
}
