//! The two span passes that look for a run drawn twice give the spans they
//! always gave. Each was rewritten to keep its places in flat vectors found by
//! hash and to pack the spans it keeps in place; each is compared here with the
//! pass it replaced, copied unchanged, on pages built to stress it.

use super::own_process::alone;
use crate::extractors::TextExtractor;
use crate::geometry::Rect;
use crate::host::heap;
use crate::layout::TextSpan;

const DEDUP_OVERLAP_RATIO: f32 = 0.30;
const DEDUP_OVERLAP_CAP_PT: f32 = 2.0;

/// The stroke and fill pass as it was before: a map of strings to maps of
/// cells to lists of rectangles, and a second list for the spans it keeps.
#[allow(clippy::all)]
    fn reference_stroke_fill(spans_in: &mut Vec<TextSpan>) {
        let spans_taken = std::mem::take(spans_in);
        use std::collections::HashMap;

        let spans = spans_taken;
        if spans.len() < 2 {
            *spans_in = spans;
            return;
        }
        let old_len = spans.len();
        // Bucket each text key into a coarse (cx,cy) grid instead of one flat
        // Vec (O(k²) when a short label repeats N times). The IoU ≥ 0.7 test is
        // unchanged: a partner with that overlap is within ≈0.176·width, so it
        // falls in this cell or a neighbour — querying the 3×3 neighbourhood
        // finds every match the full scan would. Only runs for text ≥ 2 chars
        // (shorter candidates rely on downstream positional dedup).
        const CELL: f32 = 16.0;
        type Grid = HashMap<(i32, i32), Vec<crate::geometry::Rect>>;
        let mut seen: HashMap<String, Grid> = HashMap::new();
        let mut kept: Vec<TextSpan> = Vec::with_capacity(old_len);
        let mut skipped = 0usize;
        for span in spans {
            let trimmed = span.text.trim();
            if trimmed.chars().count() < 2 {
                kept.push(span);
                continue;
            }
            let key = trimmed.to_ascii_lowercase();
            let b = span.bbox;
            let cx = ((b.x + b.width * 0.5) / CELL).floor() as i32;
            let cy = ((b.y + b.height * 0.5) / CELL).floor() as i32;
            let mut is_dup = false;
            if let Some(grid) = seen.get(&key) {
                // Saturating bounds: a span with an extreme/out-of-page bbox can
                // push cx/cy to the i32 limits, where `cx + 1` would overflow in
                // an overflow-checked build (observed on 1008.3918v2.pdf).
                'outer: for gx in cx.saturating_sub(1)..=cx.saturating_add(1) {
                    for gy in cy.saturating_sub(1)..=cy.saturating_add(1) {
                        let Some(others) = grid.get(&(gx, gy)) else {
                            continue;
                        };
                        for other in others {
                            // IoU — intersection over union. >= 0.7 means the
                            // two bboxes are almost the same rectangle, which is
                            // what stroke+fill produces.
                            let ix1 = b.x.max(other.x);
                            let iy1 = b.y.max(other.y);
                            let ix2 = (b.x + b.width).min(other.x + other.width);
                            let iy2 = (b.y + b.height).min(other.y + other.height);
                            if ix2 <= ix1 || iy2 <= iy1 {
                                continue;
                            }
                            let inter = (ix2 - ix1) * (iy2 - iy1);
                            let area_a = b.width * b.height;
                            let area_b = other.width * other.height;
                            let union = area_a + area_b - inter;
                            if union > 0.0 && inter / union >= 0.7 {
                                is_dup = true;
                                break 'outer;
                            }
                        }
                    }
                }
            }
            if is_dup {
                skipped += 1;
            } else {
                seen.entry(key)
                    .or_default()
                    .entry((cx, cy))
                    .or_default()
                    .push(b);
                kept.push(span);
            }
        }
        if skipped > 0 {
            log::debug!("Stroke+fill dedup: dropped {skipped} duplicate spans of {old_len}");
        }
        *spans_in = kept;
    }


