//! Builder images that stream from their source.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! `builderAddImagePage` and the `image` page op hold the image's source
//! until the builder is saved or disposed, and read from it only when the
//! save reaches the image:
//!
//!   JPEG — the header is scanned for the frame (size, components); the
//!          stream is the source's bytes, copied in 64 KiB pieces under
//!          `DCTDecode`, never decoded.
//!   PNG  — the colour samples are decoded one row at a time from the
//!          source and written as a Flate stream with a PNG `None` filter
//!          byte per row (`Predictor` 15); the alpha channel is a second
//!          pass over the source, written as the `/SMask` stream. The
//!          stream's `/Length` is the object after it, since the deflated
//!          size is known only once the rows are written.
//!
//! The pixels are the ones `ImageData::from_png` gives: palette, `tRNS`
//! and low bit depths expand as the PNG decoder expands them, and a 16 bit
//! sample is rounded to 8 bits as `image` rounds it, `(v + 128) / 257`.
//! An interlaced (Adam7) PNG stores its seven passes one after the other, so
//! no row of the image is complete until the last pass has been read. It is
//! decoded by seven readers over the one source, each positioned on its own
//! pass and pulled in step, one output row at a time: a reader holds the
//! window and the rows of the decoder, and the seven hold no image.
//!
//! An editor's image stamp stages the same streams as objects whose data is
//! an extent ([`StreamedImage::stage`]): the save reads a JPEG from its source
//! in pieces, and a PNG, interlaced or not, through a reader that deflates its
//! rows as they are asked for.

use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};
use crate::host::bridge_api::BoxedReader;
use crate::host::positioned_write::PositionedWrite;
use crate::object::{Extent, Object, ObjectRef, StreamBody};
use crate::source::Source;
use crate::writer::{ColorSpace, ImageData, ImageFormat};
use crate::writer::ObjectSerializer;

/// Bytes moved per read and per write when a JPEG is copied.
const PIECE: usize = 64 * 1024;

/// What a frame header says of a JPEG.
#[derive(Debug, Clone, Copy)]
struct JpegShape {
    width: u32,
    height: u32,
    components: u8,
    /// The size `image_pixel_dimensions` reads from the head of the file,
    /// which sizes the page the image is placed on.
    placement: (u32, u32),
}

/// What the PNG decoder will produce, rows of 8 or 16 bit samples.
#[derive(Debug, Clone, Copy)]
struct PngShape {
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    /// Adam7: the rows come in seven passes.
    interlaced: bool,
}

impl PngShape {
    fn has_alpha(&self) -> bool {
        matches!(self.color, png::ColorType::GrayscaleAlpha | png::ColorType::Rgba)
    }

    fn color_space(&self) -> ColorSpace {
        match self.color {
            png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha => ColorSpace::DeviceGray,
            _ => ColorSpace::DeviceRGB,
        }
    }

    /// Bytes a pixel of the decoder's rows holds.
    fn pixel_bytes(&self) -> usize {
        self.samples() * if self.depth == png::BitDepth::Sixteen { 2 } else { 1 }
    }

