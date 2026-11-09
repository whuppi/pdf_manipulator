//! FlateDecode (zlib/deflate) implementation.
//!
//! This is the most common PDF compression filter, used in ~90% of PDFs.
//! Uses the flate2 crate for zlib decompression.

#![forbid(unsafe_code)]

// ── pdf_manipulator patch: FlateDecode is a reader that recovers from damage in a probe window ──
use crate::decoders::stream::{decode_slice, failure, Origin};
use crate::decoders::StreamDecoder;
use crate::error::{Error, Result};
use flate2::{Decompress, FlushDecompress, Status};
use std::io::{self, Read};

/// A deflate stream that simply *stops* — no final block marker — rather than
/// one whose bits are corrupt.
///
/// flate2 1.1.10 began rejecting these outright ("Reject incomplete deflate
/// streams at EOF", flate2-rs#556); 1.1.9 returned whatever it had decoded.
/// Real PDFs are full of them: an incremental-update xref stream in the pdf.js
/// corpus is 51 compressed bytes that decode to 70 and never terminate, and
/// zlib, poppler and pdf.js all hand back those 70 bytes. Truncation cannot
/// corrupt the prefix — it only cuts it short — so the bytes decoded before the
/// end are exactly what a tolerant reader produces.
pub(crate) fn is_truncation(e: &io::Error) -> bool {
    if e.kind() == io::ErrorKind::UnexpectedEof {
        return true;
    }
    // zlib-rs / miniz_oxide report it in the message rather than the kind.
    let msg = e.to_string();
    msg.contains("incomplete deflate stream") || msg.contains("unexpected end")
}

/// Heuristic validator for partial-recovery output from a failing decompress.
///
/// Returns `true` if the decoded bytes look like a plausible PDF stream — any
/// of: a content-stream operator (BT/ET/Tj/TJ/Tm/Td), a common PDF object
/// marker (<</>>/stream/obj/endobj), or a `%PDF-` prefix. The set is kept
/// intentionally broad because FlateDecode also wraps object streams, xref
/// streams, font programs, and image data, none of which carry text operators.
///
/// The guard exists to distinguish genuine partially-decoded output from
/// `deflate` "success" on a misaligned input — the latter produces short runs
/// of pseudo-random bytes (#364 symptom: 128 bytes of `P\xffj!}` repeating)
/// that contain none of these markers.
///
/// Conservative fallback: if the decoded bytes are mostly ASCII (printable
/// plus whitespace), the output is also treated as plausible, because stream
/// contents in the wild include ASCII-only data (hex-encoded images, small
/// object streams) that do not hit any specific marker.
#[cfg(test)]
pub(crate) fn looks_like_real_stream(output: &[u8]) -> bool {
    let mut tracker = Tracker::default();
    tracker.feed(output);
    tracker.real()
}

const MARKERS: &[&[u8]] =
    &[b"BT", b"ET", b"Tj", b"TJ", b"Tm", b"Td", b"stream", b"endobj", b"%PDF-"];
const OPERATORS: &[&[u8]] = &[b"BT", b"ET", b"Tj", b"TJ", b"Tm", b"Td"];

/// What the recovery tests ask of the output of a decode, kept as the output
/// goes by: `looks_like_real_stream` (a marker anywhere, else at least 80%
/// printable) and the content-stream operators strategy 5 requires.
#[derive(Default)]
struct Tracker {
    total: u64,
    printable: u64,
    marker_seen: bool,
    operator_seen: bool,
    /// The last bytes seen, so a marker split across two feeds is found.
    carry: Vec<u8>,
}

impl Tracker {
    fn feed(&mut self, bytes: &[u8]) {
        const LONGEST: usize = 6;
        if !self.marker_seen || !self.operator_seen {
            let edge = [&self.carry[..], &bytes[..bytes.len().min(LONGEST - 1)]].concat();
            let found = |needles: &[&[u8]], hay: &[u8]| {
                needles.iter().any(|n| hay.windows(n.len()).any(|w| w == *n))
            };
            if !self.marker_seen && (found(MARKERS, &edge) || found(MARKERS, bytes)) {
                self.marker_seen = true;
            }
            if !self.operator_seen && (found(OPERATORS, &edge) || found(OPERATORS, bytes)) {
                self.operator_seen = true;
            }
        }
        self.total += bytes.len() as u64;
        if !self.marker_seen {
            self.printable += bytes
                .iter()
                .filter(|&&b| (0x20..=0x7E).contains(&b) || b == b'\t' || b == b'\n' || b == b'\r')
                .count() as u64;
        }
        self.carry.extend_from_slice(&bytes[bytes.len().saturating_sub(LONGEST - 1)..]);
        let keep = self.carry.len().saturating_sub(LONGEST - 1);
        self.carry.drain(..keep);
    }

