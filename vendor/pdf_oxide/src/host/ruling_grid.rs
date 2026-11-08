//! A page's ruling lines on a grid of half-point cells.
//!
//! Table detection reads a page's rulings, never its other paths. Each path
//! that could be a ruling (`PathContent::is_table_primitive`) is quantised as
//! it is painted and dropped: its ruling becomes a run of cells in a row or a
//! column, horizontal runs within the join tolerance merge as they are added
//! (the detector joins them anyway), vertical runs merge only where one holds
//! the other or they overlap by more than the detector's section clearance (a
//! break between two that meet shows where a table's sections end), and the
//! box it paints becomes occupied cells. What the grid
//! holds grows with the cells the rulings touch, never with how many paths
//! drew them.
//!
//! The quantum is a quarter of the smallest tolerance the detector decides
//! with (`LINE_AXIS_TOL`, 2 pt). The cells decide which rulings join and
//! cluster; each run also keeps the points its rulings were drawn at, so the
//! detector reads the coordinates as drawn and a decision that sits on a
//! tolerance is made on the same numbers as without a grid.
//!
//! Rulings that lie near one another form a cluster. The relation is the one
//! the detector has always applied to the boxes the rulings paint: two boxes
//! are in one cluster when the gap between them is at most 3 pt along x and
//! under 6 pt along y. (The sweep that found clusters stopped looking at 3 pt
//! along x, though it widened each box by 3 pt on every side; the widening
//! decides only along y.) Clusters are components of the occupied cells
//! under that relation, found on runs, not on a bitmap of the page.

use crate::elements::PathContent;
use crate::geometry::Rect;
use std::collections::BTreeMap;

/// Size of a cell, in points.
pub(crate) const QUANTUM: f32 = 0.5;
/// Gap along a ruling within which collinear rulings are one (the detector's `JOIN_TOL`).
pub(crate) const JOIN_TOL: f32 = 3.0;
/// Axis tolerance of a line (the detector's `LINE_AXIS_TOL`).
pub(crate) const LINE_AXIS_TOL: f32 = 2.0;
/// How far a vertical ruling must reach past a rule, on both sides, to cross it
/// when the detector looks for a table's section dividers (`SNAP_TOL` + 1 pt).
const SECTION_CLEARANCE: f32 = 4.0;
/// Gap along x within which two painted boxes are one cluster.
const X_LINK: f32 = 3.0;
/// Gap along y under which two painted boxes are one cluster.
const Y_LINK: f32 = 6.0;

/// Cells in a point length, rounded.
fn cells(pt: f32) -> u32 {
    (pt / QUANTUM).round() as u32
}

/// The cells `a..=b` of a row or column, and the points they were drawn at.
/// The cells decide which rulings join; the points are what the detector
/// reads, so that a coordinate is never moved by the quantum.
#[derive(Clone, Copy, Debug)]
struct Run {
    a: u32,
    b: u32,
    /// Where the ruling starts and ends along itself, in points.
    lo: f32,
    hi: f32,
    /// Where the ruling lies across itself, in points: that of one ruling in the run.
    coord: f32,
}

impl Run {
    /// A run over cells only, for what the detector never reads in points.
    fn cells(a: u32, b: u32) -> Self {
        Self { a, b, lo: 0.0, hi: 0.0, coord: 0.0 }
    }

    /// The ruling the run holds.
    fn line(&self, kind: LineKind) -> RunLine {
        RunLine { kind, coord: self.coord, start: self.lo, end: self.hi }
    }
}

/// Runs by row (for a horizontal ruling) or column (vertical): sorted by
/// start, and merged as `Join` says.
type Runs = BTreeMap<u32, Vec<Run>>;

/// How a new run joins the runs it meets.
#[derive(Clone, Copy)]
enum Join {
    /// Runs within this many cells of one another are one run; the runs of a
    /// row are disjoint.
    Within(u32),
    /// Runs whose rulings lie within this many points of one another, measured
    /// as drawn, are one run: the join of the detector's own `JOIN_TOL`.
    Gap(f32),
    /// Two runs are one when one holds the other or they share at least this
    /// many cells. Collinear rulings that meet, or overlap by less, stay
    /// apart, so the break between them can be seen: the detector tells a
    /// table's sections apart by the vertical rulings that stop at a rule.
    Overlapping(u32),
}