    /// Samples a pixel of the decoder's rows holds.
    fn samples(&self) -> usize {
        match self.color {
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            _ => 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Shape {
    Jpeg(JpegShape),
    Png(PngShape),
}

/// An image the save reads from its source: the source, its length and what
/// its header said.
pub struct StreamedImage {
    source: Mutex<BoxedReader>,
    length: u64,
    shape: Shape,
}

impl std::fmt::Debug for StreamedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamedImage").field("length", &self.length).field("shape", &self.shape).finish()
    }
}

/// An image as the page op holds it until the save.
#[derive(Debug, Clone)]
pub enum ImageBytes {
    /// The whole encoded file, as `ImageData::from_bytes` takes it.
    Whole(Vec<u8>),
    /// The file stays in its source.
    Streamed(Arc<StreamedImage>),
}

/// What reading the head of a source found.
pub(crate) enum Taken {
    Image(ImageBytes),
    /// Not a PNG or JPEG the embedding can read.
    Unreadable,
}

impl ImageBytes {
    /// The image's size in pixels, read from its header.
    pub(crate) fn pixel_dimensions(&self) -> Option<(u32, u32)> {
        match self {
            ImageBytes::Whole(bytes) => crate::host::ops::builder::image_pixel_dimensions(bytes),
            ImageBytes::Streamed(image) => Some(image.pixel_dimensions()),
        }
    }
}

/// Reads as much of `buf` as the source has.
fn read_head(reader: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = reader.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

/// Takes the image in `reader`: a JPEG or a PNG stays in its source.
pub(crate) fn take(mut reader: BoxedReader) -> Result<Taken> {
    let length = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 8];
    let got = read_head(&mut reader, &mut magic)?;
    reader.seek(SeekFrom::Start(0))?;

    let shape = if got >= 2 && magic[0] == 0xFF && magic[1] == 0xD8 {
        match jpeg_shape(&mut reader, length)? {
            Some(shape) => Shape::Jpeg(shape),
            None => return Ok(Taken::Unreadable),
        }
    } else if got >= 8 && magic == *b"\x89PNG\r\n\x1a\n" {
        match png_shape(&mut reader) {
            Some(shape) => Shape::Png(shape),
            None => return Ok(Taken::Unreadable),
        }
    } else {
        return Ok(Taken::Unreadable);
    };
    Ok(Taken::Image(ImageBytes::Streamed(Arc::new(StreamedImage {
        source: Mutex::new(reader),
        length,
        shape,
    }))))
}

/// The frame of the JPEG at the start of `reader`, found by walking its
/// marker segments up to the first scan: the header, never the entropy-coded
/// data. `None` when no frame comes before the scan.
fn jpeg_shape(reader: &mut BoxedReader, length: u64) -> Result<Option<JpegShape>> {
    let Some((mut shape, frame_end)) = jpeg_frame(reader)? else {
        return Ok(None);
    };
    // The page is sized by the same reading of the head as ever; the head it
    // needs ends two bytes past the frame's fixed part.
    reader.seek(SeekFrom::Start(0))?;
    let mut head = vec![0u8; length.min(frame_end + 2) as usize];
    let got = read_head(reader, &mut head)?;
    head.truncate(got);
    match crate::host::ops::builder::image_pixel_dimensions(&head) {
        Some(placement) => {
            shape.placement = placement;
            Ok(Some(shape))
        },
        None => Ok(None),
    }
}

/// The frame header and the offset just past its fixed part.
fn jpeg_frame(reader: &mut BoxedReader) -> Result<Option<(JpegShape, u64)>> {
    let mut head = BufReader::with_capacity(PIECE, &mut *reader);
    let mut byte = [0u8; 1];
    let mut two = [0u8; 2];
    head.read_exact(&mut two)?;
    loop {
        crate::cancel::poll()?;
        // Skip to the next 0xFF, then past its fill bytes to the marker.
        loop {
            if head.read(&mut byte)? == 0 {
                return Ok(None);
            }
            if byte[0] == 0xFF {
                break;
            }
        }
        let marker = loop {
            if head.read(&mut byte)? == 0 {
                return Ok(None);
            }
            if byte[0] != 0xFF {
                break byte[0];
            }
        };
        match marker {
            // A stuffed zero, a standalone marker: no segment follows.
            0x00 | 0x01 | 0xD0..=0xD8 => continue,
            // End of image, start of scan: no frame came first.
            0xD9 | 0xDA => return Ok(None),
            // The frame headers: C0 to CF but the tables and the extension.
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                let mut frame = [0u8; 8];
                if read_head(&mut head, &mut frame)? < frame.len() {
                    return Ok(None);
                }
                let height = u32::from(u16::from_be_bytes([frame[3], frame[4]]));
                let width = u32::from(u16::from_be_bytes([frame[5], frame[6]]));
                if width == 0 || height == 0 {
                    return Ok(None);
                }
                let end = head.stream_position()?;
                let shape = JpegShape { width, height, components: frame[7], placement: (0, 0) };
                return Ok(Some((shape, end)));
            },
            _ => {
                if read_head(&mut head, &mut two)? < 2 {
                    return Ok(None);
                }
                let length = u64::from(u16::from_be_bytes(two));
                if length < 2 {
                    return Ok(None);
                }
                head.seek_relative(length as i64 - 2)?;
            },
        }
    }
}