    fn nonempty(&self) -> bool {
        self.total > 0
    }

    fn real(&self) -> bool {
        self.nonempty() && (self.marker_seen || self.printable * 5 >= self.total * 4)
    }
}

/// FlateDecode filter implementation.
///
/// Decompresses data using the zlib/deflate algorithm. The decoded data is
/// not capped: the consumer's buffer is the bound.
#[derive(Default)]
pub struct FlateDecoder;

impl StreamDecoder for FlateDecoder {
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>> {
        decode_slice(input, &["FlateDecode"], None)
    }

    fn name(&self) -> &str {
        "FlateDecode"
    }
}

/// The end of a zlib stream's deflate data was reached and only the
/// Adler-32 check failed: every byte of the data was produced.
#[derive(Debug)]
struct ChecksumMismatch;

impl std::fmt::Display for ChecksumMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("zlib checksum mismatch")
    }
}

impl std::error::Error for ChecksumMismatch {}

fn is_checksum_mismatch(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|inner| inner.is::<ChecksumMismatch>())
}

/// Adler-32 (RFC 1950), kept as the data goes by.
struct Adler {
    a: u32,
    b: u32,
}

impl Adler {
    fn new() -> Self {
        Self { a: 1, b: 0 }
    }

    fn update(&mut self, data: &[u8]) {
        // 5552 bytes keep `b` inside a u32 before the reduction.
        for chunk in data.chunks(5552) {
            for &byte in chunk {
                self.a += byte as u32;
                self.b += self.a;
            }
            self.a %= 65521;
            self.b %= 65521;
        }
    }

    fn value(&self) -> u32 {
        (self.b << 16) | self.a
    }
}

/// Where an inflater is in its stream.
enum Phase {
    /// The two zlib header bytes, not yet read.
    Header,
    Data,
    /// The four Adler-32 bytes after the deflate data.
    Trailer,
    Over,
}

/// An inflater that never drops decoded bytes: the bytes a call produces
/// before it meets damage are returned, and the damage on the next call, so
/// what is recovered does not depend on how the input arrives in pieces.
///
/// It runs raw deflate and checks the zlib header and Adler-32 itself, so a
/// failed checksum is told apart from damage inside the deflate data.
struct Inflate<'a> {
    raw: Box<dyn Read + 'a>,
    inflater: Decompress,
    /// `Some` for a zlib stream: the check kept as the data goes by.
    adler: Option<Adler>,
    phase: Phase,
    input: Vec<u8>,
    at: usize,
    end: usize,
    input_ended: bool,
    failure: Option<io::Error>,
}

impl<'a> Inflate<'a> {
    /// `zlib` selects the zlib wrapper; without it the data is raw deflate.
    fn new(raw: Box<dyn Read + 'a>, zlib: bool) -> Self {
        Self {
            raw,
            inflater: Decompress::new(false),
            adler: zlib.then(Adler::new),
            phase: if zlib { Phase::Header } else { Phase::Data },
            input: vec![0u8; 16 * 1024],
            at: 0,
            end: 0,
            input_ended: false,
            failure: None,
        }
    }

    /// Makes at least one input byte available; `false` at the end of the input.
    fn fill(&mut self) -> io::Result<bool> {
        while self.at == self.end {
            if self.input_ended {
                return Ok(false);
            }
            self.at = 0;
            self.end = match self.raw.read(&mut self.input) {
                Ok(0) => {
                    self.input_ended = true;
                    0
                },
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => 0,
                Err(e) => return Err(e),
            };
        }
        Ok(true)
    }

    fn next_byte(&mut self) -> io::Result<Option<u8>> {
        if !self.fill()? {
            return Ok(None);
        }
        self.at += 1;
        Ok(Some(self.input[self.at - 1]))
    }