/// Adds `run` to `key`, merging the runs it joins. False when the run list
/// could not grow.
fn insert_run(runs: &mut Runs, key: u32, run: Run, join: Join) -> bool {
    let list = runs.entry(key).or_default();
    let mut merged = run;
    fn absorb(into: &mut Run, other: Run, first: bool) {
        into.a = into.a.min(other.a);
        into.b = into.b.max(other.b);
        into.lo = into.lo.min(other.lo);
        into.hi = into.hi.max(other.hi);
        if first {
            into.coord = other.coord;
        }
    }
    match join {
        Join::Within(reach) => {
            // Runs are disjoint, so their ends ascend with their starts.
            let first = list.partition_point(|r| r.b.saturating_add(reach) < run.a);
            let mut last = first;
            while last < list.len() && list[last].a <= run.b.saturating_add(reach) {
                absorb(&mut merged, list[last], last == first);
                last += 1;
            }
            if last > first {
                list.drain(first..last);
            }
        },
        Join::Gap(tol) => {
            // The cells only find the neighbours; the points decide. Runs lie in
            // order along the row, so their cells ascend. The merged run may
            // reach runs it did not: repeat to a fixpoint.
            let slack = cells(tol).saturating_add(1);
            loop {
                let first = list.partition_point(|r| r.b.saturating_add(slack) < merged.a);
                let found = list[first..]
                    .iter()
                    .take_while(|r| r.a <= merged.b.saturating_add(slack))
                    .position(|r| r.lo <= merged.hi + tol && merged.lo <= r.hi + tol);
                let Some(at) = found else { break };
                let other = list.remove(first + at);
                absorb(&mut merged, other, false);
            }
        },
        Join::Overlapping(min) => {
            // No run holds another, so starts and ends ascend together. The
            // merged run may now meet runs it did not before: repeat to a fixpoint.
            loop {
                let (lo, hi) = (merged.a, merged.b);
                let first = list.partition_point(|r| r.b <= lo);
                let found = list[first..].iter().take_while(|r| r.a <= hi).position(|r| {
                    let held = (r.a >= lo && r.b <= hi) || (r.a <= lo && r.b >= hi);
                    held || r.b.min(hi) - r.a.max(lo) >= min
                });
                let Some(at) = found else { break };
                let other = list.remove(first + at);
                absorb(&mut merged, other, false);
            }
        },
    }
    if list.try_reserve(1).is_err() {
        return false;
    }
    let at = list.partition_point(|r| r.a < merged.a);
    list.insert(at, merged);
    true
}

/// How a ruling lies on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineKind {
    /// A horizontal line (`PathContent::is_horizontal_line`).
    Horizontal,
    /// A vertical line (`PathContent::is_vertical_line`).
    Vertical,
    /// A horizontal side of a rectangle.
    RectHorizontal,
    /// A vertical side of a rectangle.
    RectVertical,
}

/// A merged ruling: for a horizontal one `coord` is y and `start..end` is the
/// x range; for a vertical one `coord` is x and `start..end` the y range.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RunLine {
    pub(crate) kind: LineKind,
    pub(crate) coord: f32,
    pub(crate) start: f32,
    pub(crate) end: f32,
}

impl RunLine {
    /// True for a ruling along x.
    pub(crate) fn is_horizontal(&self) -> bool {
        matches!(self.kind, LineKind::Horizontal | LineKind::RectHorizontal)
    }

    /// The box the ruling covers; a ruling has no thickness here.
    pub(crate) fn rect(&self) -> Rect {
        if self.is_horizontal() {
            Rect::new(self.start, self.coord, self.end - self.start, 0.0)
        } else {
            Rect::new(self.coord, self.start, 0.0, self.end - self.start)
        }
    }
}

/// Rulings that lie near one another, and the box they cover.
pub(crate) struct GridCluster {
    pub(crate) bbox: Rect,
    pub(crate) lines: Vec<RunLine>,
}