fn png_decoder<R: Read + Seek>(reader: R) -> png::Decoder<BufReader<R>> {
    let mut decoder = png::Decoder::new(BufReader::with_capacity(PIECE, reader));
    // Palette, `tRNS` and low bit depths expand as the `image` crate expands
    // them, which is how `ImageData::from_png` read the file.
    decoder.set_transformations(png::Transformations::EXPAND);
    decoder
}

/// What the PNG at the start of `reader` decodes to, from the chunks up to
/// its first `IDAT`.
fn png_shape(reader: &mut BoxedReader) -> Option<PngShape> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    let rows = png_decoder(reader).read_info().ok()?;
    let interlaced = rows.info().interlaced;
    let (width, height) = rows.info().size();
    let (color, depth) = rows.output_color_type();
    if width == 0 || height == 0 {
        return None;
    }
    Some(PngShape { width, height, color, depth, interlaced })
}

fn image_error(what: &str, e: impl std::fmt::Display) -> Error {
    Error::Image(format!("{what}: {e}"))
}

/// The objects a streamed image is written as, numbered by the writer.
pub(crate) struct StreamedIds {
    /// The image XObject.
    pub image: u32,
    /// The `/SMask` XObject, when [`StreamedImage::object_count`] has one.
    pub mask: Option<u32>,
    /// The objects that hold the image's and the mask's `/Length`.
    pub lengths: [Option<u32>; 2],
}

impl StreamedImage {
    /// The size `image_pixel_dimensions` reads from the head of the file,
    /// which sizes the page the image is placed on.
    pub(crate) fn pixel_dimensions(&self) -> (u32, u32) {
        match self.shape {
            Shape::Jpeg(s) => s.placement,
            Shape::Png(s) => (s.width, s.height),
        }
    }

    /// The image's own size in pixels, as its XObject says.
    pub(crate) fn size(&self) -> (u32, u32) {
        match self.shape {
            Shape::Jpeg(s) => (s.width, s.height),
            Shape::Png(s) => (s.width, s.height),
        }
    }

    /// Whether the save writes an `/SMask` object.
    pub(crate) fn has_mask(&self) -> bool {
        matches!(self.shape, Shape::Png(s) if s.has_alpha())
    }

    /// Whether the stream's `/Length` is an object of its own.
    pub(crate) fn has_length_objects(&self) -> bool {
        matches!(self.shape, Shape::Png(_))
    }

    /// The `ImageData` that stands for the image when it is written: the
    /// description only, no pixels.
    fn description(&self) -> ImageData {
        let (width, height) = self.size();
        let (color_space, format) = match self.shape {
            Shape::Jpeg(s) => (
                match s.components {
                    1 => ColorSpace::DeviceGray,
                    4 => ColorSpace::DeviceCMYK,
                    _ => ColorSpace::DeviceRGB,
                },
                ImageFormat::Jpeg,
            ),
            Shape::Png(s) => (s.color_space(), ImageFormat::Png),
        };
        ImageData {
            width,
            height,
            bits_per_component: 8,
            color_space,
            format,
            data: Vec::new(),
            soft_mask: self.has_mask().then(Vec::new),
        }
    }