    fn incomplete() -> io::Error {
        io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete deflate stream")
    }

    fn read_header(&mut self) -> io::Result<()> {
        let (Some(cmf), Some(flg)) = (self.next_byte()?, self.next_byte()?) else {
            return Err(Self::incomplete());
        };
        let bad = (cmf & 0x0F) != 8
            || (cmf >> 4) > 7
            || ((cmf as u32) << 8 | flg as u32) % 31 != 0
            || flg & 0x20 != 0;
        if bad {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid zlib header"));
        }
        Ok(())
    }

    fn check_trailer(&mut self) -> io::Result<()> {
        let mut stored = [0u8; 4];
        for byte in stored.iter_mut() {
            *byte = self.next_byte()?.ok_or_else(Self::incomplete)?;
        }
        let expected = self.adler.as_ref().map_or(0, Adler::value);
        if u32::from_be_bytes(stored) != expected {
            return Err(io::Error::new(io::ErrorKind::InvalidData, ChecksumMismatch));
        }
        Ok(())
    }
}

impl Read for Inflate<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if let Some(e) = self.failure.take() {
                self.phase = Phase::Over;
                return Err(e);
            }
            match self.phase {
                Phase::Over => return Ok(0),
                Phase::Header => {
                    self.read_header()?;
                    self.phase = Phase::Data;
                    continue;
                },
                Phase::Trailer => {
                    self.phase = Phase::Over;
                    self.check_trailer()?;
                    return Ok(0);
                },
                Phase::Data => {},
            }
            self.fill()?;
            let (in_before, out_before) = (self.inflater.total_in(), self.inflater.total_out());
            let flush = if self.input_ended { FlushDecompress::Finish } else { FlushDecompress::None };
            let step = self.inflater.decompress(&self.input[self.at..self.end], out, flush);
            self.at += (self.inflater.total_in() - in_before) as usize;
            let produced = (self.inflater.total_out() - out_before) as usize;
            if let Some(adler) = self.adler.as_mut() {
                adler.update(&out[..produced]);
            }
            match step {
                Ok(Status::StreamEnd) => {
                    // Raw deflate ends here; a zlib stream still owes its checksum.
                    self.phase = if self.adler.is_some() { Phase::Trailer } else { Phase::Over };
                    if produced > 0 {
                        return Ok(produced);
                    }
                },
                Ok(_) => {
                    if produced > 0 {
                        return Ok(produced);
                    }
                    if self.input_ended && self.at == self.end {
                        self.failure = Some(Self::incomplete());
                    }
                },
                Err(e) => {
                    let damage = io::Error::new(io::ErrorKind::InvalidData, e.to_string());
                    if produced > 0 {
                        self.failure = Some(damage);
                        return Ok(produced);
                    }
                    self.phase = Phase::Over;
                    return Err(damage);
                },
            }
        }
    }
}

/// `first` bytes, then `rest`.
struct Prefixed<'a> {
    head: [u8; 2],
    len: usize,
    at: usize,
    rest: Box<dyn Read + 'a>,
}

impl Read for Prefixed<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.at < self.len {
            let n = out.len().min(self.len - self.at);
            out[..n].copy_from_slice(&self.head[self.at..self.at + n]);
            self.at += n;
            return Ok(n);
        }
        self.rest.read(out)
    }
}

