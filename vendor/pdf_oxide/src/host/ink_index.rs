//! The boxes of a page's drawn glyph runs, searchable by region.
//!
//! The span merge asks, for the gap between two runs, whether any run's box
//! lies in it. Asking every box for every gap is quadratic in the runs of a
//! page, so the boxes are kept in grids by size class: a box of size up to
//! 2^c lies in at most four cells of the grid whose cells are 2^c wide, and a
//! region is answered by the cells it covers in each class.

use crate::geometry::Rect;
use std::collections::HashMap;

/// Boxes that can be asked whether one of them satisfies a test inside a region.
pub(crate) trait InkBoxes {
    /// True when `pred` holds for a box that may intersect the region
    /// `x0..x1` by `y0..y1`; a box outside the region is never offered and
    /// `pred` alone decides every box that is.
    fn any_near(&self, x0: f32, x1: f32, y0: f32, y1: f32, pred: &dyn Fn(&Rect) -> bool) -> bool;
}

impl InkBoxes for [Rect] {
    fn any_near(&self, _: f32, _: f32, _: f32, _: f32, pred: &dyn Fn(&Rect) -> bool) -> bool {
        self.iter().any(pred)
    }
}

impl<const N: usize> InkBoxes for [Rect; N] {
    fn any_near(&self, _: f32, _: f32, _: f32, _: f32, pred: &dyn Fn(&Rect) -> bool) -> bool {
        self.iter().any(pred)
    }
}

type Cell = (i64, i64);

/// A grid index over boxes; see the module docs.
pub(crate) struct InkIndex {
    boxes: Vec<Rect>,
    /// Size classes: class c holds boxes of up to 2^c, in cells 2^c wide.
    classes: Vec<HashMap<Cell, Vec<u32>>>,
    /// Boxes with a coordinate that is not finite: offered to every region.
    loose: Vec<u32>,
}

impl InkIndex {
    const CLASSES: usize = 48;

    pub(crate) fn new(boxes: Vec<Rect>) -> Self {
        let mut classes: Vec<HashMap<Cell, Vec<u32>>> = vec![HashMap::new(); Self::CLASSES];
        let mut loose = Vec::new();
        for (i, b) in boxes.iter().enumerate() {
            let size = b.width.max(b.height);
            if !(b.x.is_finite()
                && b.y.is_finite()
                && b.width.is_finite()
                && b.height.is_finite()
                && size >= 0.0
                && b.width >= 0.0
                && b.height >= 0.0)
            {
                loose.push(i as u32);
                continue;
            }
            let class = Self::class_of(size);
            let cell = Self::cell_size(class);
            let (x0, x1) = (Self::at(b.x, cell), Self::at(b.x + b.width, cell));
            let (y0, y1) = (Self::at(b.y, cell), Self::at(b.y + b.height, cell));
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    classes[class].entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        Self { boxes, classes, loose }
    }

    /// The smallest class whose cells are at least `size` wide; boxes under
    /// one point share class 0.
    fn class_of(size: f32) -> usize {
        if size <= 1.0 {
            return 0;
        }
        (size.log2().ceil() as usize).min(Self::CLASSES - 1)
    }

    fn cell_size(class: usize) -> f32 {
        2f32.powi(class as i32)
    }

    fn at(v: f32, cell: f32) -> i64 {
        (v / cell).floor() as i64
    }
}

impl InkBoxes for InkIndex {
    fn any_near(&self, x0: f32, x1: f32, y0: f32, y1: f32, pred: &dyn Fn(&Rect) -> bool) -> bool {
        if self.loose.iter().any(|&i| pred(&self.boxes[i as usize])) {
            return true;
        }
        // A region that is not finite or not ordered is read as a scan of all
        // boxes would read it: every box is offered.
        let finite = x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite();
        for (class, grid) in self.classes.iter().enumerate() {
            if grid.is_empty() {
                continue;
            }
            if !finite {
                if grid.values().flatten().any(|&i| pred(&self.boxes[i as usize])) {
                    return true;
                }
                continue;
            }
            let cell = Self::cell_size(class);
            // A margin absorbs the rounding of the edges a test computes.
            let m = cell * 1e-3 + 1e-4;
            let (cx0, cx1) = (Self::at(x0.min(x1) - m, cell), Self::at(x0.max(x1) + m, cell));
            let (cy0, cy1) = (Self::at(y0.min(y1) - m, cell), Self::at(y0.max(y1) + m, cell));
            for cx in cx0..=cx1 {
                for cy in cy0..=cy1 {
                    if let Some(ids) = grid.get(&(cx, cy)) {
                        if ids.iter().any(|&i| pred(&self.boxes[i as usize])) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Boxes of every size, with a few that are not finite: for regions
    /// the way the merge builds them, the index agrees with a scan of every
    /// box on whether one satisfies the test.
    #[test]
    fn any_near_is_a_scan_of_every_box() {
        let mut state: u64 = 0xD1B5_4A32_D192_ED03;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for round in 0..100 {
            let mut boxes = Vec::new();
            for k in 0..(20 + round * 3) {
                let size = |n: u64| 2f32.powf((n % 1400) as f32 / 100.0 - 4.0);
                let mut b = Rect::new(
                    (next() % 40_000) as f32 / 40.0 - 200.0,
                    (next() % 40_000) as f32 / 40.0 - 200.0,
                    size(next()),
                    size(next()),
                );
                if k % 37 == 5 {
                    b.x = f32::NAN;
                }
                boxes.push(b);
            }
            let index = InkIndex::new(boxes.clone());
            for _ in 0..60 {
                let x0 = (next() % 40_000) as f32 / 40.0 - 200.0;
                let x1 = x0 + (next() % 3_000) as f32 / 100.0;
                let y0 = (next() % 40_000) as f32 / 40.0 - 200.0;
                let y1 = y0 + (next() % 2_000) as f32 / 100.0;
                // The test the merge applies: a box over the gap and the band.
                let pred = |b: &Rect| {
                    let overlap = (b.x + b.width).min(x1) - b.x.max(x0);
                    overlap > (x1 - x0) * 0.35 && b.y < y1 && b.y + b.height > y0
                };
                assert_eq!(
                    index.any_near(x0, x1, y0, y1, &pred),
                    boxes.iter().any(pred),
                    "round {round}"
                );
            }
        }
    }
}
