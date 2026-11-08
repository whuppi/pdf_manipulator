//! Stream filters as readers.
//!
//! This file is fork-only code (NOT upstream).
//!
//! [`reader`] composes the filters of a stream dictionary, in `/Filter`
//! order, over the stream's stored data, and yields the decoded bytes as they
//! are produced: nothing is held beyond a filter's own window (a predictor's
//! one previous row, an LZW table, a deflate window). [`read_whole`] is the
//! one fallible whole read behind every named whole read of a stream.
//!
//! The stored data is given as a factory that opens it from the start, not a
//! single reader, because a Flate stage that meets damage re-reads its input
//! from the start with another strategy (see `flate.rs`).

use super::flate::FlateReader;
use super::predictor::DecodeParams;
use super::normalize_filter_name;
use crate::error::{Error, Result};
use crate::object::Object;
use std::collections::HashMap;
use std::io::{self, Read};
use std::rc::Rc;

/// Opens a stage's input from its first byte.
pub(crate) type Open<'a> = dyn Fn() -> Result<Box<dyn Read + 'a>> + 'a;

/// A decode failure as an `io::Error` that `from_io` turns back into the typed error.
pub(crate) fn failure(e: Error) -> io::Error {
    io::Error::other(e)
}

enum Stage {
    Flate,
    Hex,
    Ascii85,
    Lzw { early_change: bool },
    RunLength,
    Brotli,
    /// An image codec: the bytes as encoded for that codec.
    Codec,
}

struct Spec<'a> {
    stages: Vec<Stage>,
    open: Box<Open<'a>>,
    origin: Option<Origin>,
}

/// Where a stream comes from, for the warnings its decoding raises.
#[derive(Clone)]
pub struct Origin {
    /// Names the stream in a warning ("object 12 0").
    pub label: String,
    /// The document's structured warnings.
    pub sink: std::sync::Arc<crate::extractors::warnings::WarningSink>,
}

impl Origin {
    /// Records `message` against the stream.
    pub(crate) fn warn(&self, message: String) {
        self.sink.push(crate::extractors::warnings::Warning {
            category: crate::extractors::warnings::WarningCategory::SpecViolation,
            page: None,
            message: format!("{} ({})", message, self.label),
            spec_section: Some("7.4.4"),
        });
    }
}

/// The decoded data of the stream described by `dict`, read from `open`.
pub fn reader<'a>(
    dict: &HashMap<String, Object>,
    open: impl Fn() -> Result<Box<dyn Read + 'a>> + 'a,
) -> Result<Box<dyn Read + 'a>> {
    reader_in(dict, None, open)
}

/// [`reader`] for a stream whose origin is known, so that what its decoding
/// recovers from is reported against it.
pub fn reader_in<'a>(
    dict: &HashMap<String, Object>,
    origin: Option<Origin>,
    open: impl Fn() -> Result<Box<dyn Read + 'a>> + 'a,
) -> Result<Box<dyn Read + 'a>> {
    let filters = dict
        .get("Filter")
        .map(crate::object::extract_filter_names)
        .unwrap_or_default();
    let params = crate::object::extract_decode_params(dict.get("DecodeParms"));
    chain_in(&filters, params.as_ref(), origin, open)
}

/// The filters `filters`, in order, over the data `open` yields, then the
/// predictor `params` names. One `DecodeParms` serves every stage, as it
/// does in `decode_stream_with_params`.
pub fn chain<'a>(
    filters: &[String],
    params: Option<&DecodeParams>,
    open: impl Fn() -> Result<Box<dyn Read + 'a>> + 'a,
) -> Result<Box<dyn Read + 'a>> {
    chain_in(filters, params, None, open)
}

