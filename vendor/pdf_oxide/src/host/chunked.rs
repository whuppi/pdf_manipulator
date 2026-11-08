//! The chunked wire the host writes a sequence of items on: render writes
//! pages, extractImages writes images.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Wire, after the sink's own length prefix, one frame per
//! `[len: u32 LE][binary response]`:
//!
//! ```text
//! item:   header { kind = "header", ...the item's descriptor }
//!         data   { kind = "data", data: bytes }   (zero or more, each <= 256 KiB)
//!         end    { kind = "end" }
//! ...one such item per page or image, then the terminator [0u32].
//! ```
//!
//! A failure mid-item ends the stream with the op's error response and no
//! end frame for that item; the reader closes what it had open.

use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::host::binary_codec::ResponseWriter;

/// The most payload bytes one data frame carries.
pub const DATA_FRAME_BYTES: usize = 256 * 1024;

/// IDAT chunk size the PNG encoder buffers before it writes a chunk.
pub(crate) const PNG_CHUNK_BYTES: usize = 32 * 1024;

/// Cuts what it is given into data frames of at most [`DATA_FRAME_BYTES`].
pub(crate) struct Framer<'a, W: Write> {
    pub(crate) out: &'a mut W,
    buf: Vec<u8>,
}

impl<'a, W: Write> Framer<'a, W> {
    pub(crate) fn new(out: &'a mut W) -> Self {
        Self {
            out,
            buf: Vec::new(),
        }
    }

    fn send(&mut self, frame: ResponseWriter) -> io::Result<()> {
        let frame = frame.finish();
        self.out.write_all(&(frame.len() as u32).to_le_bytes())?;
        self.out.write_all(&frame)
    }

    fn send_data(&mut self) -> io::Result<()> {
        let mut frame = ResponseWriter::ok();
        frame.put_str("kind", "data");
        frame.put_bytes("data", &self.buf);
        self.buf.clear();
        self.send(frame)
    }

    /// Adds `data` to the item, writing a data frame each time 256 KiB are
    /// buffered.
    pub(crate) fn push(&mut self, mut data: &[u8]) -> io::Result<()> {
        while !data.is_empty() {
            let room = DATA_FRAME_BYTES - self.buf.len();
            let take = room.min(data.len());
            self.buf.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.buf.len() == DATA_FRAME_BYTES {
                self.send_data()?;
            }
        }
        Ok(())
    }

    /// Opens an item: the header frame, with the fields `describe` puts on it.
    pub(crate) fn header(&mut self, describe: impl FnOnce(&mut ResponseWriter)) -> io::Result<()> {
        let mut frame = ResponseWriter::ok();
        frame.put_str("kind", "header");
        describe(&mut frame);
        self.send(frame)
    }

    /// Writes what is buffered and the end frame.
    pub(crate) fn end(&mut self) -> io::Result<()> {
        if !self.buf.is_empty() {
            self.send_data()?;
        }
        let mut frame = ResponseWriter::ok();
        frame.put_str("kind", "end");
        self.send(frame)
    }

    /// The zero-length frame that ends the stream.
    pub(crate) fn terminate(&mut self) -> Result<()> {
        self.out
            .write_all(&0u32.to_le_bytes())
            .map_err(Error::Io)
    }
}

/// The bytes the PNG encoder has produced and the item has not yet passed to
/// the [`Framer`]. The encoder owns its writer for `'static`, so it cannot
/// hold the item's writer itself; the item drains this after every row, so it
/// never holds more than one row's encoding.
#[derive(Clone)]
pub(crate) struct Encoded(pub(crate) Rc<RefCell<Vec<u8>>>);

impl Write for Encoded {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn png_error(e: png::EncodingError) -> Error {
    match e {
        png::EncodingError::IoError(e) => Error::Io(e),
        other => Error::InvalidPdf(format!("PNG encoding failed: {other}")),
    }
}
