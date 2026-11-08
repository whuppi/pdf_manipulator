//! Page rendering as a chunked wire: pixels reach the sink as they are
//! rastered, never as a whole image.
//!
//! This file is fork-only code (NOT upstream).
//!
//! A page is rastered in bands (`rendering::band`). Each band's rows go
//! straight to a PNG encoder (`png::StreamWriter`, which keeps only the
//! previous row, the current row and its filtered row) or, for the raw
//! format, are written as they are. What the encoder emits is cut into data
//! frames of at most [`DATA_FRAME_BYTES`], so neither the raster nor its
//! encoded form is ever held whole.
//!
//! Wire: the chunked framing of `host::chunked`, one item per page, whose
//! header is `{ kind = "header", page, width, height, format }`.

use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::error::{Error, Result};
use crate::host::chunked::{png_error, Encoded, Framer, PNG_CHUNK_BYTES};
use crate::rendering::{fit_options, render_page_rows, RenderOptions, RowSink};

#[cfg(test)]
pub use crate::host::chunked::DATA_FRAME_BYTES;

/// How large a rendered page is, by the kind the wire names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSize {
    /// Dots per inch: the page box at that resolution.
    Dpi(u32),
    /// As large as fits inside the box, keeping the aspect ratio.
    Fit { max_width: u32, max_height: u32 },
    /// As large as fits inside a square.
    Thumbnail(u32),
}

/// What the data frames of a page carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFormat {
    /// A PNG file.
    Png,
    /// Straight-alpha RGBA rows, top to bottom, 4 bytes a pixel.
    Rgba,
}

impl RenderFormat {
    /// The format a wire name stands for.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "png" => Some(Self::Png),
            "rgba" => Some(Self::Rgba),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Rgba => "rgba",
        }
    }
}

impl RenderSize {
    /// The render options that give `page` this size.
    fn options_for(self, doc: &PdfDocument, page: usize) -> Result<RenderOptions> {
        let base = RenderOptions::default();
        match self {
            Self::Dpi(dpi) => {
                if dpi == 0 {
                    return Err(Error::InvalidOperation("dpi must be positive".into()));
                }
                Ok(RenderOptions::with_dpi(dpi))
            },
            Self::Fit {
                max_width,
                max_height,
            } => fit_options(doc, page, max_width, max_height, &base),
            Self::Thumbnail(size) => fit_options(doc, page, size, size, &base),
        }
    }
}

/// Renders `pages` to `writer` as chunked pages, then the terminator.
/// Returns the number of pages written.
pub fn render_pages_streamed<W: Write>(
    doc: &PdfDocument,
    pages: &[usize],
    size: RenderSize,
    format: RenderFormat,
    writer: &mut W,
    cancel: &CancelCheck,
) -> Result<usize> {
    let mut framer = Framer::new(writer);
    cancel.each(pages.iter().copied(), |page| {
        let options = size.options_for(doc, page)?;
        let mut sink = PageSink {
            page,
            format,
            framer: &mut framer,
            width: 0,
            encoded: Rc::new(RefCell::new(Vec::new())),
            png: None,
        };
        render_page_rows(doc, page, &options, &mut sink, cancel)?;
        sink.finish()
    })?;
    framer.terminate()?;
    Ok(pages.len())
}

/// One page's rows on their way to the wire.
struct PageSink<'f, 'a, W: Write> {
    page: usize,
    format: RenderFormat,
    framer: &'f mut Framer<'a, W>,
    /// Pixels per row, known at [`RowSink::begin`].
    width: usize,
    encoded: Rc<RefCell<Vec<u8>>>,
    png: Option<png::StreamWriter<'static, Encoded>>,
}

impl<W: Write> PageSink<'_, '_, W> {
    /// Moves what the encoder has produced to the framer.
    fn drain(&mut self) -> io::Result<()> {
        let mut encoded = self.encoded.borrow_mut();
        let result = self.framer.push(&encoded);
        encoded.clear();
        result
    }

    /// Ends the page: the encoder's last bytes, the buffered data, the end
    /// frame.
    fn finish(mut self) -> Result<()> {
        if let Some(png) = self.png.take() {
            png.finish().map_err(png_error)?;
            self.drain().map_err(Error::Io)?;
        }
        self.framer.end().map_err(Error::Io)
    }
}

impl<W: Write> RowSink for PageSink<'_, '_, W> {
    fn begin(&mut self, width: u32, height: u32) -> Result<()> {
        let (page, format) = (self.page, self.format);
        self.framer
            .header(|frame| {
                frame.put_i32("page", page as i32);
                frame.put_i32("width", width as i32);
                frame.put_i32("height", height as i32);
                frame.put_str("format", format.name());
            })
            .map_err(Error::Io)?;
        self.width = width as usize;
        if self.format == RenderFormat::Png {
            let mut encoder = png::Encoder::new(Encoded(Rc::clone(&self.encoded)), width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_compression(png::Compression::Fast);
            encoder.set_filter(png::Filter::Sub);
            let writer = encoder.write_header().map_err(png_error)?;
            self.png = Some(
                writer
                    .into_stream_writer_with_size(PNG_CHUNK_BYTES)
                    .map_err(png_error)?,
            );
            self.drain().map_err(Error::Io)?;
        }
        Ok(())
    }

    fn rows(&mut self, _y: u32, rgba_straight: &[u8]) -> Result<()> {
        for row in rgba_straight.chunks(self.width * 4) {
            match &mut self.png {
                Some(png) => {
                    png.write_all(row).map_err(Error::Io)?;
                    self.drain().map_err(Error::Io)?;
                },
                None => self.framer.push(row).map_err(Error::Io)?,
            }
        }
        Ok(())
    }
}