/// [`chain`] for a stream whose origin is known.
pub fn chain_in<'a>(
    filters: &[String],
    params: Option<&DecodeParams>,
    origin: Option<Origin>,
    open: impl Fn() -> Result<Box<dyn Read + 'a>> + 'a,
) -> Result<Box<dyn Read + 'a>> {
    let mut stages = Vec::with_capacity(filters.len());
    for name in filters {
        stages.push(match normalize_filter_name(name)? {
            "FlateDecode" => Stage::Flate,
            "ASCIIHexDecode" => Stage::Hex,
            "ASCII85Decode" => Stage::Ascii85,
            "LZWDecode" => Stage::Lzw { early_change: params.map_or(true, |p| p.early_change) },
            "RunLengthDecode" => Stage::RunLength,
            "BrotliDecode" => Stage::Brotli,
            _ => Stage::Codec,
        });
    }
    let spec = Rc::new(Spec { stages, open: Box::new(open), origin });
    let decoded = build(&spec, spec.stages.len())?;
    match params {
        Some(p) if p.predictor != 1 => predictor_reader(decoded, p),
        _ => Ok(decoded),
    }
}

/// The first `upto` stages over a freshly opened input.
fn build<'a>(spec: &Rc<Spec<'a>>, upto: usize) -> Result<Box<dyn Read + 'a>> {
    if upto == 0 {
        return (spec.open)();
    }
    let up = build(spec, upto - 1)?;
    Ok(match spec.stages[upto - 1] {
        Stage::Flate => {
            let again = Rc::clone(spec);
            Box::new(FlateReader::new(
                up,
                Box::new(move || build(&again, upto - 1)),
                spec.origin.clone(),
            ))
        },
        Stage::Hex => Box::new(Pulled::new(Hex { up, high: None })),
        Stage::Ascii85 => Box::new(Pulled::new(Ascii85 { up, acc: 0, count: 0 })),
        Stage::Lzw { early_change } => Box::new(Pulled::new(Lzw::new(up, early_change))),
        Stage::RunLength => Box::new(Pulled::new(RunLength { up, state: RunState::Length })),
        Stage::Brotli => Box::new(BrotliReader(brotli::Decompressor::new(up, 4096))),
        Stage::Codec => up,
    })
}

/// The decoded data of the stream `object`, read as the stream decodes. A
/// dictionary or a null where a stream is expected is an empty stream, as for
/// `Object::decode_stream_data`.
pub(crate) fn decoded_reader(object: &Object) -> Result<Box<dyn Read + '_>> {
    match object {
        Object::Stream { dict, body } => {
            let filtered = dict.get("Filter").map(crate::object::extract_filter_names).unwrap_or_default();
            if filtered.is_empty() {
                body.raw_reader()
            } else {
                reader_in(dict, body.origin(), || body.raw_reader())
            }
        },
        Object::Dictionary(_) | Object::Null => Ok(Box::new(io::empty())),
        other => Err(Error::InvalidObjectType {
            expected: "Stream".to_string(),
            found: other.type_name().to_string(),
        }),
    }
}

/// The first `cap` bytes of the decoded data of the stream `object`, read as
/// the stream decodes: the rest of the stream is never read.
pub(crate) fn read_decoded_window(object: &Object, cap: u64) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    decoded_reader(object)?.take(cap).read_to_end(&mut out).map_err(crate::encryption::stream::from_io)?;
    Ok(out)
}

/// Reads `r` to its end into one buffer, growing it in 64 KiB steps. A
/// refused allocation is a typed decode error, not an abort. `hint` is the
/// size to expect (0 when unknown) and is reserved first.
pub fn read_whole(r: impl Read, hint: usize) -> Result<Vec<u8>> {
    let mut out = read_whole_loose(r, hint)?;
    // The last 64 KiB step is mostly empty for a small stream, and a caller
    // that caches the buffer charges its length, not its capacity. A large
    // stream's spare is under a step, which is below the eighth that is kept.
    if out.capacity() - out.len() > out.len() / 8 {
        out.shrink_to_fit();
    }
    Ok(out)
}