/// Up to `want` bytes from `r`; fewer only at its end.
fn take_bytes(r: &mut dyn Read, want: usize) -> io::Result<Vec<u8>> {
    let mut got = Vec::with_capacity(want);
    let mut one = [0u8; 1];
    while got.len() < want {
        match r.read(&mut one) {
            Ok(0) => break,
            Ok(_) => got.push(one[0]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

/// What each way of reading the stream accepts, as the whole-buffer
/// decoder judged it.
#[derive(Clone, Copy, PartialEq)]
enum Rule {
    /// Zlib and raw deflate: a clean end, or damage after plausible output or a truncation.
    Plain,
    /// Deflate after the first two bytes: a clean end, or damage after plausible output.
    Skipped,
    /// Zlib with a corrected header byte: output, whole or damaged-but-plausible.
    Corrected,
    /// Deflate from a later offset: output that carries content-stream operators.
    Scanned,
}

impl Rule {
    fn accepts(self, t: &Tracker, damage: Option<&io::Error>) -> bool {
        match (self, damage) {
            (Rule::Plain, None) | (Rule::Skipped, None) => true,
            (Rule::Plain, Some(e)) => t.nonempty() && (is_truncation(e) || t.real()),
            (Rule::Skipped, Some(_)) | (Rule::Corrected, Some(_)) => t.nonempty() && t.real(),
            (Rule::Corrected, None) => t.nonempty(),
            (Rule::Scanned, _) => t.nonempty() && t.operator_seen,
        }
    }
}

/// Which way of reading the stream is being tried.
#[derive(Clone, Copy)]
enum Way {
    Zlib,
    Deflate,
    DeflateAfter(usize),
    CorrectedHeader,
}

/// The decoded output the reader serves.
enum State<'a> {
    Start,
    /// All the output, decided within the probe window.
    Held { buf: Vec<u8>, at: usize },
    /// The output goes on from a live decoder after `prefix`.
    Live(Box<Live<'a>>),
    Done,
}

struct Live<'a> {
    decoder: Box<dyn Read + 'a>,
    prefix: Vec<u8>,
    at: usize,
    tracker: Tracker,
    /// Damage after the window is judged as strategy 1 judges it; otherwise
    /// the way was already accepted whole and damage ends the data.
    judge: bool,
}

/// The decoded output held back before it is served; damage inside this
/// window can still be recovered from by another strategy.
const PROBE: usize = 64 * 1024;

/// FlateDecode as a reader.
///
/// The whole-buffer decoder tried zlib, then raw deflate, deflate after
/// skipping two bytes, zlib with a corrected header byte, and raw deflate from
/// offsets up to 19, each from the start of the input, and judged each
/// result by the whole output. A reader cannot take back bytes it served, so
/// it holds back the first [`PROBE`] bytes of each attempt: damage inside the
/// window runs the next strategy exactly as before (each over a reopened
/// input). An attempt that outgrows the window is first run through
/// uncounted, to learn how it ends and what its output looks like, and then
/// served; the first strategy instead streams on, and damage after the
/// window is judged by the same test, kept as the output goes by.
pub(crate) struct FlateReader<'a> {
    first: Option<Box<dyn Read + 'a>>,
    reopen: Box<dyn Fn() -> Result<Box<dyn Read + 'a>> + 'a>,
    state: State<'a>,
    /// Names the stream in what the reader reports.
    origin: Option<Origin>,
}

impl<'a> FlateReader<'a> {
    pub(crate) fn new(
        first: Box<dyn Read + 'a>,
        reopen: Box<dyn Fn() -> Result<Box<dyn Read + 'a>> + 'a>,
        origin: Option<Origin>,
    ) -> Self {
        Self { first: Some(first), reopen, state: State::Start, origin }
    }

    /// A decoder for `way` over a reopened input, or `None` when the input
    /// is too short for that way to apply.
    fn open(&self, way: Way) -> Result<Option<Box<dyn Read + 'a>>> {
        let mut raw = (self.reopen)()?;
        let io = Error::Io;
        Ok(match way {
            Way::Zlib => Some(Box::new(Inflate::new(raw, true))),
            Way::Deflate => Some(Box::new(Inflate::new(raw, false))),
            Way::DeflateAfter(skip) => {
                // `input[skip..]` must hold at least one byte; the one read
                // to check belongs to the data, so it goes back in front.
                if take_bytes(&mut *raw, skip).map_err(io)?.len() < skip {
                    return Ok(None);
                }
                let next = take_bytes(&mut *raw, 1).map_err(io)?;
                if next.is_empty() {
                    return Ok(None);
                }
                Some(Box::new(Inflate::new(
                    Box::new(Prefixed { head: [next[0], 0], len: 1, at: 0, rest: raw }),
                    false,
                )))
            },
            Way::CorrectedHeader => {
                let head = take_bytes(&mut *raw, 2).map_err(io)?;
                if head.len() < 2 || head[0] & 0x0F == 8 {
                    return Ok(None);
                }
                let corrected = [(head[0] & 0xF0) | 0x08, head[1]];
                Some(Box::new(Inflate::new(
                    Box::new(Prefixed { head: corrected, len: 2, at: 0, rest: raw }),
                    true,
                )))
            },
        })
    }

    /// Runs one way to a decision.
    fn attempt(
        &self,
        decoder: Box<dyn Read + 'a>,
        way: Way,
        rule: Rule,
        damage_out: &mut Option<String>,
    ) -> Result<Option<State<'a>>> {
        let mut decoder = decoder;
        let mut buf = Vec::new();
        let mut chunk = vec![0u8; 8 * 1024];
        let mut damage: Option<io::Error> = None;
        while buf.len() < PROBE {
            match decoder.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e) => {
                    damage = Some(e);
                    break;
                },
            }
        }
        let mut tracker = Tracker::default();
        tracker.feed(&buf);
        if damage.is_some() || buf.len() < PROBE {
            if rule.accepts(&tracker, damage.as_ref()) {
                if damage.is_some() {
                    log::warn!("FlateDecode partial recovery: {} bytes before damage", buf.len());
                }
                return Ok(Some(State::Held { buf, at: 0 }));
            }
            *damage_out = damage.map(|e| e.to_string());
            return Ok(None);
        }
        // The window is full and the data goes on.
        if matches!(way, Way::Zlib) {
            return Ok(Some(State::Live(Box::new(Live {
                decoder,
                prefix: buf,
                at: 0,
                tracker,
                judge: true,
            }))));
        }
        drop(decoder);
        let Some(mut counted) = self.open(way)? else { return Ok(None) };
        let mut whole = Tracker::default();
        let mut end_damage: Option<io::Error> = None;
        loop {
            match counted.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => whole.feed(&chunk[..n]),
                Err(e) => {
                    end_damage = Some(e);
                    break;
                },
            }
        }
        if !rule.accepts(&whole, end_damage.as_ref()) {
            *damage_out = end_damage.map(|e| e.to_string());
            return Ok(None);
        }
        let Some(serving) = self.open(way)? else { return Ok(None) };
        Ok(Some(State::Live(Box::new(Live {
            decoder: serving,
            prefix: Vec::new(),
            at: 0,
            tracker: Tracker::default(),
            judge: false,
        }))))
    }

    /// Decides how the stream decodes: the first way whose result is accepted.
    fn decide(&mut self) -> Result<State<'a>> {
        let mut raw = self.first.take().expect("the first input is taken once");
        let first = take_bytes(&mut *raw, 1).map_err(Error::Io)?;
        if first.is_empty() {
            // A zero-length stream decodes to zero bytes.
            return Ok(State::Held { buf: Vec::new(), at: 0 });
        }
        let joined: Box<dyn Read + 'a> =
            Box::new(Prefixed { head: [first[0], 0], len: 1, at: 0, rest: raw });
        let (mut zlib_error, mut deflate_error) = (None, None);

        // 1: zlib. The data it already opened is the data it reads.
        let primary = Box::new(Inflate::new(joined, true));
        if let Some(state) = self.attempt(primary, Way::Zlib, Rule::Plain, &mut zlib_error)? {
            return Ok(state);
        }
        // 2: raw deflate, in case the zlib header is corrupt.
        if let Some(d) = self.open(Way::Deflate)? {
            if let Some(state) =
                self.attempt(d, Way::Deflate, Rule::Plain, &mut deflate_error)?
            {
                return Ok(state);
            }
        }
        // 3: deflate after a corrupt two-byte zlib header.
        if let Some(d) = self.open(Way::DeflateAfter(2))? {
            if let Some(state) =
                self.attempt(d, Way::DeflateAfter(2), Rule::Skipped, &mut None)?
            {
                return Ok(state);
            }
        }
        // 4: zlib with the compression-method nibble corrected.
        if let Some(d) = self.open(Way::CorrectedHeader)? {
            if let Some(state) =
                self.attempt(d, Way::CorrectedHeader, Rule::Corrected, &mut None)?
            {
                return Ok(state);
            }
        }
        // 5: raw deflate from offsets 1 to 19, except the 2 already tried.
        for offset in (1..20).filter(|&o| o != 2) {
            let Some(d) = self.open(Way::DeflateAfter(offset))? else { break };
            if let Some(state) =
                self.attempt(d, Way::DeflateAfter(offset), Rule::Scanned, &mut None)?
            {
                return Ok(state);
            }
        }
        let size = {
            let mut all = (self.reopen)()?;
            let mut n = 0u64;
            let mut chunk = vec![0u8; 8 * 1024];
            loop {
                match all.read(&mut chunk).map_err(Error::Io)? {
                    0 => break n,
                    k => n += k as u64,
                }
            }
        };
        Err(Error::Decode(format!(
            "FlateDecode decompression failed: stream is labeled as compressed but all decompression attempts failed. \
            This violates PDF Spec ISO 32000-1:2008, Section 7.3.8.2. \
            Zlib error: {}, Deflate error: {}. Compressed size: {} bytes.",
            zlib_error.unwrap_or_else(|| "none".to_string()),
            deflate_error.unwrap_or_else(|| "none".to_string()),
            size
        )))
    }
}

