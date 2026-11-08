//! The sort over every glyph of a page polls for a cancel and ends at once
//! when one lands, and a run with no cancel puts the glyphs in the order a
//! plain sort by line and then by x would.
//!
//! The sort is a stable merge sort over indices: it polls before each run of
//! 4,096 glyphs, every 4,096 steps of a merge and of the final reordering.
//!
//! No Dart battery can observe this: a cancel from Dart lands at whatever poll
//! the timing reaches, so a battery sees "cancelled or finished", never that
//! the glyph sort of a page of tens of thousands of lines holds a poll.

use crate::cancel::{enter, poll, CancelCheck};
use crate::extractors::text::{AlignedChar, TextExtractor};

const GLYPHS: usize = 20_000;

/// `GLYPHS` glyphs in a scrambled order, many on each line and some at the
/// same x, so the order the sort settles is the stable one.
fn glyphs() -> Vec<AlignedChar> {
    let mut state = 0x2545_F491u32;
    (0..GLYPHS)
        .map(|i| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            AlignedChar {
                char: char::from(b'a' + (i % 26) as u8),
                origin_x: ((state >> 8) % 50) as f32,
                origin_y: ((state >> 16) % 200) as f32 + 0.25,
                width: 1.0,
                height: 1.0,
                advance_width: i as f32,
            }
        })
        .collect()
}

/// Line by line from the top, left to right, equal glyphs in the order they came.
fn reference(glyphs: &[AlignedChar]) -> Vec<f32> {
    let mut keyed: Vec<&AlignedChar> = glyphs.iter().collect();
    keyed.sort_by(|a, b| {
        (b.origin_y.round() as i32)
            .cmp(&(a.origin_y.round() as i32))
            .then(a.origin_x.partial_cmp(&b.origin_x).unwrap())
    });
    keyed.iter().map(|g| g.advance_width).collect()
}

/// No Dart battery can observe this: the sorted order is the same with or
/// without the sort's polls, and a poll count is not visible from Dart.
#[test]
fn a_run_with_no_cancel_keeps_the_order_and_polls() {
    let input = glyphs();
    let expected = reference(&input);

    let mut alone = input.clone();
    TextExtractor::sort_glyphs(&mut alone);
    assert_eq!(alone.iter().map(|g| g.advance_width).collect::<Vec<_>>(), expected);

    let check = CancelCheck::tripping_at(usize::MAX);
    let mut polled = input;
    {
        let _job = enter(&check);
        TextExtractor::sort_glyphs(&mut polled);
        poll().expect("a check that never trips");
    }
    assert_eq!(polled.iter().map(|g| g.advance_width).collect::<Vec<_>>(), expected);
    assert!(check.polls() > 2, "the sort polled {} times", check.polls() - 1);
}

/// No Dart battery can observe this: a cancel from Dart lands where the timing
/// puts it, never inside the one sort of a page's glyphs.
#[test]
fn a_cancel_during_the_sort_ends_it_with_the_cancelled_error() {
    let check = CancelCheck::tripping_at(2);
    let mut input = glyphs();
    let _job = enter(&check);
    TextExtractor::sort_glyphs(&mut input);
    assert_eq!(check.polls(), 2, "the sort polled {} times: twice is the poll, then the cancel", check.polls());
    let error = poll().expect_err("the cancel is reported right after the sort");
    assert!(
        matches!(&error, crate::error::Error::Io(e) if e.to_string() == "operation cancelled"),
        "{error:?}"
    );
}