fn read_whole_loose(mut r: impl Read, hint: usize) -> Result<Vec<u8>> {
    const STEP: usize = 64 * 1024;
    let mut out: Vec<u8> = Vec::new();
    let refused = |wanted: usize| {
        Error::Decode(format!("could not allocate {wanted} bytes to hold the decoded stream"))
    };
    if hint > 0 {
        reserve(&mut out, hint, true).map_err(|_| refused(hint))?;
    }
    loop {
        let spare = out.capacity() - out.len();
        if spare == 0 {
            // Full: a small probe says whether anything is left before the
            // buffer grows, so a hint that was exact costs no second allocation.
            let mut probe = [0u8; 256];
            let n = match r.read(&mut probe) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(crate::encryption::stream::from_io(e)),
                Ok(n) => n,
            };
            if n == 0 {
                return Ok(out);
            }
            reserve(&mut out, STEP, false).map_err(|_| refused(out.len() + STEP))?;
            out.extend_from_slice(&probe[..n]);
            continue;
        }
        // The spare capacity is zeroed in place before the read, so no
        // uninitialised byte is ever offered to the reader.
        let at = out.len();
        out.resize(at + spare.min(STEP), 0);
        match r.read(&mut out[at..]) {
            Ok(0) => {
                out.truncate(at);
                return Ok(out);
            },
            Ok(n) => out.truncate(at + n),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => out.truncate(at),
            Err(e) => return Err(crate::encryption::stream::from_io(e)),
        }
    }
}

fn reserve(out: &mut Vec<u8>, extra: usize, exact: bool) -> std::result::Result<(), ()> {
    #[cfg(test)]
    if let Some(limit) = TEST_ALLOCATION_LIMIT.with(|l| l.get()) {
        if out.len() + extra > limit {
            return Err(());
        }
    }
    let grown = if exact { out.try_reserve_exact(extra) } else { out.try_reserve(extra) };
    grown.map_err(|_| ())
}

#[cfg(test)]
thread_local! {
    /// The most bytes `read_whole` may hold on this thread; a test sets it to
    /// stand in for an allocator that refuses.
    pub(crate) static TEST_ALLOCATION_LIMIT: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

/// Runs the filters `filters` over `input` and reads the result to the end.
/// A test build also runs it with the input handed over in pieces of 1 to
/// 65536 bytes and requires the same result each time, so every decoder
/// test that goes through here proves the reader and the whole read agree.
pub(crate) fn decode_slice<S: AsRef<str>>(
    input: &[u8],
    filters: &[S],
    params: Option<&DecodeParams>,
) -> Result<Vec<u8>> {
    let names: Vec<String> = filters.iter().map(|f| f.as_ref().to_string()).collect();
    let run = |piece: usize| -> Result<Vec<u8>> {
        let source = move || -> Result<Box<dyn Read + '_>> { Ok(Box::new(Pieces { rest: input, piece })) };
        read_whole(chain(&names, params, source)?, 0)
    };
    let whole = run(usize::MAX);
    #[cfg(test)]
    for piece in [1usize, 2, 3, 7, 64, 1024, 65536] {
        if piece < 64 && input.len() > 64 * 1024 {
            continue;
        }
        match (&whole, run(piece)) {
            (Ok(a), Ok(b)) => assert_eq!(a, &b, "pieces of {piece}"),
            (Err(_), Err(_)) => {},
            (a, b) => panic!("pieces of {piece}: whole {a:?}, pieces {b:?}"),
        }
    }
    whole
}

/// A slice that yields at most `piece` bytes a read.
pub(crate) struct Pieces<'a> {
    pub(crate) rest: &'a [u8],
    pub(crate) piece: usize,
}

impl Read for Pieces<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = buf.len().min(self.piece).min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

/// A filter that turns a chunk of its input into output.
trait Refill {
    /// Appends what the next chunk of input decodes to; `Ok(false)` when the
    /// stream has ended (the call may still have appended its last bytes).
    fn refill(&mut self, out: &mut Vec<u8>) -> io::Result<bool>;
}

/// Serves a [`Refill`] as a reader. When a refill fails after producing
/// bytes, the bytes are served first and the error next.
struct Pulled<T> {
    filter: T,
    buf: Vec<u8>,
    at: usize,
    ended: bool,
    error: Option<io::Error>,
}

impl<T: Refill> Pulled<T> {
    fn new(filter: T) -> Self {
        Self { filter, buf: Vec::new(), at: 0, ended: false, error: None }
    }
}