/// A page's rulings; see the module docs.
pub(crate) struct RulingGrid {
    origin_x: f32,
    origin_y: f32,
    cols: u32,
    rows: u32,
    lines_h: Runs,
    lines_v: Runs,
    sides_h: Runs,
    sides_v: Runs,
    /// Cells the painted boxes of all rulings cover, as runs by row.
    occupied: Runs,
    primitives: usize,
    has_rectangle: bool,
    vertical_extent: Option<(f32, f32)>,
    refused: bool,
}

impl RulingGrid {
    /// A grid over `bounds`; rulings outside it land on its edge.
    pub(crate) fn new(bounds: Rect) -> Self {
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        let (x, y) = (finite(bounds.x), finite(bounds.y));
        let (w, h) = (finite(bounds.width).max(0.0), finite(bounds.height).max(0.0));
        Self {
            origin_x: x,
            origin_y: y,
            cols: cells(w).saturating_add(1),
            rows: cells(h).saturating_add(1),
            lines_h: Runs::new(),
            lines_v: Runs::new(),
            sides_h: Runs::new(),
            sides_v: Runs::new(),
            occupied: Runs::new(),
            primitives: 0,
            has_rectangle: false,
            vertical_extent: None,
            refused: false,
        }
    }

    /// A grid over the rulings among `paths`, bounded by where they lie.
    pub(crate) fn from_paths(paths: &[PathContent]) -> Self {
        let mut bounds: Option<Rect> = None;
        for path in paths.iter().filter(|p| p.is_table_primitive()) {
            let r = path.rendered_bbox();
            bounds = Some(bounds.map_or(r, |b| b.union(&r)));
        }
        let mut grid = Self::new(bounds.unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0)));
        for path in paths {
            grid.add(path);
        }
        grid
    }

    fn col(&self, x: f32) -> u32 {
        let c = ((x - self.origin_x) / QUANTUM).round();
        if c.is_nan() {
            0
        } else {
            c.clamp(0.0, (self.cols - 1) as f32) as u32
        }
    }

    fn row(&self, y: f32) -> u32 {
        let c = ((y - self.origin_y) / QUANTUM).round();
        if c.is_nan() {
            0
        } else {
            c.clamp(0.0, (self.rows - 1) as f32) as u32
        }
    }

    fn x_of(&self, col: u32) -> f32 {
        self.origin_x + col as f32 * QUANTUM
    }

    fn y_of(&self, row: u32) -> f32 {
        self.origin_y + row as f32 * QUANTUM
    }

    /// Takes in one path: a path that cannot be a ruling is dropped, any other
    /// is counted, its painted box occupied, and its ruling added.
    pub(crate) fn add(&mut self, path: &PathContent) {
        if self.refused || !path.is_table_primitive() {
            return;
        }
        self.primitives += 1;
        let rendered = path.rendered_bbox();
        if path.is_vertical_line(LINE_AXIS_TOL) {
            let (lo, hi) = (rendered.y, rendered.y + rendered.height);
            self.vertical_extent = Some(match self.vertical_extent {
                Some((a, b)) => (a.min(lo), b.max(hi)),
                None => (lo, hi),
            });
        }

        let (r0, r1) = (self.row(rendered.top()), self.row(rendered.bottom()));
        let (c0, c1) = (self.col(rendered.left()), self.col(rendered.right()));
        let mut ok = true;
        for row in r0..=r1 {
            ok &= insert_run(&mut self.occupied, row, Run::cells(c0, c1), Join::Within(1));
        }

        // Horizontal rulings are joined as the detector joins them; vertical
        // ones are not, since the break between two that meet is what shows
        // where a table's sections end.
        let join = Join::Gap(JOIN_TOL);
        let stacked = Join::Overlapping(cells(2.0 * SECTION_CLEARANCE));
        if path.is_horizontal_line(LINE_AXIS_TOL) {
            let y = rendered.center().y;
            let run = Run { a: c0, b: c1, lo: rendered.left(), hi: rendered.right(), coord: y };
            let key = self.row(y);
            ok &= insert_run(&mut self.lines_h, key, run, join);
        } else if path.is_vertical_line(LINE_AXIS_TOL) {
            let x = rendered.center().x;
            let run = Run { a: r0, b: r1, lo: rendered.top(), hi: rendered.bottom(), coord: x };
            let key = self.col(x);
            ok &= insert_run(&mut self.lines_v, key, run, stacked);
        } else if path.is_rectangle() {
            self.has_rectangle = true;
            let b = path.bbox;
            let (bc0, bc1) = (self.col(b.left()), self.col(b.right()));
            let (br0, br1) = (self.row(b.top()), self.row(b.bottom()));
            for coord in [b.top(), b.bottom()] {
                let run = Run { a: bc0, b: bc1, lo: b.left(), hi: b.right(), coord };
                let key = self.row(coord);
                ok &= insert_run(&mut self.sides_h, key, run, join);
            }
            for coord in [b.left(), b.right()] {
                let run = Run { a: br0, b: br1, lo: b.top(), hi: b.bottom(), coord };
                let key = self.col(coord);
                ok &= insert_run(&mut self.sides_v, key, run, stacked);
            }
        }
        self.refused = !ok;
    }

    /// Paths that could be rulings, counted as they were added.
    pub(crate) fn primitives(&self) -> usize {
        self.primitives
    }

    /// True when a rectangle that is no line was added.
    pub(crate) fn has_rectangle(&self) -> bool {
        self.has_rectangle
    }

    /// The lowest and highest y the vertical lines paint, from the paths themselves.
    pub(crate) fn vertical_extent(&self) -> Option<(f32, f32)> {
        self.vertical_extent
    }

    /// True when a run list could not grow and later paths were dropped.
    pub(crate) fn refused(&self) -> bool {
        self.refused
    }

    fn lines_of<'a>(&'a self, runs: &'a Runs, kind: LineKind) -> impl Iterator<Item = RunLine> + 'a {
        runs.values().flat_map(move |list| list.iter().map(move |run| run.line(kind)))
    }

    /// Every ruling of the page, lines and rectangle sides alike.
    pub(crate) fn all_lines(&self) -> impl Iterator<Item = RunLine> + '_ {
        self.lines_of(&self.lines_h, LineKind::Horizontal)
            .chain(self.lines_of(&self.lines_v, LineKind::Vertical))
            .chain(self.lines_of(&self.sides_h, LineKind::RectHorizontal))
            .chain(self.lines_of(&self.sides_v, LineKind::RectVertical))
    }

    /// The rulings of horizontal lines only.
    pub(crate) fn horizontal_lines(&self) -> impl Iterator<Item = RunLine> + '_ {
        self.lines_of(&self.lines_h, LineKind::Horizontal)
    }

    /// The rulings of vertical lines only.
    pub(crate) fn vertical_lines(&self) -> impl Iterator<Item = RunLine> + '_ {
        self.lines_of(&self.lines_v, LineKind::Vertical)
    }

    /// The clusters of rulings, top of the page first, then left to right.
    pub(crate) fn clusters(
        &self,
        cancel: &crate::cancel::CancelCheck,
    ) -> crate::error::Result<Vec<GridCluster>> {
        let mut stride = crate::cancel::Stride::new(cancel);
        // Occupied runs in row order; `index` finds the runs of a row.
        let mut runs: Vec<(u32, u32, u32)> = Vec::new();
        let mut index: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
        for (&row, list) in &self.occupied {
            index.insert(row, (runs.len(), runs.len() + list.len()));
            runs.extend(list.iter().map(|r| (row, r.a, r.b)));
        }
        let x_link = cells(X_LINK);
        // Boxes under 6 pt apart: fewer than 12 cells between their nearest rows.
        let y_link = cells(Y_LINK) - 1;

        let mut parent: Vec<usize> = (0..runs.len()).collect();
        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for i in 0..runs.len() {
            stride.tick()?;
            let (row, a, b) = runs[i];
            for next in row..=row.saturating_add(y_link) {
                let Some(&(s, e)) = index.get(&next) else { continue };
                let list = &runs[s..e];
                // First run that reaches within `x_link` of this one's start.
                let mut j = list.partition_point(|r| r.2.saturating_add(x_link) < a);
                while j < list.len() && list[j].1 <= b.saturating_add(x_link) {
                    let other = s + j;
                    if other != i {
                        let (ri, rj) = (find(&mut parent, i), find(&mut parent, other));
                        if ri != rj {
                            parent[ri] = rj;
                        }
                    }
                    j += 1;
                }
            }
        }

        // Components, with the cells they cover.
        let mut extent: BTreeMap<usize, (u32, u32, u32, u32)> = BTreeMap::new();
        for i in 0..runs.len() {
            stride.tick()?;
            let root = find(&mut parent, i);
            let (row, a, b) = runs[i];
            let e = extent.entry(root).or_insert((row, row, a, b));
            e.0 = e.0.min(row);
            e.1 = e.1.max(row);
            e.2 = e.2.min(a);
            e.3 = e.3.max(b);
        }
        let mut roots: Vec<usize> = extent.keys().copied().collect();
        // Top of the page first (the larger y), then left to right.
        roots.sort_by(|a, b| {
            let (ea, eb) = (extent[a], extent[b]);
            eb.1.cmp(&ea.1).then(ea.2.cmp(&eb.2))
        });
        let mut slot: BTreeMap<usize, usize> = BTreeMap::new();
        let mut clusters: Vec<GridCluster> = Vec::with_capacity(roots.len());
        for (n, root) in roots.iter().enumerate() {
            slot.insert(*root, n);
            let (r0, r1, c0, c1) = extent[root];
            clusters.push(GridCluster {
                bbox: Rect::new(
                    self.x_of(c0),
                    self.y_of(r0),
                    self.x_of(c1) - self.x_of(c0),
                    self.y_of(r1) - self.y_of(r0),
                ),
                lines: Vec::new(),
            });
        }

        // Each ruling joins the cluster that covers its first cell.
        let mut label = |row: u32, col: u32| -> Option<usize> {
            let &(s, e) = index.get(&row)?;
            let list = &runs[s..e];
            let j = list.partition_point(|r| r.2 < col);
            (j < list.len() && list[j].1 <= col).then(|| find(&mut parent, s + j))
        };
        let mut place = |clusters: &mut Vec<GridCluster>,
                         runs_of: &Runs,
                         kind: LineKind|
         -> crate::error::Result<()> {
            let horizontal = matches!(kind, LineKind::Horizontal | LineKind::RectHorizontal);
            for (&key, list) in runs_of {
                for run in list {
                    stride.tick()?;
                    let (row, col) = if horizontal { (key, run.a) } else { (run.a, key) };
                    let Some(root) = label(row, col) else { continue };
                    if let Some(&n) = slot.get(&root) {
                        clusters[n].lines.push(run.line(kind));
                    }
                }
            }
            Ok(())
        };
        place(&mut clusters, &self.lines_h, LineKind::Horizontal)?;
        place(&mut clusters, &self.lines_v, LineKind::Vertical)?;
        place(&mut clusters, &self.sides_h, LineKind::RectHorizontal)?;
        place(&mut clusters, &self.sides_v, LineKind::RectVertical)?;
        Ok(clusters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(x: f32, y: f32, w: f32) -> PathContent {
        PathContent::line(x, y, x + w, y)
    }

    fn v(x: f32, y: f32, hgt: f32) -> PathContent {
        PathContent::line(x, y, x, y + hgt)
    }

    fn grid(paths: &[PathContent]) -> RulingGrid {
        RulingGrid::from_paths(paths)
    }

    /// The quantum is a quarter of the smallest tolerance the detector decides with.
    #[test]
    fn the_quantum_is_a_quarter_of_the_smallest_tolerance() {
        use crate::structure::spatial_table_detector::{JOIN_TOL as J, SNAP_TOL as S};
        assert_eq!(QUANTUM, S.min(LINE_AXIS_TOL).min(J) / 4.0);
        assert_eq!(JOIN_TOL, J);
    }

    /// Two collinear rulings within the join tolerance are one run; a wider gap is two.
    #[test]
    fn collinear_rulings_within_the_join_tolerance_merge() {
        let g = grid(&[h(0.0, 10.0, 30.0), h(32.5, 10.0, 30.0), h(100.0, 10.0, 30.0)]);
        let lines: Vec<RunLine> = g.horizontal_lines().collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!((lines[0].start, lines[0].end), (0.0, 62.5));
        // The gap is measured as drawn: 3 pt joins, a hair more does not.
        let g = grid(&[h(0.0, 10.0, 30.0), h(33.0, 10.0, 30.0)]);
        assert_eq!(g.horizontal_lines().count(), 1);
        let g = grid(&[h(0.0, 10.0, 30.0), h(33.25, 10.0, 30.0)]);
        assert_eq!(g.horizontal_lines().count(), 2);
    }

    /// The clearance is the detector's: a ruling crosses a rule only past `SNAP_TOL` + 1 pt.
    #[test]
    fn the_section_clearance_is_the_detectors() {
        use crate::structure::spatial_table_detector::SNAP_TOL;
        assert_eq!(SECTION_CLEARANCE, SNAP_TOL + 1.0);
    }

    /// Vertical rulings that meet end to end, or overlap by less than twice the
    /// clearance, stay two; ones that overlap by more, or lie inside another, are one.
    #[test]
    fn vertical_rulings_that_meet_stay_apart() {
        let g = grid(&[v(10.0, 0.0, 30.0), v(10.0, 30.0, 30.0)]);
        assert_eq!(g.vertical_lines().count(), 2);
        let g = grid(&[v(10.0, 0.0, 30.0), v(10.0, 24.0, 30.0)]);
        assert_eq!(g.vertical_lines().count(), 2);
        let g = grid(&[v(10.0, 0.0, 30.0), v(10.0, 20.0, 30.0)]);
        assert_eq!(g.vertical_lines().count(), 1);
        let g = grid(&[v(10.0, 0.0, 30.0), v(10.0, 5.0, 4.0)]);
        assert_eq!(g.vertical_lines().count(), 1, "a short ruling inside a long one");
        let g = grid(&[v(10.0, 0.0, 30.0), v(10.0, 0.0, 30.0)]);
        assert_eq!(g.vertical_lines().count(), 1, "a ruling painted twice is one");
    }

    /// The cluster relation is the old sweep's: 3 pt along x inclusive, under 6 pt along y.
    #[test]
    fn clusters_link_at_three_points_along_x_and_under_six_along_y() {
        let clusters = |dx: f32, dy: f32| {
            grid(&[h(0.0, 0.0, 30.0), h(30.0 + dx, dy, 30.0)])
                .clusters(&crate::cancel::CancelCheck::none())
                .unwrap()
                .len()
        };
        assert_eq!(clusters(2.9, 0.0), 1);
        assert_eq!(clusters(3.0, 0.0), 1);
        assert_eq!(clusters(3.5, 0.0), 2);
        // A stroked line is 1 pt thick, so its box ends 0.5 pt above and below.
        assert_eq!(clusters(0.0, 6.0), 1);
        assert_eq!(clusters(0.0, 6.5), 1);
        assert_eq!(clusters(0.0, 7.5), 2);
    }

    /// Rulings far from the page's edge land where they are; clusters come top first.
    #[test]
    fn clusters_come_top_of_the_page_first() {
        let g = grid(&[h(10.0, 100.0, 50.0), h(10.0, 500.0, 50.0)]);
        let clusters = g.clusters(&crate::cancel::CancelCheck::none()).unwrap();
        assert_eq!(clusters.len(), 2);
        assert!(clusters[0].bbox.y > clusters[1].bbox.y);
        assert_eq!(clusters[0].lines.len(), 1);
    }

    /// Paths that cannot be rulings are dropped and not counted.
    #[test]
    fn a_path_that_is_no_ruling_is_dropped() {
        let g = grid(&[PathContent::rect(0.0, 0.0, 3.0, 3.0), h(0.0, 0.0, 50.0)]);
        assert_eq!(g.primitives(), 1);
    }
}
