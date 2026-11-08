// Copyright 2011 Google Inc.
// Copyright 2020 Yevhenii Reizner
//
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use alloc::vec::Vec;

use tiny_skia_path::PathVerb;

use crate::{Path, Point};
// ── pdf_manipulator patch: edges are clipped on the sides only ──
use crate::Rect;
// ── end pdf_manipulator patch ──

use crate::edge::{CubicEdge, Edge, LineEdge, QuadraticEdge};
use crate::edge_clipper::EdgeClipperIter;
use crate::geom::ScreenIntRect;
use crate::path_geometry;

#[derive(Copy, Clone, PartialEq, Debug)]
enum Combine {
    No,
    Partial,
    Total,
}

#[derive(Copy, Clone, Debug)]
pub struct ShiftedIntRect {
    shifted: ScreenIntRect,
    shift: i32,
}

impl ShiftedIntRect {
    pub fn new(rect: &ScreenIntRect, shift: i32) -> Option<Self> {
        let shifted = ScreenIntRect::from_xywh(
            rect.x() << shift,
            rect.y() << shift,
            rect.width() << shift,
            rect.height() << shift,
        )?;
        Some(ShiftedIntRect { shifted, shift })
    }

    pub fn shifted(&self) -> &ScreenIntRect {
        &self.shifted
    }

    pub fn recover(&self) -> ScreenIntRect {
        ScreenIntRect::from_xywh(
            self.shifted.x() >> self.shift,
            self.shifted.y() >> self.shift,
            self.shifted.width() >> self.shift,
            self.shifted.height() >> self.shift,
        )
        .unwrap() // cannot fail, because the original rect was valid
    }
}

pub struct BasicEdgeBuilder {
    edges: Vec<Edge>,
    clip_shift: i32,
    // ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
    /// The window's origin in whole pixels; the path is in the device plane.
    origin: (i32, i32),
    // ── end pdf_manipulator patch ──
}

impl BasicEdgeBuilder {
    // ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
    pub fn new(clip_shift: i32, origin: (i32, i32)) -> Self {
        BasicEdgeBuilder {
            edges: Vec::with_capacity(64), // TODO: stack array + fallback
            clip_shift,
            origin,
        }
    }
    // ── end pdf_manipulator patch ──

    // Skia returns a linked list here, but it's a nightmare to use in Rust,
    // so we're mimicking it with Vec.
    pub fn build_edges(
        path: &Path,
        clip: Option<&ShiftedIntRect>,
        clip_shift: i32,
        // ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
        origin: (i32, i32),
        // ── end pdf_manipulator patch ──
    ) -> Option<Vec<Edge>> {
        // If we're convex, then we need both edges, even if the right edge is past the clip.
        // let can_cull_to_the_right = !path.isConvex();
        let can_cull_to_the_right = false; // TODO: this

        let mut builder = BasicEdgeBuilder::new(clip_shift, origin);
        if !builder.build(path, clip, can_cull_to_the_right) {
            log::warn!("infinite or NaN segments detected during edges building");
            return None;
        }

        if builder.edges.len() < 2 {
            return None;
        }

        Some(builder.edges)
    }