impl<T: Refill> Read for Pulled<T> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.at < self.buf.len() {
                let n = out.len().min(self.buf.len() - self.at);
                out[..n].copy_from_slice(&self.buf[self.at..self.at + n]);
                self.at += n;
                return Ok(n);
            }
            if let Some(e) = self.error.take() {
                self.ended = true;
                return Err(e);
            }
            if self.ended || out.is_empty() {
                return Ok(0);
            }
            self.buf.clear();
            self.at = 0;
            match self.filter.refill(&mut self.buf) {
                Ok(more) => self.ended = !more,
                Err(e) => self.error = Some(e),
            }
        }
    }
}

const CHUNK: usize = 8 * 1024;

/// One read of up to a chunk from `up` into `scratch`.
fn pull(up: &mut dyn Read, scratch: &mut [u8; CHUNK]) -> io::Result<usize> {
    loop {
        match up.read(scratch) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            other => return other,
        }
    }
}

// ── ASCIIHexDecode ──

struct Hex<'a> {
    up: Box<dyn Read + 'a>,
    high: Option<u8>,
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

impl Refill for Hex<'_> {
    fn refill(&mut self, out: &mut Vec<u8>) -> io::Result<bool> {
        let mut scratch = [0u8; CHUNK];
        let n = pull(&mut *self.up, &mut scratch)?;
        if n == 0 {
            // An odd last digit is the high nibble of a byte whose low nibble is 0.
            if let Some(high) = self.high.take() {
                out.push(high << 4);
            }
            return Ok(false);
        }
        for &c in &scratch[..n] {
            if c.is_ascii_whitespace() || c == b'>' {
                continue;
            }
            let v = hex_value(c).ok_or_else(|| {
                failure(Error::Decode(format!("ASCIIHexDecode: invalid hex digit '{}'", c as char)))
            })?;
            match self.high.take() {
                None => self.high = Some(v),
                Some(high) => out.push((high << 4) | v),
            }
        }
        Ok(true)
    }
}

// ── ASCII85Decode ──

struct Ascii85<'a> {
    up: Box<dyn Read + 'a>,
    acc: u32,
    count: usize,
}

impl Ascii85<'_> {
    /// The partial group at the end of the data.
    fn finish(&mut self, out: &mut Vec<u8>) -> io::Result<()> {
        if self.count == 0 {
            return Ok(());
        }
        if self.count == 1 {
            return Err(failure(Error::Decode(
                "ASCII85Decode: incomplete group (need at least 2 characters)".to_string(),
            )));
        }
        let mut acc = self.acc;
        for _ in self.count..5 {
            acc = acc.checked_mul(85).and_then(|v| v.checked_add(84)).ok_or_else(|| {
                failure(Error::Decode("ASCII85Decode: overflow in padding".to_string()))
            })?;
        }
        out.extend_from_slice(&acc.to_be_bytes()[..self.count - 1]);
        self.count = 0;
        Ok(())
    }
}

impl Refill for Ascii85<'_> {
    fn refill(&mut self, out: &mut Vec<u8>) -> io::Result<bool> {
        let mut scratch = [0u8; CHUNK];
        let n = pull(&mut *self.up, &mut scratch)?;
        if n == 0 {
            self.finish(out)?;
            return Ok(false);
        }
        for &byte in &scratch[..n] {
            match byte {
                // The end marker `~>`: nothing after it is read.
                b'~' => {
                    self.finish(out)?;
                    return Ok(false);
                },
                b'z' => {
                    if self.count != 0 {
                        return Err(failure(Error::Decode(
                            "ASCII85Decode: 'z' must not appear in the middle of a group"
                                .to_string(),
                        )));
                    }
                    out.extend_from_slice(&[0, 0, 0, 0]);
                },
                b'!'..=b'u' => {
                    self.acc = self
                        .acc
                        .checked_mul(85)
                        .and_then(|v| v.checked_add((byte - b'!') as u32))
                        .ok_or_else(|| {
                            failure(Error::Decode("ASCII85Decode: overflow in decoding".to_string()))
                        })?;
                    self.count += 1;
                    if self.count == 5 {
                        out.extend_from_slice(&self.acc.to_be_bytes());
                        self.acc = 0;
                        self.count = 0;
                    }
                },
                _ if byte.is_ascii_whitespace() => {},
                _ => {
                    return Err(failure(Error::Decode(format!(
                        "ASCII85Decode: invalid character '{}'",
                        byte as char
                    ))));
                },
            }
        }
        Ok(true)
    }
}