impl Read for FlateReader<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            match &mut self.state {
                State::Start => {
                    self.state = self.decide().map_err(failure)?;
                },
                State::Held { buf, at } => {
                    let n = out.len().min(buf.len() - *at);
                    out[..n].copy_from_slice(&buf[*at..*at + n]);
                    *at += n;
                    return Ok(n);
                },
                State::Live(live) => {
                    if live.at < live.prefix.len() {
                        let n = out.len().min(live.prefix.len() - live.at);
                        out[..n].copy_from_slice(&live.prefix[live.at..live.at + n]);
                        live.at += n;
                        return Ok(n);
                    }
                    match live.decoder.read(out) {
                        Ok(0) => {
                            self.state = State::Done;
                            return Ok(0);
                        },
                        Ok(n) => {
                            if live.judge {
                                live.tracker.feed(&out[..n]);
                            }
                            return Ok(n);
                        },
                        Err(e) => {
                            let total = live.tracker.total;
                            let checksum_only = live.judge && is_checksum_mismatch(&e);
                            if checksum_only {
                                // The deflate data ended cleanly and only its
                                // Adler-32 failed, the one thing the other ways
                                // of reading the stream never check: every byte
                                // is there, and every way would return them.
                                let message = format!(
                                    "FlateDecode: checksum mismatch, data kept ({total} bytes)"
                                );
                                match &self.origin {
                                    // The document's sink, against the stream.
                                    Some(origin) => origin.warn(message),
                                    None => crate::extractors::warnings::push_global_warning(
                                        crate::extractors::warnings::Warning {
                                            category: crate::extractors::warnings::WarningCategory::SpecViolation,
                                            page: None,
                                            message,
                                            spec_section: Some("7.4.4"),
                                        },
                                    ),
                                }
                            }
                            let kept = checksum_only
                                || !live.judge
                                || is_truncation(&e)
                                || live.tracker.real();
                            self.state = State::Done;
                            if kept {
                                log::warn!("FlateDecode partial recovery after damage: {}", e);
                                return Ok(0);
                            }
                            return Err(failure(Error::Decode(format!(
                                "FlateDecode: the stream is corrupt after {} bytes of output: {}",
                                total,
                                e
                            ))));
                        },
                    }
                },
                State::Done => return Ok(0),
            }
        }
    }
}
// ── end pdf_manipulator patch ──

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    // #364 — when a strategy's partial recovery emits garbage (valid-looking
    // deflate bits that decode to pseudo-random bytes on a misaligned input),
    // the decoder must not accept it. Strategy 5 already validates via PDF
    // content-stream operators; strategies 1–4 now validate via
    // `looks_like_real_stream`. This test pins that guard.
    #[test]
    fn looks_like_real_stream_rejects_repeating_garbage() {
        // Actual symptom from nougat_026.pdf page 1 before the fix: 128 bytes
        // of `P\xffj!}\xef\xbd\xbd\xef\xbd\xbd...` high-bit-heavy repetition.
        let garbage = b"P\xffj!}\xef\xbd\xbd\xef\xbd\xbd\xef\xbd\xbd\xef\xbd\xbd\xef\xbd\xbd\xef\xbd\xbd\xef\xbd\xbd\xef\xbd\xbd".repeat(4);
        assert!(
            !looks_like_real_stream(&garbage),
            "misaligned-deflate garbage must be rejected as a partial recovery"
        );
    }

    #[test]
    fn looks_like_real_stream_accepts_content_stream_operators() {
        let real = b"BT /F1 12 Tf 100 700 Td (hello) Tj ET";
        assert!(looks_like_real_stream(real));
    }

    #[test]
    fn looks_like_real_stream_accepts_ascii_only_object_stream() {
        // Object-stream-like payload: ASCII, no content-stream operators.
        let object_stream = b"1 0 obj\n<< /Length 42 >>\nstream\nhello world\nendstream\nendobj\n";
        assert!(looks_like_real_stream(object_stream));
    }

    #[test]
    fn looks_like_real_stream_rejects_empty() {
        assert!(!looks_like_real_stream(&[]));
    }

    /// A deflate stream that ends without its final block marker still yields
    /// the bytes it did encode.
    ///
    /// flate2 1.1.10 rejects such a stream where 1.1.9 returned the prefix
    /// (flate2-rs#556). PDFs in the wild rely on the tolerant behaviour: an
    /// incremental-update xref stream of 51 compressed bytes decodes to 70 and
    /// simply stops, and zlib, poppler and pdf.js all accept it. When this
    /// decoder refused it, xref parsing failed, the reader fell back to
    /// reconstruction, and a page tree rebuilt from an incomplete object table
    /// dropped a whole page's worth of text.
    ///
    /// The payload here is deliberately BINARY — an xref stream is not text —
    /// because the partial-recovery path used to be gated on a content-stream
    /// heuristic that only accepts `BT`/`Tj` markers or printable ASCII, and so
    /// rejected exactly this shape.
    #[test]
    fn test_empty_stream_decodes_to_no_bytes() {
        // An empty Form XObject — what a zero-area transparency group is
        // written as — reaches the decoder as a zero-length stream. It must
        // decode to nothing rather than fail: the caller renders the form, and
        // an error there abandons the rest of the parent content stream.
        let decoded = FlateDecoder::default()
            .decode(&[])
            .expect("an empty stream must decode to zero bytes, not fail");

        assert!(
            decoded.is_empty(),
            "an empty stream must not invent bytes, got {}",
            decoded.len()
        );
    }

    #[test]
    fn test_truncated_deflate_stream_yields_its_decoded_prefix() {
        // Binary payload, compressible, with no content-stream operators.
        let original: Vec<u8> = (0u16..600).map(|i| (i % 251) as u8).collect();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original).unwrap();
        let complete = encoder.finish().unwrap();

        // Cut the tail off: the final block marker and checksum never arrive.
        let truncated = &complete[..complete.len() * 3 / 4];
        assert!(truncated.len() < complete.len(), "test needs a genuinely shortened stream");

        let decoded = FlateDecoder::default()
            .decode(truncated)
            .expect("a truncated deflate stream must still yield its decoded prefix");

        assert!(!decoded.is_empty(), "recovery returned nothing for a truncated stream");
        assert!(
            original.starts_with(&decoded),
            "recovered bytes must be a prefix of the original, not garbage"
        );
    }

    #[test]
    fn test_flate_decode_simple() {
        let decoder = FlateDecoder::default();

        // Compress some data
        let original = b"Hello, FlateDecode!";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Decompress
        let decoded = decoder.decode(&compressed).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_flate_decode_empty() {
        let decoder = FlateDecoder::default();

        // Compress empty data
        let original = b"";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let decoded = decoder.decode(&compressed).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_flate_decode_large_data() {
        let decoder = FlateDecoder::default();

        // Create large repeated data
        let original = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".repeat(1000);
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original).unwrap();
        let compressed = encoder.finish().unwrap();

        let decoded = decoder.decode(&compressed).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_flate_decode_invalid_data() {
        let decoder = FlateDecoder::default();

        // Invalid zlib data - should fail decompression
        // SPEC COMPLIANCE: We now correctly reject invalid compressed data
        // instead of returning it as raw data (which violated PDF spec)
        let invalid = b"This is not zlib compressed data";
        let result = decoder.decode(invalid);
        assert!(result.is_err());

        // Verify error message mentions spec compliance
        if let Err(e) = result {
            let error_msg = format!("{}", e);
            assert!(error_msg.contains("FlateDecode decompression failed"));
        }
    }

    #[test]
    fn test_flate_decoder_name() {
        let decoder = FlateDecoder::default();
        assert_eq!(decoder.name(), "FlateDecode");
    }

    // ── pdf_manipulator patch: the decompression cap, its limit API and its env variable are gone ──
    // ── end pdf_manipulator patch ──
}