    // TODO: build_poly
    pub fn build(
        &mut self,
        path: &Path,
        clip: Option<&ShiftedIntRect>,
        can_cull_to_the_right: bool,
    ) -> bool {
        if let Some(clip) = clip {
            let clip = clip.recover().to_rect();
            // ── pdf_manipulator patch: edges are clipped on the sides only ──
            // A chop at the top or bottom re-creates the edge from a rounded
            // intersection, which a canvas that holds the whole edge never
            // does. The scan converter starts the edge at the first visible
            // row instead (`Edge::advance_to`). Only an edge that runs more
            // rows past the window than the fixed point can carry is chopped
            // (`rows_of_reach`).
            let reach = crate::scan::path::rows_of_reach(clip.height() as u32) as f32;
            // The clip is the window's columns in the device plane, and the
            // rows it can carry.
            let clip = Rect::from_ltrb(
                clip.left() + self.origin.0 as f32,
                clip.top() + self.origin.1 as f32 - reach,
                clip.right() + self.origin.0 as f32,
                clip.bottom() + self.origin.1 as f32 + reach,
            )
            .unwrap_or(clip);
            // ── end pdf_manipulator patch ──
            for edges in EdgeClipperIter::new(path, clip, can_cull_to_the_right) {
                for edge in edges {
                    match edge {
                        PathEdge::LineTo(p0, p1) => {
                            if !p0.is_finite() || !p1.is_finite() {
                                return false;
                            }

                            self.push_line(&[p0, p1])
                        }
                        PathEdge::QuadTo(p0, p1, p2) => {
                            if !p0.is_finite() || !p1.is_finite() || !p2.is_finite() {
                                return false;
                            }

                            self.push_quad(&[p0, p1, p2])
                        }
                        PathEdge::CubicTo(p0, p1, p2, p3) => {
                            if !p0.is_finite()
                                || !p1.is_finite()
                                || !p2.is_finite()
                                || !p3.is_finite()
                            {
                                return false;
                            }

                            self.push_cubic(&[p0, p1, p2, p3])
                        }
                    }
                }
            }
        } else {
            for edge in edge_iter(path) {
                match edge {
                    PathEdge::LineTo(p0, p1) => {
                        self.push_line(&[p0, p1]);
                    }
                    PathEdge::QuadTo(p0, p1, p2) => {
                        let points = [p0, p1, p2];
                        let mut mono_x = [Point::zero(); 5];
                        let n = path_geometry::chop_quad_at_y_extrema(&points, &mut mono_x);
                        for i in 0..=n {
                            self.push_quad(&mono_x[i * 2..]);
                        }
                    }
                    PathEdge::CubicTo(p0, p1, p2, p3) => {
                        let points = [p0, p1, p2, p3];
                        let mut mono_y = [Point::zero(); 10];
                        let n = path_geometry::chop_cubic_at_y_extrema(&points, &mut mono_y);
                        for i in 0..=n {
                            self.push_cubic(&mono_y[i * 3..]);
                        }
                    }
                }
            }
        }

        true
    }

    // ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
    /// The origin in the edges' fixed-point units (`1 << (clip_shift + 6)` a pixel).
    fn fixed_origin(&self) -> (i32, i32) {
        let unit = 1i64 << (self.clip_shift + 6);
        let scaled = |v: i32| (i64::from(v) * unit).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        (scaled(self.origin.0), scaled(self.origin.1))
    }
    // ── end pdf_manipulator patch ──

    fn push_line(&mut self, points: &[Point; 2]) {
        // ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
        if let Some(edge) = LineEdge::new(points[0], points[1], self.clip_shift, self.fixed_origin())
        {
            // ── end pdf_manipulator patch ──
            let combine = if edge.is_vertical() && !self.edges.is_empty() {
                if let Some(Edge::Line(last)) = self.edges.last_mut() {
                    combine_vertical(&edge, last)
                } else {
                    Combine::No
                }
            } else {
                Combine::No
            };

            match combine {
                Combine::Total => {
                    self.edges.pop();
                }
                Combine::Partial => {}
                Combine::No => self.edges.push(Edge::Line(edge)),
            }
        }
    }

    fn push_quad(&mut self, points: &[Point]) {
        // ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
        if let Some(edge) = QuadraticEdge::new(points, self.clip_shift, self.fixed_origin()) {
            // ── end pdf_manipulator patch ──
            self.edges.push(Edge::Quadratic(edge));
        }
    }

    fn push_cubic(&mut self, points: &[Point]) {
        // ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
        if let Some(edge) = CubicEdge::new(points, self.clip_shift, self.fixed_origin()) {
            // ── end pdf_manipulator patch ──
            self.edges.push(Edge::Cubic(edge));
        }
    }
}