// ── RunLengthDecode ──

enum RunState {
    Length,
    Literal(usize),
    Run(usize),
}

struct RunLength<'a> {
    up: Box<dyn Read + 'a>,
    state: RunState,
}

impl Refill for RunLength<'_> {
    fn refill(&mut self, out: &mut Vec<u8>) -> io::Result<bool> {
        let mut scratch = [0u8; CHUNK];
        let n = pull(&mut *self.up, &mut scratch)?;
        if n == 0 {
            return match self.state {
                RunState::Length => Ok(false),
                RunState::Literal(left) => Err(failure(Error::Decode(format!(
                    "RunLengthDecode: not enough data for literal run ({left} bytes missing)"
                )))),
                RunState::Run(_) => Err(failure(Error::Decode(
                    "RunLengthDecode: missing byte for run".to_string(),
                ))),
            };
        }
        let mut i = 0;
        while i < n {
            match self.state {
                RunState::Length => {
                    let length = scratch[i];
                    i += 1;
                    match length {
                        0..=127 => self.state = RunState::Literal(length as usize + 1),
                        128 => return Ok(false),
                        129..=255 => self.state = RunState::Run(257 - length as usize),
                    }
                },
                RunState::Literal(left) => {
                    let take = left.min(n - i);
                    out.extend_from_slice(&scratch[i..i + take]);
                    i += take;
                    self.state =
                        if take == left { RunState::Length } else { RunState::Literal(left - take) };
                },
                RunState::Run(count) => {
                    let byte = scratch[i];
                    i += 1;
                    out.resize(out.len() + count, byte);
                    self.state = RunState::Length;
                },
            }
        }
        Ok(true)
    }
}

// ── LZWDecode ──

struct Lzw<'a> {
    up: Box<dyn Read + 'a>,
    decoder: weezl::decode::Decoder,
    input_ended: bool,
}

impl<'a> Lzw<'a> {
    fn new(up: Box<dyn Read + 'a>, early_change: bool) -> Self {
        use weezl::{decode::Decoder, BitOrder};
        // The code width grows one code early (TIFF size switch) with
        // `/EarlyChange 1` and at the GIF boundary with `/EarlyChange 0`;
        // reading one as the other decodes the wrong bytes without an error.
        let decoder = if early_change {
            Decoder::with_tiff_size_switch(BitOrder::Msb, 8)
        } else {
            Decoder::new(BitOrder::Msb, 8)
        };
        Self { up, decoder, input_ended: false }
    }

    fn decode_error(e: impl std::fmt::Debug) -> io::Error {
        failure(Error::Decode(format!("LZWDecode error: {:?}", e)))
    }
}

impl Refill for Lzw<'_> {
    fn refill(&mut self, out: &mut Vec<u8>) -> io::Result<bool> {
        use weezl::LzwStatus;
        let mut scratch = [0u8; CHUNK];
        let n = if self.input_ended { 0 } else { pull(&mut *self.up, &mut scratch)? };
        self.input_ended = n == 0;
        let mut input = &scratch[..n];
        let mut room = [0u8; CHUNK];
        loop {
            let step = self.decoder.decode_bytes(input, &mut room);
            out.extend_from_slice(&room[..step.consumed_out]);
            input = &input[step.consumed_in..];
            match step.status.map_err(Self::decode_error)? {
                LzwStatus::Done => return Ok(false),
                LzwStatus::Ok => {
                    if step.consumed_in == 0 && step.consumed_out == 0 {
                        break;
                    }
                },
                LzwStatus::NoProgress => break,
            }
        }
        if self.input_ended {
            // The stream stopped without its end code.
            return Err(Self::decode_error("no end code"));
        }
        Ok(true)
    }
}

// ── BrotliDecode ──

struct BrotliReader<R: Read>(brotli::Decompressor<R>);

impl<R: Read> Read for BrotliReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0
            .read(buf)
            .map_err(|e| failure(Error::Decode(format!("BrotliDecode: {}", e))))
    }
}

// ── predictors ──

