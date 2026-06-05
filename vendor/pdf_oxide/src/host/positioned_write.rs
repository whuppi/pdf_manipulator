//! Write trait with position tracking — no Seek required.
//!
//! PDF serialization needs to know byte offsets for the xref table
//! but never seeks backwards. PositionedWrite captures exactly that:
//! sequential Write + "where am I?" position query.
//!
//! Two impls:
//!   CountingWriter<W: Write> — wraps any Write, tracks position via counter.
//!     Use for streaming sinks that don't support Seek.
//!   SeekWriter<W: Write + Seek> — wraps Write+Seek, delegates position to
//!     stream_position(). Use for Cursor<Vec<u8>>, BufWriter<File>, etc.

use std::io::{self, Seek, Write};

/// A writer that knows its current byte position.
pub trait PositionedWrite: Write {
    /// Return the current byte offset in the output stream.
    fn position(&mut self) -> u64;
}

/// Wraps any Write, tracks position with a counter.
pub struct CountingWriter<W: Write> {
    inner: W,
    pos: u64,
}

impl<W: Write> CountingWriter<W> {
    /// Create a new counting writer starting at position 0.
    pub fn new(inner: W) -> Self {
        Self { inner, pos: 0 }
    }

    /// Consume the wrapper and return the inner writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.pos += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write> PositionedWrite for CountingWriter<W> {
    fn position(&mut self) -> u64 {
        self.pos
    }
}

/// Wraps any Write+Seek, delegates position to stream_position().
pub struct SeekWriter<W: Write + Seek> {
    inner: W,
}

impl<W: Write + Seek> SeekWriter<W> {
    /// Create a new seek-based writer.
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Consume the wrapper and return the inner writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write + Seek> Write for SeekWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write + Seek> PositionedWrite for SeekWriter<W> {
    fn position(&mut self) -> u64 {
        self.inner.stream_position().unwrap_or(0)
    }
}