fn combine_vertical(edge: &LineEdge, last: &mut LineEdge) -> Combine {
    if last.dx != 0 || edge.x != last.x {
        return Combine::No;
    }

    if edge.winding == last.winding {
        return if edge.last_y + 1 == last.first_y {
            last.first_y = edge.first_y;
            Combine::Partial
        } else if edge.first_y == last.last_y + 1 {
            last.last_y = edge.last_y;
            Combine::Partial
        } else {
            Combine::No
        };
    }

    if edge.first_y == last.first_y {
        return if edge.last_y == last.last_y {
            Combine::Total
        } else if edge.last_y < last.last_y {
            last.first_y = edge.last_y + 1;
            Combine::Partial
        } else {
            last.first_y = last.last_y + 1;
            last.last_y = edge.last_y;
            last.winding = edge.winding;
            Combine::Partial
        };
    }

    if edge.last_y == last.last_y {
        if edge.first_y > last.first_y {
            last.last_y = edge.first_y - 1;
        } else {
            last.last_y = last.first_y - 1;
            last.first_y = edge.first_y;
            last.winding = edge.winding;
        }

        return Combine::Partial;
    }

    Combine::No
}

pub fn edge_iter(path: &Path) -> PathEdgeIter<'_> {
    PathEdgeIter {
        path,
        verb_index: 0,
        points_index: 0,
        move_to: Point::zero(),
        needs_close_line: false,
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PathEdge {
    LineTo(Point, Point),
    QuadTo(Point, Point, Point),
    CubicTo(Point, Point, Point, Point),
}

/// Lightweight variant of PathIter that only returns segments (e.g. lines/quads).
///
/// Does not return Move or Close. Always "auto-closes" each contour.
pub struct PathEdgeIter<'a> {
    path: &'a Path,
    verb_index: usize,
    points_index: usize,
    move_to: Point,
    needs_close_line: bool,
}

impl PathEdgeIter<'_> {
    fn close_line(&mut self) -> Option<PathEdge> {
        self.needs_close_line = false;

        let edge = PathEdge::LineTo(self.path.points()[self.points_index - 1], self.move_to);
        Some(edge)
    }
}

impl Iterator for PathEdgeIter<'_> {
    type Item = PathEdge;

    fn next(&mut self) -> Option<Self::Item> {
        if self.verb_index < self.path.verbs().len() {
            let verb = self.path.verbs()[self.verb_index];
            self.verb_index += 1;

            match verb {
                PathVerb::Move => {
                    if self.needs_close_line {
                        let res = self.close_line();
                        self.move_to = self.path.points()[self.points_index];
                        self.points_index += 1;
                        return res;
                    }

                    self.move_to = self.path.points()[self.points_index];
                    self.points_index += 1;
                    self.next()
                }
                PathVerb::Close => {
                    if self.needs_close_line {
                        return self.close_line();
                    }

                    self.next()
                }
                _ => {
                    // Actual edge.
                    self.needs_close_line = true;

                    let edge;
                    match verb {
                        PathVerb::Line => {
                            edge = PathEdge::LineTo(
                                self.path.points()[self.points_index - 1],
                                self.path.points()[self.points_index + 0],
                            );
                            self.points_index += 1;
                        }
                        PathVerb::Quad => {
                            edge = PathEdge::QuadTo(
                                self.path.points()[self.points_index - 1],
                                self.path.points()[self.points_index + 0],
                                self.path.points()[self.points_index + 1],
                            );
                            self.points_index += 2;
                        }
                        PathVerb::Cubic => {
                            edge = PathEdge::CubicTo(
                                self.path.points()[self.points_index - 1],
                                self.path.points()[self.points_index + 0],
                                self.path.points()[self.points_index + 1],
                                self.path.points()[self.points_index + 2],
                            );
                            self.points_index += 3;
                        }
                        _ => unreachable!(),
                    };

                    Some(edge)
                }
            }
        } else if self.needs_close_line {
            self.close_line()
        } else {
            None
        }
    }
}