/// A predictor as a row filter: it holds the one row above the row it decodes.
struct Predictor<'a> {
    up: std::io::BufReader<Box<dyn Read + 'a>>,
    png: bool,
    /// Stored bytes of a row, including the tag byte of a PNG row.
    row_in: usize,
    /// Decoded bytes of a row.
    pixel_bytes: usize,
    colors: usize,
    bits_per_component: usize,
    /// Components in a row.
    samples: usize,
    /// The PNG byte distance of a pixel.
    bpp: usize,
    /// The row above, then the row being decoded (PNG only).
    window: Vec<u8>,
    row: Vec<u8>,
}

/// The data of `up` with the predictor `params` names reversed.
fn predictor_reader<'a>(up: Box<dyn Read + 'a>, params: &DecodeParams) -> Result<Box<dyn Read + 'a>> {
    let png = match params.predictor {
        2 => false,
        10..=15 => true,
        other => return Err(Error::Decode(format!("Unsupported predictor: {other}"))),
    };
    Ok(Box::new(Pulled::new(Predictor {
        up: std::io::BufReader::with_capacity(CHUNK, up),
        png,
        row_in: if png { params.bytes_per_row() } else { params.pixel_bytes_per_row() },
        pixel_bytes: params.pixel_bytes_per_row(),
        colors: params.colors,
        bits_per_component: params.bits_per_component,
        samples: params.columns * params.colors,
        bpp: params.bytes_per_pixel(),
        window: Vec::new(),
        row: Vec::new(),
    })))
}

impl Predictor<'_> {
    /// Reads the next row of stored bytes into `self.row`; `false` at a clean end.
    fn next_row(&mut self) -> io::Result<bool> {
        self.row.clear();
        self.row.resize(self.row_in.max(1), 0);
        let mut have = 0;
        while have < self.row.len() {
            match self.up.read(&mut self.row[have..]) {
                Ok(0) => break,
                Ok(n) => have += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                Err(e) => return Err(e),
            }
        }
        if have == 0 {
            return Ok(false);
        }
        if self.row_in == 0 || have < self.row_in {
            return Err(failure(Error::Decode(format!(
                "Data length is not a multiple of row size {}",
                self.row_in
            ))));
        }
        Ok(true)
    }
}

impl Refill for Predictor<'_> {
    fn refill(&mut self, out: &mut Vec<u8>) -> io::Result<bool> {
        while out.len() < CHUNK {
            if !self.next_row()? {
                return Ok(false);
            }
            if self.png {
                let above = self.window.len();
                // The row above sits at the front of the window, so the row
                // algorithms see it as row 1; the first row has none.
                let row_idx = usize::from(above > 0);
                super::predictor::png_row(
                    self.row[0],
                    &self.row[1..],
                    &mut self.window,
                    row_idx,
                    self.pixel_bytes,
                    // ── pdf_manipulator patch: the PNG byte distance and the TIFF depth reach the row functions ──
                    self.bpp,
                    // ── end pdf_manipulator patch ──
                )
                .map_err(failure)?;
                out.extend_from_slice(&self.window[above..]);
                self.window.drain(..above);
            } else {
                super::predictor::tiff_row(
                    &self.row,
                    self.colors,
                    self.bits_per_component,
                    self.samples,
                    out,
                );
            }
        }
        Ok(true)
    }
}

/// A test build's check that the predictor reader gives the whole decode's result at every piece size.
#[cfg(test)]
pub(crate) fn assert_predictor_reader_agrees(
    data: &[u8],
    params: &DecodeParams,
    whole: &Result<Vec<u8>>,
) {
    for piece in [1usize, 2, 3, 7, 64, 1024, 65536] {
        if piece < 64 && data.len() > 64 * 1024 {
            continue;
        }
        let source = move || -> Result<Box<dyn Read + '_>> { Ok(Box::new(Pieces { rest: data, piece })) };
        let again = chain(&[], Some(params), source).and_then(|r| read_whole(r, 0));
        match (whole, again) {
            (Ok(a), Ok(b)) => assert_eq!(a, &b, "predictor pieces of {piece}"),
            (Err(_), Err(_)) => {},
            (a, b) => panic!("predictor pieces of {piece}: whole {a:?}, reader {b:?}"),
        }
    }
}