    /// Writes the image's objects to `out`, noting each one's offset in
    /// `offsets`: the image, then its length object and its mask.
    pub(crate) fn write<W: PositionedWrite>(
        &self,
        out: &mut W,
        serializer: &ObjectSerializer,
        ids: &StreamedIds,
        offsets: &mut Vec<(u32, usize)>,
    ) -> Result<()> {
        let description = self.description();
        let mut dict = description.build_xobject_dict();
        match self.shape {
            Shape::Jpeg(_) => {
                dict.insert("Length".to_string(), Object::Integer(self.length as i64));
                let offset = out.position() as usize;
                serializer.write_stream_open(&mut *out, ids.image, 0, &dict)?;
                self.copy_jpeg(&mut *out)?;
                out.write_all(b"\nendstream\nendobj\n")?;
                offsets.push((ids.image, offset));
            },
            Shape::Png(shape) => {
                if let Some(mask) = ids.mask {
                    dict.insert("SMask".to_string(), Object::Reference(ObjectRef::new(mask, 0)));
                }
                let length_id = ids.lengths[0].expect("a PNG's length object is numbered");
                self.write_png_stream(out, serializer, ids.image, length_id, dict, shape, Pass::Colour, offsets)?;
                if let (Some(mask), Some(mask_length)) = (ids.mask, ids.lengths[1]) {
                    let mask_dict = description.build_soft_mask_dict().expect("a mask has a description");
                    self.write_png_stream(out, serializer, mask, mask_length, mask_dict, shape, Pass::Alpha, offsets)?;
                }
            },
        }
        Ok(())
    }

    fn source(&self) -> std::sync::MutexGuard<'_, BoxedReader> {
        self.source.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The JPEG's bytes, source to `out`, a piece at a time.
    fn copy_jpeg(&self, out: &mut dyn Write) -> Result<()> {
        let mut source = self.source();
        source.seek(SeekFrom::Start(0))?;
        let mut piece = vec![0u8; PIECE];
        let mut copied = 0u64;
        while copied < self.length {
            crate::cancel::poll()?;
            let want = piece.len().min((self.length - copied) as usize);
            let n = source.read(&mut piece[..want])?;
            if n == 0 {
                return Err(Error::UnexpectedEof);
            }
            out.write_all(&piece[..n])?;
            copied += n as u64;
        }
        Ok(())
    }

    /// One stream of the PNG: `pass` of its rows, deflated, with `dict` as
    /// the head and `length_id` holding the deflated size.
    #[allow(clippy::too_many_arguments)]
    fn write_png_stream<W: PositionedWrite>(
        &self,
        out: &mut W,
        serializer: &ObjectSerializer,
        id: u32,
        length_id: u32,
        mut dict: std::collections::HashMap<String, Object>,
        shape: PngShape,
        pass: Pass,
        offsets: &mut Vec<(u32, usize)>,
    ) -> Result<()> {
        dict.insert("Length".to_string(), Object::Reference(ObjectRef::new(length_id, 0)));
        let offset = out.position() as usize;
        serializer.write_stream_open(&mut *out, id, 0, &dict)?;
        let start = out.position();
        self.deflate_rows(&mut *out, shape, pass)?;
        let length = out.position() - start;
        out.write_all(b"\nendstream\nendobj\n")?;
        offsets.push((id, offset));

        let offset = out.position() as usize;
        out.write_all(&serializer.serialize_indirect(length_id, 0, &Object::Integer(length as i64)))?;
        offsets.push((length_id, offset));
        Ok(())
    }