/// The positional pass as it was before: a map of strings to lists of
/// positions, a clone of the previous text, and a second list.
#[allow(clippy::all)]
fn reference_positional(spans_in: &mut Vec<TextSpan>) {
        // Take ownership of spans to avoid cloning during iteration
        let old_len = spans_in.len();
        let spans = std::mem::take(spans_in);
        let mut deduplicated = Vec::with_capacity(old_len);
        let mut prev_y_rounded: Option<i32> = None;
        let mut prev_x: Option<f32> = None;
        let mut prev_text: Option<String> = None;
        // Every place a given string has been kept, not merely the last one.
        //
        // One slot per string cannot see an overprint. A page that draws each
        // glyph twice — a grey pass and a black pass a fraction of a point
        // apart, the usual way of faking a bold — yields single-glyph spans, so
        // a title like `SICHERHEITS` stores `S` at its first position and then
        // overwrites it with the `S` at the end of the same word. The second
        // pass then compares its `S` against the wrong one and finds no
        // duplicate.
        const MAX_TRACKED_POSITIONS: usize = 32;
        let mut seen_content: std::collections::HashMap<String, Vec<(f32, f32)>> =
            std::collections::HashMap::new();

        let mut geometric_skips = 0;
        let mut content_skips = 0;

        for span in spans {
            let y_rounded = span.bbox.y.round() as i32;
            let x = span.bbox.x;

            // PHASE 1: Geometric deduplication — require BOTH position AND text match
            let geometric_duplicate = if let (Some(prev_y), Some(prev_x_val), Some(ref prev_txt)) =
                (prev_y_rounded, prev_x, &prev_text)
            {
                // Threshold scales with the span's per-glyph advance so that
                // single-glyph narrow spans (`l`, `r`, `I`) are never wrongly
                // treated as overlapping with their legitimate neighbour.
                let char_count = span.text.chars().count().max(1) as f32;
                let per_glyph_width = (span.bbox.width / char_count).max(0.1);
                let threshold =
                    (per_glyph_width * DEDUP_OVERLAP_RATIO).min(DEDUP_OVERLAP_CAP_PT);
                y_rounded == prev_y && (x - prev_x_val).abs() < threshold && span.text == *prev_txt
            } else {
                false
            };

            // PHASE 2: Content-based deduplication — require positions to OVERLAP
            //
            // Short runs are included. They were excluded because a five-byte
            // floor is a cheap stand-in for "this string is distinctive enough
            // that repeating it at one position means a duplicate", but it also
            // exempts every overprinted glyph, and PHASE 1 cannot cover them:
            // it compares against the immediately preceding span only, and the
            // sort that decides adjacency uses the same integer-rounded row key
            // that the overprint's sub-point offset straddles, so the two
            // copies are never neighbours — a page's whole grey pass is emitted
            // before its black pass begins.
            //
            // What replaces the floor for a short run is a tighter bar on
            // position: the same per-glyph advance PHASE 1 uses, so two
            // legitimate repeats of one letter — the `ll` in `callibrator`, an
            // `SS` — stand a full advance apart and are never confused with a
            // pair that differs by a fraction of one.
            let content_duplicate = {
                let char_count = span.text.chars().count().max(1) as f32;
                let per_glyph_width = (span.bbox.width / char_count).max(0.1);
                let x_bar = if span.text.len() >= 5 {
                    5.0
                } else {
                    (per_glyph_width * DEDUP_OVERLAP_RATIO).min(DEDUP_OVERLAP_CAP_PT)
                };
                seen_content.get(&span.text).is_some_and(|seen| {
                    seen.iter().any(|(prev_x_val, prev_y_val)| {
                        (span.bbox.y - prev_y_val).abs() < 2.0
                            && (span.bbox.x - prev_x_val).abs() < x_bar
                    })
                })
            };

            if geometric_duplicate {
                geometric_skips += 1;
            } else if content_duplicate {
                content_skips += 1;
            } else {
                prev_y_rounded = Some(y_rounded);
                prev_x = Some(x);
                prev_text = Some(span.text.clone());

                // Track content for duplicate detection. Bounded, so a page
                // repeating one string thousands of times cannot grow this
                // without limit; the positions that matter for an overprint are
                // the recent ones.
                let at = seen_content.entry(span.text.clone()).or_default();
                if at.len() < MAX_TRACKED_POSITIONS {
                    at.push((span.bbox.x, span.bbox.y));
                }
                // Move span instead of cloning
                deduplicated.push(span);
            }
        }

        log::debug!(
            "Deduplicated {} spans (geometric: {}, content: {}) ({} -> {} spans)",
            geometric_skips + content_skips,
            geometric_skips,
            content_skips,
            old_len,
            deduplicated.len()
        );

        *spans_in = deduplicated;
    }



fn next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *seed >> 33
}

