//! The horizontal extents a row's members claim, searchable by overlap.
//!
//! Row snapping admits a run to a row only when it overlaps no member
//! substantially, so asking "does any member share this run's space" must not
//! read every member of the row: on a page of one long baseline that read is
//! quadratic in the runs.
//!
//! Extents are kept by left edge within a width class (a class holds widths
//! that differ by less than a factor of two). Members of one class near a new
//! run are few, because members overlap little, so a probe reads each class's
//! members whose left edge lies within one class width of the run.

use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Total order over `f32` for a key: `total_cmp`.
#[derive(Clone, Copy, PartialEq)]
struct Key(f32);

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// The extents of one row's members.
#[derive(Default)]
pub(crate) struct RowSpace {
    classes: u64,
    by_left: BTreeMap<(u8, Key, usize), ()>,
}

impl RowSpace {
    const CLASSES: i32 = 64;
    /// Class 0 holds widths below 2^-15; class c holds [2^(c-16), 2^(c-15)).
    const BIAS: i32 = 16;

    fn class(width: f32) -> u8 {
        ((width.log2().floor() as i32) + Self::BIAS).clamp(0, Self::CLASSES - 1) as u8
    }

    /// A width no member of `class` reaches.
    fn bound(class: u8) -> f32 {
        if i32::from(class) == Self::CLASSES - 1 {
            f32::INFINITY
        } else {
            2f32.powi(i32::from(class) - Self::BIAS + 1)
        }
    }

    /// Records member `id`, whose extent `left..right` is finite and not empty.
    pub(crate) fn insert(&mut self, id: usize, left: f32, right: f32) {
        let class = Self::class(right - left);
        self.classes |= 1 << class;
        self.by_left.insert((class, Key(left), id), ());
    }

    /// True when `shares` holds for a member that could overlap `left..right`:
    /// one whose left edge is below `right` and, as its width is under its
    /// class bound, above `left` less that bound.
    pub(crate) fn any_sharing(
        &self,
        left: f32,
        right: f32,
        shares: impl Fn(usize) -> bool,
    ) -> bool {
        for class in 0..Self::CLASSES as u8 {
            if self.classes & (1 << class) == 0 {
                continue;
            }
            // A margin over the bound absorbs the rounding of `left + width`.
            let from = left - Self::bound(class) * 1.001 - 1e-3;
            let range = (class, Key(from), 0)..(class, Key(right), 0);
            if self.by_left.range(range).any(|(&(_, _, id), _)| shares(id)) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extents of every size and position, overlapping freely: the search
    /// finds a sharing member exactly when a scan of all members does.
    #[test]
    fn any_sharing_is_a_scan_of_every_member() {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for round in 0..200 {
            let mut space = RowSpace::default();
            let mut members: Vec<(f32, f32)> = Vec::new();
            for _ in 0..(5 + round % 40) {
                let left = (next() % 20_000) as f32 / 10.0 - 500.0;
                // Widths from a hair to the whole row, on a log scale.
                let width = 2f32.powf((next() % 2_200) as f32 / 100.0 - 13.0);
                space.insert(members.len(), left, left + width);
                members.push((left, left + width));
            }
            for _ in 0..50 {
                let left = (next() % 20_000) as f32 / 10.0 - 500.0;
                let width = 2f32.powf((next() % 2_200) as f32 / 100.0 - 13.0);
                let right = left + width;
                // The predicate a caller supplies: a substantial overlap.
                let shares = |m: usize| {
                    let (ml, mr) = members[m];
                    let overlap = right.min(mr) - left.max(ml);
                    let shorter = (right - left).min(mr - ml);
                    overlap > 2.0f32.max(shorter * 0.25)
                };
                let scan = (0..members.len()).any(shares);
                assert_eq!(space.any_sharing(left, right, shares), scan, "round {round}");
            }
        }
    }
}