    /// The rows of `pass`, each behind a `None` filter byte, to `out` as one
    /// zlib stream. Only the previous row of the decoder and the row being
    /// converted are held; an interlaced image holds those of seven decoders.
    fn deflate_rows(&self, out: &mut dyn Write, shape: PngShape, pass: Pass) -> Result<()> {
        let mut plain_source;
        let mut rows = if shape.interlaced {
            Rows::Adam7(Adam7::open(&self.source, self.length, shape)?)
        } else {
            plain_source = self.source();
            plain_source.seek(SeekFrom::Start(0))?;
            let rows = png_decoder(&mut *plain_source).read_info().map_err(|e| image_error("PNG", e))?;
            check_unchanged(&rows, shape)?;
            Rows::Plain(rows)
        };
        let colours = match pass {
            Pass::Colour => shape.samples() - usize::from(shape.has_alpha()),
            Pass::Alpha => 1,
        };
        let mut zlib =
            flate2::write::ZlibEncoder::new(out, flate2::Compression::best());
        let mut line = Vec::with_capacity(1 + shape.width as usize * colours);
        for _ in 0..shape.height {
            crate::cancel::poll()?;
            let row = rows.next()?;
            line.clear();
            line.push(0);
            narrow_row(row, shape, pass, &mut line);
            zlib.write_all(&line)?;
        }
        zlib.finish()?;
        Ok(())
    }
}

fn check_unchanged<R: std::io::BufRead + Seek>(rows: &png::Reader<R>, shape: PngShape) -> Result<()> {
    if rows.info().size() != (shape.width, shape.height)
        || rows.output_color_type() != (shape.color, shape.depth)
        || rows.info().interlaced != shape.interlaced
    {
        return Err(Error::Image("PNG: the image changed under the save".into()));
    }
    Ok(())
}

/// A handle on the source of an image that several decoders share: a
/// borrow of it while a save writes it, or the image itself when a stamp's
/// stream outlives the op that staged it.
trait ImageSource: std::ops::Deref<Target = Mutex<BoxedReader>> + Clone {}
impl<S: std::ops::Deref<Target = Mutex<BoxedReader>> + Clone> ImageSource for S {}

/// The image a stamp's stream reads, owned by the stream.
#[derive(Clone)]
struct OwnedSource(Arc<StreamedImage>);

impl std::ops::Deref for OwnedSource {
    type Target = Mutex<BoxedReader>;
    fn deref(&self) -> &Mutex<BoxedReader> {
        &self.0.source
    }
}

/// The rows of the image, top to bottom, as the decoder gives them.
enum Rows<R: Read + Seek, S: ImageSource> {
    Plain(png::Reader<BufReader<R>>),
    Adam7(Adam7<S>),
}

impl<R: Read + Seek, S: ImageSource> Rows<R, S> {
    fn next(&mut self) -> Result<&[u8]> {
        match self {
            Rows::Plain(rows) => {
                Ok(rows.next_row().map_err(|e| image_error("PNG", e))?.ok_or(Error::UnexpectedEof)?.data())
            },
            Rows::Adam7(passes) => passes.next(),
        }
    }
}

/// One Adam7 pass: its pixels sit `dx` apart from column `x0` and its rows
/// `dy` apart from row `y0`.
#[derive(Clone, Copy)]
struct PassGrid {
    x0: u32,
    y0: u32,
    dx: u32,
    dy: u32,
    /// Pixels in a row of the pass and rows in it.
    width: u32,
    height: u32,
}

/// The passes of a `width` x `height` image that have a pixel, in order.
fn passes_of(width: u32, height: u32) -> Vec<PassGrid> {
    const GRIDS: [(u32, u32, u32, u32); 7] =
        [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];
    GRIDS
        .into_iter()
        .filter(|&(x0, y0, _, _)| x0 < width && y0 < height)
        .map(|(x0, y0, dx, dy)| PassGrid {
            x0,
            y0,
            dx,
            dy,
            width: (width - x0).div_ceil(dx),
            height: (height - y0).div_ceil(dy),
        })
        .collect()
}

/// A read position of its own in a source that several decoders share: each
/// read seeks to where this decoder left off.
struct Position<S: ImageSource> {
    source: S,
    at: u64,
    length: u64,
}

impl<S: ImageSource> Read for Position<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut source = self.source.lock().unwrap_or_else(|e| e.into_inner());
        source.seek(SeekFrom::Start(self.at))?;
        let n = source.read(buf)?;
        self.at += n as u64;
        Ok(n)
    }
}

