//! A page's text spans on a grid of buckets, so a table asks for the spans
//! near it and never reads the page's other spans.
//!
//! Table detection works one table, or one cluster of rulings, at a time. A
//! pass that reads every span for each of them takes time in the product of
//! the two, which a page of ten thousand small ruled boxes and its text turns
//! into minutes.

use crate::geometry::Rect;
use crate::layout::TextSpan;
use std::collections::HashMap;

/// Side of a bucket, in points.
const SIDE: f32 = 64.0;
/// A span that reaches more buckets than this is kept in a list every query reads.
const WIDEST: i64 = 1024;

/// The spans of a page by the buckets their boxes reach.
pub(crate) struct SpanIndex<'a> {
    spans: &'a [TextSpan],
    buckets: HashMap<(i32, i32), Vec<u32>>,
    wide: Vec<u32>,
}

fn bucket(v: f32) -> i32 {
    (v / SIDE).floor().clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

impl<'a> SpanIndex<'a> {
    /// Indexes `spans`.
    pub(crate) fn new(spans: &'a [TextSpan]) -> Self {
        let mut buckets: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let mut wide = Vec::new();
        for (i, span) in spans.iter().enumerate() {
            let b = span.bbox;
            let finite = b.x.is_finite() && b.y.is_finite() && b.width.is_finite() && b.height.is_finite();
            if !finite {
                wide.push(i as u32);
                continue;
            }
            let (x0, x1) = (bucket(b.left().min(b.right())), bucket(b.left().max(b.right())));
            let (y0, y1) = (bucket(b.top().min(b.bottom())), bucket(b.top().max(b.bottom())));
            if (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1) > WIDEST {
                wide.push(i as u32);
                continue;
            }
            for bx in x0..=x1 {
                for by in y0..=y1 {
                    buckets.entry((bx, by)).or_default().push(i as u32);
                }
            }
        }
        Self { spans, buckets, wide }
    }

    /// The spans whose boxes meet `region`, in the order of the page.
    pub(crate) fn near(&self, region: Rect) -> Vec<TextSpan> {
        let mut found: Vec<u32> = self.wide.clone();
        let finite = region.x.is_finite() && region.y.is_finite() && region.width.is_finite() && region.height.is_finite();
        let (x0, x1) = (bucket(region.left().min(region.right())), bucket(region.left().max(region.right())));
        let (y0, y1) = (bucket(region.top().min(region.bottom())), bucket(region.top().max(region.bottom())));
        let reach = (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1);
        if !finite || reach > self.buckets.len() as i64 {
            // Fewer buckets are held than the region reaches: read them all.
            for list in self.buckets.values() {
                found.extend_from_slice(list);
            }
        } else {
            for bx in x0..=x1 {
                for by in y0..=y1 {
                    if let Some(list) = self.buckets.get(&(bx, by)) {
                        found.extend_from_slice(list);
                    }
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
            .into_iter()
            .map(|i| &self.spans[i as usize])
            .filter(|s| region.intersects(&s.bbox))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_at(x: f32, y: f32, w: f32) -> TextSpan {
        TextSpan { text: "w".to_string(), bbox: Rect::new(x, y, w, 8.0), ..Default::default() }
    }

    /// The spans come back in the order of the page, each once, and only those that meet the region.
    #[test]
    fn near_returns_the_spans_that_meet_the_region_in_page_order() {
        let spans = vec![span_at(10.0, 10.0, 5.0), span_at(500.0, 500.0, 5.0), span_at(60.0, 10.0, 80.0), span_at(12.0, 12.0, 5.0)];
        let index = SpanIndex::new(&spans);
        let near = index.near(Rect::new(0.0, 0.0, 100.0, 40.0));
        let xs: Vec<f32> = near.iter().map(|s| s.bbox.x).collect();
        assert_eq!(xs, vec![10.0, 60.0, 12.0]);
        assert!(index.near(Rect::new(1000.0, 1000.0, 5.0, 5.0)).is_empty());
    }

    /// A span wider than the widest bucket run is still found.
    #[test]
    fn a_page_wide_span_is_found_from_any_region() {
        let spans = vec![span_at(0.0, 10.0, 90_000.0)];
        let index = SpanIndex::new(&spans);
        assert_eq!(index.near(Rect::new(80_000.0, 5.0, 10.0, 10.0)).len(), 1);
    }
}
