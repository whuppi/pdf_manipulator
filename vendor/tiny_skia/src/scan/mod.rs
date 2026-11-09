// Copyright 2011 The Android Open Source Project
// Copyright 2020 Yevhenii Reizner
//
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

pub mod hairline;
pub mod hairline_aa;
pub mod path;
pub mod path_aa;

use crate::{IntRect, Rect};

use crate::blitter::Blitter;
use crate::geom::{IntRectExt, ScreenIntRect};

// ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
/// `rect` is in the device plane, rounded there; the window's `origin` leaves
/// as whole pixels.
pub fn fill_rect(rect: &Rect, clip: &ScreenIntRect, origin: (i32, i32), blitter: &mut dyn Blitter) {
    if let Some(rect) = rect.round().and_then(|r| path::in_window(&r, origin)) {
        fill_int_rect(&rect, clip, blitter);
    }
}
// ── end pdf_manipulator patch ──

fn fill_int_rect(rect: &IntRect, clip: &ScreenIntRect, blitter: &mut dyn Blitter) {
    let rect = match rect.intersect(&clip.to_int_rect()) {
        Some(v) => v,
        None => return, // everything was clipped out
    };

    let rect = match rect.to_screen_int_rect() {
        Some(v) => v,
        None => return,
    };

    blitter.blit_rect(&rect);
}

// ── pdf_manipulator patch: a pixmap is a window onto a larger device plane ──
pub fn fill_rect_aa(
    rect: &Rect,
    clip: &ScreenIntRect,
    origin: (i32, i32),
    blitter: &mut dyn Blitter,
) {
    hairline_aa::fill_rect(rect, clip, origin, blitter);
}
// ── end pdf_manipulator patch ──
