//! Band rendering: a page is rastered one horizontal band at a time.
//!
//! This file is fork-only code (NOT upstream).
//!
//! The raster of a page is not bounded by anything in the file, so the
//! renderer never holds it whole. A [`BandPlan`] cuts the page into bands of
//! rows that together stay inside [`BAND_BUDGET_BYTES`]; each band is a pixmap
//! of the page's width, rendered by replaying the page's content with the base
//! transform translated up by the band's top, and handed to a [`RowSink`] as
//! straight-alpha rows. Every scratch buffer the renderer makes is the size of
//! the pixmap it is drawing on, so it is band-sized with no further work.

use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::error::Result;
use crate::object::{Object, ObjectRef};
use crate::rendering::page_renderer::{PageRenderer, RenderOptions, MAX_FORM_DEPTH};
use std::ops::Range;

/// Raster bytes every band-sized buffer of one render may use together.
pub(crate) const BAND_BUDGET_BYTES: usize = 2 * 1024 * 1024;

/// How far, in device pixels, a stroke thinner than a pixel draws past its
/// path: an anti-aliased hairline spreads over the pixels either side of it,
/// and a cap adds up to two more.
pub(crate) const HAIRLINE_REACH: f64 = 2.0;

/// How a page's rows are cut into bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandPlan {
    /// Page raster width in pixels; every band is this wide.
    pub width: u32,
    /// Page raster height in pixels.
    pub height: u32,
    /// Rows in a band; the last band may have fewer.
    pub rows_per_band: u32,
    /// Deepest nesting of transparency groups, soft masks and non-separable
    /// blends the page can reach: each level holds one more band-sized buffer.
    pub depth: u32,
}

impl BandPlan {
    /// The plan that fits `bytes_per_pixel` of persistent state per pixel and
    /// `depth` further band-sized buffers of the same shape into the budget,
    /// with at least one row per band.
    pub fn new(width: u32, height: u32, bytes_per_pixel: usize, depth: u32) -> Self {
        let row_bytes = (width as usize).saturating_mul(bytes_per_pixel).max(1);
        let cost = row_bytes.saturating_mul(1 + depth as usize);
        let rows = (BAND_BUDGET_BYTES / cost).max(1);
        Self {
            width,
            height,
            rows_per_band: u32::try_from(rows).unwrap_or(u32::MAX).min(height.max(1)),
            depth,
        }
    }

    /// The same plan with a forced band height, for proofs that compare cuts.
    pub fn with_rows_per_band(mut self, rows_per_band: u32) -> Self {
        self.rows_per_band = rows_per_band.max(1).min(self.height.max(1));
        self
    }

    /// `(top, rows)` of each band covering the page rows in `range`.
    pub fn bands(&self, range: Range<u32>) -> impl Iterator<Item = (u32, u32)> {
        let end = range.end.min(self.height);
        let step = self.rows_per_band.max(1);
        (range.start..end)
            .step_by(step as usize)
            .map(move |top| (top, step.min(end - top)))
    }
}

/// Receives a rendered page as straight-alpha RGBA rows, top to bottom.
pub trait RowSink {
    /// The raster's size, called once before the first [`Self::rows`] and
    /// before any row of the page is delivered.
    fn begin(&mut self, _width: u32, _height: u32) -> Result<()> {
        Ok(())
    }

    /// `rgba_straight` holds whole rows of the page's width, the first of
    /// them page row `y`. Called once per band, in order.
    fn rows(&mut self, y: u32, rgba_straight: &[u8]) -> Result<()>;
}

/// The size of the page raster a band is cut from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BandContext {
    pub(crate) width: u32,
    pub(crate) page_height: u32,
    /// Rows in a band: the cut a question about the whole page is asked in.
    pub(crate) rows: u32,
}

impl BandContext {
    /// A page whose raster is not known yet.
    pub(crate) fn whole(width: u32, height: u32) -> Self {
        Self {
            width,
            page_height: height,
            rows: height,
        }
    }
}

/// The part of the device plane a pixmap shows. Everything the renderer
/// draws is in the device plane, in whole-page coordinates; a band is a
/// window onto it, and any other pixmap made like the band shows the same
/// part.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Window {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

impl Window {
    pub(crate) fn of(pixmap: &tiny_skia::Pixmap) -> Self {
        let (left, top) = pixmap.origin();
        Self {
            left,
            top,
            width: pixmap.width(),
            height: pixmap.height(),
        }
    }

    pub(crate) fn of_mask(mask: &tiny_skia::Mask) -> Self {
        let (left, top) = mask.origin();
        Self {
            left,
            top,
            width: mask.width(),
            height: mask.height(),
        }
    }