impl<S: ImageSource> Seek for Position<S> {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        let at = match to {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(d) => i128::from(self.at) + i128::from(d),
            SeekFrom::End(d) => i128::from(self.length) + i128::from(d),
        };
        self.at = u64::try_from(at)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before the start"))?;
        Ok(self.at)
    }
}

/// A decoder over the whole image, taking only the rows of one pass: the
/// rows of the passes before it are read and dropped, the rows of the passes
/// after it are never read.
struct PassRows<S: ImageSource> {
    rows: png::Reader<BufReader<Position<S>>>,
    grid: PassGrid,
    /// Rows of the earlier passes still to drop.
    skip: u32,
}

impl<S: ImageSource> PassRows<S> {
    /// Puts the pass's next row into `out`, each pixel at its place.
    fn place_next(&mut self, out: &mut [u8], pixel: usize) -> Result<()> {
        while self.skip > 0 {
            crate::cancel::poll()?;
            self.rows.next_interlaced_row().map_err(|e| image_error("PNG", e))?.ok_or(Error::UnexpectedEof)?;
            self.skip -= 1;
        }
        let row = self.rows.next_interlaced_row().map_err(|e| image_error("PNG", e))?.ok_or(Error::UnexpectedEof)?;
        let data = row.data();
        let grid = self.grid;
        if data.len() < grid.width as usize * pixel {
            return Err(Error::Image("PNG: a pass row is shorter than its pixels".into()));
        }
        for (k, from) in data.chunks_exact(pixel).take(grid.width as usize).enumerate() {
            let x = (grid.x0 + k as u32 * grid.dx) as usize;
            out[x * pixel..(x + 1) * pixel].copy_from_slice(from);
        }
        Ok(())
    }
}

/// The rows of an Adam7 image, each put together from the passes that have a
/// pixel in it.
struct Adam7<S: ImageSource> {
    passes: Vec<PassRows<S>>,
    pixel: usize,
    /// The row being put together.
    row: Vec<u8>,
    /// The next row of the image.
    next: u32,
    height: u32,
}

impl<S: ImageSource> Adam7<S> {
    fn open(source: S, length: u64, shape: PngShape) -> Result<Self> {
        let mut passes = Vec::new();
        let mut earlier = 0u32;
        for grid in passes_of(shape.width, shape.height) {
            let position = Position { source: source.clone(), at: 0, length };
            let mut decoder = png::Decoder::new(BufReader::with_capacity(PASS_WINDOW, position));
            decoder.set_transformations(png::Transformations::EXPAND);
            let rows = decoder.read_info().map_err(|e| image_error("PNG", e))?;
            check_unchanged(&rows, shape)?;
            passes.push(PassRows { rows, grid, skip: earlier });
            earlier += grid.height;
        }
        let pixel = shape.pixel_bytes();
        Ok(Self { passes, pixel, row: vec![0; shape.width as usize * pixel], next: 0, height: shape.height })
    }

    fn next(&mut self) -> Result<&[u8]> {
        let y = self.next;
        if y >= self.height {
            return Err(Error::UnexpectedEof);
        }
        for pass in &mut self.passes {
            let grid = pass.grid;
            if y >= grid.y0 && (y - grid.y0) % grid.dy == 0 {
                pass.place_next(&mut self.row, self.pixel)?;
            }
        }
        self.next += 1;
        Ok(&self.row)
    }
}

/// Bytes each pass of an interlaced PNG reads ahead of its decoder.
const PASS_WINDOW: usize = 16 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    /// The colour samples of each pixel.
    Colour,
    /// The alpha sample of each pixel.
    Alpha,
}

/// A 16 bit sample as `image` rounds it to 8 bits.
fn narrow(high: u8, low: u8) -> u8 {
    ((u32::from(u16::from_be_bytes([high, low])) + 128) / 257) as u8
}

