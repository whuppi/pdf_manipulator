//! Write trait with position tracking — no Seek required.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! PDF serialization needs to know byte offsets for the xref table
//! but never seeks backwards. PositionedWrite captures exactly that:
//! sequential Write + "where am I?" position query.
//!
//! One impl:
//!   CountingWriter<W: Write> — wraps any Write, tracks position via counter.
//!     A position is a count of the bytes written, never a seek that can
//!     fail into 0.

use std::io::{self, Write};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Accepts at most 3 bytes per call to force short writes.
    struct ChunkySink {
        out: Vec<u8>,
    }

    impl Write for ChunkySink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let n = buf.len().min(3);
            self.out.extend_from_slice(&buf[..n]);
            Ok(n)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn position_counts_across_writes() {
        let mut w = CountingWriter::new(Vec::new());
        w.write_all(b"abc").unwrap();
        w.write_all(b"de").unwrap();
        w.write_all(b"fghij").unwrap();
        assert_eq!(w.position(), 10);
        assert_eq!(w.into_inner(), b"abcdefghij".to_vec());
    }

    #[test]
    fn position_tracks_short_inner_writes() {
        let mut sink = ChunkySink { out: Vec::new() };
        let mut w = CountingWriter::new(&mut sink);
        w.write_all(&[0u8; 10]).unwrap();
        assert_eq!(w.position(), 10);
        assert_eq!(sink.out.len(), 10);
    }
}