    /// True when device bounds lie wholly outside the window, one pixel of
    /// slack on every side for anti-aliasing. A paint that misses draws
    /// nothing here.
    pub(crate) fn misses(&self, bounds: tiny_skia::Rect) -> bool {
        f64::from(bounds.right()) < f64::from(self.left) - 1.0
            || f64::from(bounds.bottom()) < f64::from(self.top) - 1.0
            || f64::from(bounds.left()) > f64::from(self.left) + f64::from(self.width) + 1.0
            || f64::from(bounds.top()) > f64::from(self.top) + f64::from(self.height) + 1.0
    }

    /// True when the device-plane rectangle `[left, top, right, bottom]` does
    /// not touch the window at all (no slack).
    pub(crate) fn excludes(&self, left: f64, top: f64, right: f64, bottom: f64) -> bool {
        right < f64::from(self.left)
            || bottom < f64::from(self.top)
            || left > f64::from(self.left) + f64::from(self.width)
            || top > f64::from(self.top) + f64::from(self.height)
    }

    /// The window as a device-plane rectangle.
    pub(crate) fn rect(&self) -> Option<tiny_skia::Rect> {
        tiny_skia::Rect::from_xywh(
            self.left as f32,
            self.top as f32,
            self.width as f32,
            self.height as f32,
        )
    }

    /// True when `path` under `transform`, widened by `reach` device units
    /// (the outline reach of a stroke), misses the band. A path whose bounds
    /// cannot be placed never counts as missing: the rasteriser decides.
    pub(crate) fn misses_path(
        &self,
        path: &tiny_skia::Path,
        transform: tiny_skia::Transform,
        reach: f64,
    ) -> bool {
        let Some([x0, y0, x1, y1]) = super::device_bounds(path, transform) else {
            return false;
        };
        tiny_skia::Rect::from_ltrb(
            (x0 - reach) as f32,
            (y0 - reach) as f32,
            (x1 + reach) as f32,
            (y1 + reach) as f32,
        )
        .is_some_and(|bounds| self.misses(bounds))
    }

    /// [`Self::misses_path`] for a stroke: the outline reaches at most the
    /// line width, times the miter limit where a miter join spikes, from the
    /// centerline.
    pub(crate) fn misses_stroke(
        &self,
        path: &tiny_skia::Path,
        transform: tiny_skia::Transform,
        width: f32,
        miter_limit: f32,
    ) -> bool {
        let reach = (f64::from(width.abs())
            * f64::from(miter_limit.max(1.5))
            * super::device_scale(transform))
        .max(HAIRLINE_REACH);
        self.misses_path(path, transform, reach)
    }

    /// True when the unit square under `transform` (an image's footprint)
    /// misses the band.
    pub(crate) fn misses_unit_square(&self, transform: tiny_skia::Transform) -> bool {
        let mut builder = tiny_skia::PathBuilder::new();
        builder.push_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("unit square"));
        builder
            .finish()
            .is_some_and(|square| self.misses_path(&square, transform, 0.0))
    }
}