/// The 8 bit samples of `pass` out of one decoded row, appended to `line`.
fn narrow_row(row: &[u8], shape: PngShape, pass: Pass, line: &mut Vec<u8>) {
    let samples = shape.samples();
    let bytes = if shape.depth == png::BitDepth::Sixteen { 2 } else { 1 };
    let (first, count) = match (pass, shape.has_alpha()) {
        (Pass::Colour, false) => (0, samples),
        (Pass::Colour, true) => (0, samples - 1),
        (Pass::Alpha, _) => (samples - 1, 1),
    };
    if bytes == 1 && count == samples {
        line.extend_from_slice(&row[..shape.width as usize * samples]);
        return;
    }
    for pixel in row.chunks_exact(samples * bytes).take(shape.width as usize) {
        for sample in first..first + count {
            let at = sample * bytes;
            line.push(if bytes == 1 { pixel[at] } else { narrow(pixel[at], pixel[at + 1]) });
        }
    }
}

/// The streams of an image as an editor stages them: the image XObject's
/// dictionary and data, and its `/SMask`'s when it has one. The dictionaries
/// carry no `/Length` and no `/SMask`: the save states the one, the caller
/// the other.
pub(crate) struct Staged {
    pub image: (std::collections::HashMap<String, Object>, StreamBody),
    pub mask: Option<(std::collections::HashMap<String, Object>, StreamBody)>,
}

/// A reader over the source of a [`StreamedImage`] that keeps its own
/// position, so the streams of one image read it without disturbing each
/// other.
struct Shared {
    image: Arc<StreamedImage>,
    at: u64,
}

impl Read for Shared {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut source = self.image.source();
        source.seek(SeekFrom::Start(self.at))?;
        let n = source.read(buf)?;
        self.at += n as u64;
        Ok(n)
    }
}

impl Seek for Shared {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.at = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(d) => self.at.saturating_add_signed(d),
            SeekFrom::End(d) => self.image.length.saturating_add_signed(d),
        };
        Ok(self.at)
    }
}

/// The rows of one pass of a PNG as the zlib stream `deflate_rows` writes, a
/// reader that decodes and deflates as it is read. Only the decoder's row
/// and the deflater's output so far are held. Seeking back starts the
/// stream over; seeking forward reads and drops.
struct Deflated {
    image: Arc<StreamedImage>,
    shape: PngShape,
    pass: Pass,
    run: Option<Run>,
    /// What the deflater has produced and not yet handed out.
    out: Vec<u8>,
    taken: usize,
    /// Bytes handed out since the start of the stream.
    emitted: u64,
    /// The stream's length, once a pass has measured it.
    total: Option<u64>,
}

struct Run {
    rows: Rows<Shared, OwnedSource>,
    zlib: Option<flate2::write::ZlibEncoder<Vec<u8>>>,
    line: Vec<u8>,
    row: u32,
}

impl Deflated {
    fn new(image: &Arc<StreamedImage>, shape: PngShape, pass: Pass, total: Option<u64>) -> Self {
        Deflated { image: Arc::clone(image), shape, pass, run: None, out: Vec::new(), taken: 0, emitted: 0, total }
    }

    /// The length of the stream, found by deflating it once.
    fn measure(image: &Arc<StreamedImage>, shape: PngShape, pass: Pass) -> Result<u64> {
        let mut counter = crate::source::Counter::default();
        std::io::copy(&mut Deflated::new(image, shape, pass, None), &mut counter)?;
        Ok(counter.0)
    }

