//! Image bounds edits on the belt: a stage that sits between the operators of
//! a page and the stream the save writes, and moves the images the caller
//! named.
//!
//! The stage holds the last `cm` since the last `q`. When the next operator is
//! a `Do` of a modified image it writes that `cm` adjusted (or one of its own
//! when none preceded); any other operator writes it unchanged first. It holds
//! one matrix and the matrix stack, never the content, and every error goes
//! to the save.

use super::ImageModification;
use crate::content::operators::Operator;
use crate::host::content::{InlineImages, Mode, Operators};
use crate::redaction::serialize::serialize_operator;
use std::collections::HashMap;
use std::io::{self, Write};

type M = [f32; 6];

const IDENTITY: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m` applied first, then `n` (row-vector convention).
fn mul(m: M, n: M) -> M {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn inverse(m: M) -> Option<M> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < f32::EPSILON {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

/// The matrix that places an image where `modification` wants it, in page
/// space: `inner` is the last `cm` before the `Do` (the identity when none)
/// and `ctm` the matrix in force before it. The modification is made on the
/// composed matrix, then mapped back through the enclosing transform; a
/// singular enclosing transform (nothing visible anyway) edits `inner`
/// directly.
fn adjusted(inner: M, ctm: M, modification: &ImageModification) -> M {
    let composed = mul(inner, ctm);
    let mut wanted = composed;
    if let Some(width) = modification.width {
        let len = (composed[0] * composed[0] + composed[1] * composed[1]).sqrt();
        if len > f32::EPSILON {
            wanted[0] = composed[0] / len * width;
            wanted[1] = composed[1] / len * width;
        }
    }
    if let Some(height) = modification.height {
        let len = (composed[2] * composed[2] + composed[3] * composed[3]).sqrt();
        if len > f32::EPSILON {
            wanted[2] = composed[2] / len * height;
            wanted[3] = composed[3] / len * height;
        }
    }
    if let Some(x) = modification.x {
        wanted[4] = x;
    }
    if let Some(y) = modification.y {
        wanted[5] = y;
    }
    match inverse(ctm) {
        Some(inv) => mul(wanted, inv),
        None => [
            modification.width.unwrap_or(inner[0]),
            inner[1],
            inner[2],
            modification.height.unwrap_or(inner[3]),
            modification.x.unwrap_or(inner[4]),
            modification.y.unwrap_or(inner[5]),
        ],
    }
}

/// The stage. It is a [`Write`] over operator-aligned chunks (the redaction
/// pass writes one operator at a time) and takes operators directly through
/// [`Self::push`].
pub(super) struct ImageModStage<'a, W: Write> {
    mods: &'a HashMap<String, ImageModification>,
    out: W,
    ctm: M,
    saved: Vec<M>,
    held: Option<M>,
    bytes: Vec<u8>,
}

impl<'a, W: Write> ImageModStage<'a, W> {
    pub fn new(mods: &'a HashMap<String, ImageModification>, out: W) -> Self {
        Self { mods, out, ctm: IDENTITY, saved: Vec::new(), held: None, bytes: Vec::new() }
    }

    fn emit(&mut self, op: &Operator) -> io::Result<()> {
        self.bytes.clear();
        serialize_operator(&mut self.bytes, op);
        self.out.write_all(&self.bytes)
    }

    /// Writes the held `cm` as it was read.
    fn flush_held(&mut self) -> io::Result<()> {
        if let Some(m) = self.held.take() {
            self.emit(&Operator::Cm { a: m[0], b: m[1], c: m[2], d: m[3], e: m[4], f: m[5] })?;
            self.ctm = mul(m, self.ctm);
        }
        Ok(())
    }

    /// Takes one operator of the page.
    pub fn push(&mut self, op: &Operator) -> io::Result<()> {
        match op {
            Operator::Cm { a, b, c, d, e, f } => {
                self.flush_held()?;
                self.held = Some([*a, *b, *c, *d, *e, *f]);
            },
            Operator::Do { name } if self.mods.contains_key(name.as_str()) => {
                let inner = self.held.take().unwrap_or(IDENTITY);
                let placed = adjusted(inner, self.ctm, &self.mods[name.as_str()]);
                self.out.write_all(
                    format!(
                        "{:.6} {:.6} {:.6} {:.6} {:.6} {:.6} cm\n",
                        placed[0], placed[1], placed[2], placed[3], placed[4], placed[5]
                    )
                    .as_bytes(),
                )?;
                self.ctm = mul(placed, self.ctm);
                self.emit(op)?;
            },
            other => {
                self.flush_held()?;
                match other {
                    Operator::SaveState => self.saved.push(self.ctm),
                    Operator::RestoreState => {
                        if let Some(saved) = self.saved.pop() {
                            self.ctm = saved;
                        }
                    },
                    _ => {},
                }
                self.emit(other)?;
            },
        }
        Ok(())
    }

    /// Writes what the stage still holds. Called once, after the last operator.
    pub fn finish(&mut self) -> io::Result<()> {
        self.flush_held()?;
        self.out.flush()
    }

    /// Takes every operator of `chunk`, a run of whole operators.
    fn push_chunk(&mut self, chunk: &[u8]) -> io::Result<()> {
        let mut ops = Operators::new(chunk, Mode::Full).with_inline_images(InlineImages::Collect);
        for item in ops.by_ref() {
            crate::cancel::poll_io()?;
            let op = item.map_err(io::Error::other)?;
            self.push(&op)?;
        }
        if !ops.damage().is_clean() {
            return Err(io::Error::other(
                "the content stream is damaged; refusing to move an image in a stream whose \
                 operators cannot be read in full",
            ));
        }
        Ok(())
    }
}

impl<W: Write> Write for ImageModStage<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.push_chunk(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