/// `n` runs on a small grid, with the texts that make the passes decide:
/// repeats, case variants, padded and empty texts, one-glyph runs, texts of
/// five bytes and more, non-ASCII text, and sub-point offsets that straddle the
/// bounds each pass compares against.
fn page(seed: u64, n: usize, distinct: bool) -> Vec<TextSpan> {
    const POOL: [&str; 20] = [
        "ab", "AB", "Ab", " ab ", "a", "A", "é", "éa", "Hello world", "hello world", "SICHERHEITS",
        "S", "ll", "  ", "", "x y", "12345", "1234", "Éa", "ÀÉ",
    ];
    let mut s = seed.wrapping_mul(0x9e3779b97f4a7c15) ^ 0x5851f42d4c957f2d;
    (0..n)
        .map(|i| {
            let text = if distinct && next(&mut s) % 3 != 0 {
                format!("t{}", next(&mut s) % (n as u64 / 2 + 1))
            } else {
                POOL[(next(&mut s) % POOL.len() as u64) as usize].to_string()
            };
            let x = (next(&mut s) % 40) as f32 * 7.0 + (next(&mut s) % 6) as f32 * 0.1;
            let y = 100.0 + (next(&mut s) % 12) as f32 * 11.0 + (next(&mut s) % 4) as f32 * 0.45;
            let w = 4.0 + (next(&mut s) % 30) as f32;
            let h = 8.0 + (next(&mut s) % 4) as f32;
            TextSpan { text, bbox: Rect::new(x, y, w, h), sequence: i, ..TextSpan::default() }
        })
        .collect()
}

fn same(a: &[TextSpan], b: &[TextSpan], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: count");
    for (i, (p, q)) in a.iter().zip(b).enumerate() {
        assert_eq!(p.text, q.text, "{what}: text {i}");
        assert_eq!(p.sequence, q.sequence, "{what}: sequence {i}");
        assert_eq!(
            [p.bbox.x.to_bits(), p.bbox.y.to_bits(), p.bbox.width.to_bits(), p.bbox.height.to_bits()],
            [q.bbox.x.to_bits(), q.bbox.y.to_bits(), q.bbox.width.to_bits(), q.bbox.height.to_bits()],
            "{what}: box {i}"
        );
    }
}

/// No Dart battery can tell which of the two passes ran: the spans that remain
/// are the same either way. Compared here for 40 pages of 2,500 runs, and for
/// 40 pages whose runs are mostly distinct texts (the table grows many times).
#[test]
fn the_passes_that_look_for_a_run_drawn_twice_keep_the_spans_they_always_kept() {
    let mut dropped = (0usize, 0usize);
    for seed in 0..40u64 {
        for distinct in [false, true] {
            let spans = page(seed, 2_500, distinct);

            let mut old = spans.clone();
            reference_stroke_fill(&mut old);
            let mut new = spans.clone();
            TextExtractor::dedup_stroke_fill_spans(&mut new);
            same(&old, &new, &format!("stroke and fill, seed {seed}, distinct {distinct}"));
            dropped.0 += spans.len() - new.len();

            let mut old = spans.clone();
            reference_positional(&mut old);
            let mut new = spans.clone();
            TextExtractor::dedup_positional_spans(&mut new);
            same(&old, &new, &format!("positional, seed {seed}, distinct {distinct}"));
            dropped.1 += spans.len() - new.len();
        }
    }
    // The pages must make both passes drop runs, or the comparison proves nothing.
    assert!(dropped.0 > 1_000 && dropped.1 > 1_000, "dropped {dropped:?}");
}

/// Each of the two passes takes at most
/// [`crate::host::piece_budget::DEDUPE_BYTES_PER_PIECE`] bytes a run and 256 KiB
/// at its peak, over the spans it is given, on a page of 100,000 runs whose
/// texts are mostly distinct; and the pass it replaced takes more than that on
/// the same page, so the bound is one only the new passes keep. No Dart battery
/// can observe this: the spans that remain are the same, and only the allocator
/// counter of the engine sees what the pass held to find them.
#[test]
fn the_passes_hold_a_declared_number_of_bytes_a_piece() {
    alone(module_path!(), "the_passes_hold_a_declared_number_of_bytes_a_piece", || {
        const RUNS: usize = 100_000;
        const C: usize = 256 * 1024;
        let k = crate::host::piece_budget::DEDUPE_BYTES_PER_PIECE;
        let bound = RUNS * k + C;
        let spans = page(7, RUNS, true);
        let growth = |pass: &dyn Fn(&mut Vec<TextSpan>)| {
            let mut list = spans.clone();
            let ((), growth) = heap::growth(|| pass(&mut list));
            growth
        };
        let new_sf = growth(&|l| TextExtractor::dedup_stroke_fill_spans(l));
        let new_pos = growth(&|l| TextExtractor::dedup_positional_spans(l));
        let old_sf = growth(&|l| reference_stroke_fill(l));
        let old_pos = growth(&|l| reference_positional(l));
        println!(
            "{RUNS} runs: stroke and fill {new_sf} bytes (was {old_sf}), positional {new_pos} (was {old_pos}), bound {bound}"
        );
        assert!(new_sf <= bound && new_pos <= bound, "{new_sf} and {new_pos} bytes, over {bound}");
        assert!(old_sf > bound && old_pos > bound, "the old passes took {old_sf} and {old_pos}, not over {bound}");
    });
}