    fn open(&self) -> std::io::Result<Run> {
        let rows = if self.shape.interlaced {
            let source = OwnedSource(Arc::clone(&self.image));
            Rows::Adam7(Adam7::open(source, self.image.length, self.shape).map_err(std::io::Error::other)?)
        } else {
            let shared = Shared { image: Arc::clone(&self.image), at: 0 };
            let rows = png_decoder(shared).read_info().map_err(std::io::Error::other)?;
            check_unchanged(&rows, self.shape).map_err(std::io::Error::other)?;
            Rows::Plain(rows)
        };
        Ok(Run {
            rows,
            zlib: Some(flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best())),
            line: Vec::new(),
            row: 0,
        })
    }

    /// Moves the deflater forward by one row, or finishes it.
    fn advance(&mut self) -> std::io::Result<()> {
        crate::cancel::poll_io()?;
        if self.run.is_none() {
            self.run = Some(self.open()?);
        }
        let (shape, pass) = (self.shape, self.pass);
        let run = self.run.as_mut().expect("opened above");
        let Some(zlib) = run.zlib.as_mut() else { return Ok(()) };
        if run.row < shape.height {
            let row = run.rows.next().map_err(std::io::Error::other)?;
            run.line.clear();
            run.line.push(0);
            narrow_row(row, shape, pass, &mut run.line);
            zlib.write_all(&run.line)?;
            run.row += 1;
            self.out = std::mem::take(zlib.get_mut());
        } else {
            self.out = run.zlib.take().expect("checked above").finish()?;
            if let Some(total) = self.total {
                if self.emitted + self.out.len() as u64 != total {
                    return Err(std::io::Error::other("PNG: the image changed under the save"));
                }
            }
        }
        self.taken = 0;
        Ok(())
    }

    fn finished(&self) -> bool {
        self.run.as_ref().is_some_and(|run| run.zlib.is_none())
    }
}

impl Read for Deflated {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.taken == self.out.len() {
            if self.finished() {
                return Ok(0);
            }
            self.advance()?;
        }
        let n = buf.len().min(self.out.len() - self.taken);
        buf[..n].copy_from_slice(&self.out[self.taken..self.taken + n]);
        self.taken += n;
        self.emitted += n as u64;
        Ok(n)
    }
}

impl Seek for Deflated {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(d) => self.emitted.saturating_add_signed(d),
            SeekFrom::End(d) => self
                .total
                .ok_or_else(|| std::io::Error::other("the length of the stream is not known"))?
                .saturating_add_signed(d),
        };
        if target < self.emitted {
            self.run = None;
            self.out = Vec::new();
            self.taken = 0;
            self.emitted = 0;
        }
        let mut dropped = vec![0u8; PIECE];
        while self.emitted < target {
            let want = dropped.len().min((target - self.emitted) as usize);
            if self.read(&mut dropped[..want])? == 0 {
                break;
            }
        }
        Ok(self.emitted)
    }
}

impl StreamedImage {
    /// The image as the streams an editor stages. A PNG is deflated once here
    /// to learn the lengths its streams state, and again, a row at a time,
    /// when the save reads them.
    pub(crate) fn stage(self: &Arc<Self>) -> Result<Staged> {
        let description = self.description();
        let dict = description.build_xobject_dict();
        match self.shape {
            Shape::Jpeg(_) => {
                let reader = Shared { image: Arc::clone(self), at: 0 };
                let body = extent_over(Box::new(reader), self.length);
                Ok(Staged { image: (dict, body), mask: None })
            },
            Shape::Png(shape) => {
                let stream = |pass| -> Result<StreamBody> {
                    let total = Deflated::measure(self, shape, pass)?;
                    let reader = Deflated::new(self, shape, pass, Some(total));
                    Ok(extent_over(Box::new(reader), total))
                };
                let image = (dict, stream(Pass::Colour)?);
                let mask = match description.build_soft_mask_dict() {
                    Some(mask_dict) => Some((mask_dict, stream(Pass::Alpha)?)),
                    None => None,
                };
                Ok(Staged { image, mask })
            },
        }
    }
}

fn extent_over(reader: Box<dyn crate::source::ReadSeek>, len: u64) -> StreamBody {
    let source = Source::over(reader);
    StreamBody::Extent(Extent::new(source, 0, len, ObjectRef::new(0, 0), &std::collections::HashMap::new()))
}

impl From<Vec<u8>> for ImageBytes {
    fn from(bytes: Vec<u8>) -> Self {
        ImageBytes::Whole(bytes)
    }
}