thread_local! {
    static FILLS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Records that a fill was handed to the rasteriser, whether or not it
/// reached the band.
pub(crate) fn count_fill() {
    FILLS.with(|fills| fills.set(fills.get().wrapping_add(1)));
}

// ── pdf_manipulator patch: a lane starts on a clean thread (host::caches::install) ──
/// Starts this thread's fill count over.
pub(crate) fn reset_fills() {
    FILLS.with(|fills| fills.set(0));
}
// ── end pdf_manipulator patch ──

/// Fills handed to the rasteriser on this thread so far. A caller that needs
/// to know whether some drawing produced any outline at all, wherever the
/// outline fell, compares two readings around it: the answer must not depend
/// on which band is being rendered.
pub(crate) fn fills_counted() -> u64 {
    FILLS.with(std::cell::Cell::get)
}

/// Converts a premultiplied RGBA8 buffer to straight alpha in place. The one
/// conversion every output of the renderer goes through.
pub(crate) fn demultiply_in_place(data: &mut [u8]) {
    for chunk in data.as_chunks_mut::<4>().0 {
        let a = chunk[3];
        if a != 0 && a != 255 {
            let a32 = a as u32;
            chunk[0] = ((chunk[0] as u32 * 255 + a32 / 2) / a32).min(255) as u8;
            chunk[1] = ((chunk[1] as u32 * 255 + a32 / 2) / a32).min(255) as u8;
            chunk[2] = ((chunk[2] as u32 * 255 + a32 / 2) / a32).min(255) as u8;
        }
    }
}

/// Renders page `page` (zero-based) and hands its rows to `sink`, band by
/// band. Returns the raster's `(width, height)`.
///
/// `cancel` is checked before every band. A failure after the first band has
/// been delivered leaves the sink with the rows already written.
pub fn render_page_rows(
    doc: &PdfDocument,
    page: usize,
    options: &RenderOptions,
    sink: &mut dyn RowSink,
    cancel: &CancelCheck,
) -> Result<(u32, u32)> {
    let mut renderer = PageRenderer::new(options.clone());
    let page_height = std::cell::Cell::new(0u32);
    let mut begun = false;
    renderer.render_bands(
        doc,
        page,
        &|_, height| {
            page_height.set(height);
            0..height
        },
        cancel,
        &mut |top, pixmap| {
            if !begun {
                begun = true;
                sink.begin(pixmap.width(), page_height.get())?;
            }
            demultiply_in_place(pixmap.data_mut());
            sink.rows(top, pixmap.data())
        },
    )
}

/// The deepest nesting of transparency groups, soft masks and non-separable
/// blends reachable from `resources`, found from dictionaries alone: forms
/// with a `/Group`, soft masks in `/ExtGState`, and non-separable blend
/// modes, recursing through the resources of forms and tiling patterns.
/// Content is never read. A reference already on the path ends that branch,
/// so a cycle cannot loop.
pub(crate) fn group_depth(doc: &PdfDocument, resources: &Object) -> u32 {
    walk_resources(doc, resources, &mut Vec::new())
}

fn walk_resources(doc: &PdfDocument, resources: &Object, path: &mut Vec<ObjectRef>) -> u32 {
    if path.len() as u32 >= MAX_FORM_DEPTH {
        return 0;
    }
    let Ok(resources) = doc.resolve_object(resources) else {
        return 0;
    };
    let Some(dict) = resources.as_dict() else {
        return 0;
    };
    let mut deepest = 0;

    for entry in resource_entries(doc, dict.get("XObject")) {
        if let Some((inner, group)) = form_levels(doc, &entry, path) {
            deepest = deepest.max(inner + u32::from(group));
        }
    }

    for entry in resource_entries(doc, dict.get("ExtGState")) {
        let Ok(state) = doc.resolve_object(&entry) else {
            continue;
        };
        let Some(state) = state.as_dict() else {
            continue;
        };
        if state.get("BM").is_some_and(is_non_separable_blend) {
            deepest = deepest.max(1);
        }
        if let Some(mask) = state.get("SMask").and_then(|m| doc.resolve_object(m).ok()) {
            if let Some(group) = mask.as_dict().and_then(|m| m.get("G")) {
                if let Some((inner, _)) = form_levels(doc, group, path) {
                    deepest = deepest.max(inner + 1);
                }
            }
        }
    }

    for entry in resource_entries(doc, dict.get("Pattern")) {
        let Ok(pattern) = doc.resolve_object(&entry) else {
            continue;
        };
        let Some(resources) = pattern.as_dict().and_then(|p| p.get("Resources")) else {
            continue;
        };
        let reference = entry.as_reference();
        if reference.is_some_and(|r| path.contains(&r)) {
            continue;
        }
        path.extend(reference);
        deepest = deepest.max(walk_resources(doc, resources, path));
        if reference.is_some() {
            path.pop();
        }
    }

    deepest
}

/// The entries of a resource sub-dictionary (`/XObject`, `/ExtGState`,
/// `/Pattern`), each as the object the dictionary holds: a reference stays a
/// reference.
fn resource_entries(doc: &PdfDocument, sub: Option<&Object>) -> Vec<Object> {
    let Some(sub) = sub.and_then(|s| doc.resolve_object(s).ok()) else {
        return Vec::new();
    };
    sub.as_dict()
        .map(|d| d.values().cloned().collect())
        .unwrap_or_default()
}

/// For a form XObject entry: the group levels inside its own resources and
/// whether the form is itself a group. `None` for anything that is not a form
/// or that is already on the path.
fn form_levels(doc: &PdfDocument, entry: &Object, path: &mut Vec<ObjectRef>) -> Option<(u32, bool)> {
    let reference = entry.as_reference();
    if reference.is_some_and(|r| path.contains(&r)) {
        return None;
    }
    let form = doc.resolve_object(entry).ok()?;
    let Object::Stream { dict, .. } = &form else {
        return None;
    };
    if dict.get("Subtype").and_then(Object::as_name) != Some("Form") {
        return None;
    }
    path.extend(reference);
    let inner = dict
        .get("Resources")
        .map_or(0, |r| walk_resources(doc, r, path));
    if reference.is_some() {
        path.pop();
    }
    Some((inner, dict.contains_key("Group")))
}

fn is_non_separable_blend(bm: &Object) -> bool {
    let non_separable = |name: &str| matches!(name, "Hue" | "Saturation" | "Color" | "Luminosity");
    match bm {
        Object::Name(name) => non_separable(name),
        Object::Array(names) => names
            .first()
            .and_then(Object::as_name)
            .is_some_and(non_separable),
        _ => false,
    }
}
