//! Windowed scans of a document's bytes.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Every file-level scan of the engine (the reconstruction of a damaged
//! cross-reference table, the offset map of a lost object, the sweep for
//! object streams, the search for a trailer) goes through [`scan`]: the bytes
//! are read in [`WINDOW`]-byte windows that overlap by [`OVERLAP`] bytes, so
//! a pattern that sits on the edge of two windows is found exactly once.
//! What a scan holds is one window plus whatever its caller keeps per match;
//! no scan holds the file.

use crate::error::{Error, Result};
use crate::object::Object;
use crate::lexer::{token, Token};
use crate::parser::{parse_dictionary, parse_object};
use crate::source::Source;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

/// Bytes read from the source at a time.
pub(crate) const WINDOW: usize = 64 * 1024;

/// Bytes a window keeps from the one before it. A pattern of up to
/// `OVERLAP + 1` bytes cannot hide on an edge.
pub(crate) const OVERLAP: usize = 16;

/// The patterns the scans look for.
pub(crate) mod pat {
    pub(crate) const OBJ: &[u8] = b"obj";
    pub(crate) const TRAILER: &[u8] = b"trailer";
    pub(crate) const STARTXREF: &[u8] = b"startxref";
    pub(crate) const EOF_MARK: &[u8] = b"%%EOF";
}

/// Positioned reads: the one thing a scan needs of its input.
pub(crate) trait ReadAt {
    /// Fills `buf` from `offset`; fewer bytes than `buf.len()` only at the
    /// end of the input.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;

    /// The input's length in bytes.
    fn len(&mut self) -> Result<u64>;
}

/// A document's [`Source`] as a [`ReadAt`].
pub(crate) struct OfSource<'a>(pub(crate) &'a Source);

impl ReadAt for OfSource<'_> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.0.read_at(offset, buf)
    }

    fn len(&mut self) -> Result<u64> {
        self.0.len()
    }
}

/// Any `Read + Seek` as a [`ReadAt`]. It leaves the reader wherever the last
/// read ended: a caller that goes on reading seeks first.
pub(crate) struct OfSeek<'a, R>(pub(crate) &'a mut R);

impl<R: Read + Seek> ReadAt for OfSeek<'_, R> {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.0.seek(SeekFrom::Start(offset))?;
        let mut filled = 0;
        while filled < buf.len() {
            let n = self.0.read(&mut buf[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        Ok(filled)
    }

    fn len(&mut self) -> Result<u64> {
        Ok(self.0.seek(SeekFrom::End(0))?)
    }
}

/// Whether a scan goes on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Continue,
    Stop,
}

/// One match: which pattern, where, and the window it was found in (so the
/// bytes around it are usually at hand).
pub(crate) struct Hit<'w> {
    pub(crate) pattern: usize,
    pub(crate) offset: u64,
    window: &'w [u8],
    base: u64,
}

impl<'w> Hit<'w> {
    /// Up to `len` bytes at `start`: the window's own when they lie in it,
    /// else read from `r`. Fewer than `len` only at the end of the input.
    pub(crate) fn bytes<R: ReadAt>(
        &self,
        r: &mut R,
        start: u64,
        len: usize,
    ) -> Result<Cow<'w, [u8]>> {
        let window_end = self.base + self.window.len() as u64;
        if start >= self.base && start + len as u64 <= window_end {
            let from = (start - self.base) as usize;
            return Ok(Cow::Borrowed(&self.window[from..from + len]));
        }
        let mut buf = vec![0u8; len];
        let n = r.read_at(start, &mut buf)?;
        buf.truncate(n);
        Ok(Cow::Owned(buf))
    }
}

/// Finds patterns in a stream of chunks, each once, across chunk edges.
pub(crate) struct Matcher<'p> {
    patterns: &'p [&'p [u8]],
    /// The last [`OVERLAP`] bytes fed, then the chunk being scanned.
    buf: Vec<u8>,
    /// The input offset of `buf[0]`.
    base: u64,
}

impl<'p> Matcher<'p> {
    pub(crate) fn new(patterns: &'p [&'p [u8]], from: u64) -> Self {
        Self { patterns, buf: Vec::with_capacity(WINDOW + OVERLAP), base: from }
    }

    /// Scans the next `chunk`, which starts where the last one ended, and
    /// calls `visit` for each match that ends inside it, in offset order. A
    /// match that ends inside the bytes kept from the last chunk was visited
    /// with that chunk.
    pub(crate) fn feed(
        &mut self,
        chunk: &[u8],
        visit: &mut dyn FnMut(&Hit<'_>) -> Result<Flow>,
    ) -> Result<Flow> {
        let kept = self.buf.len();
        self.buf.extend_from_slice(chunk);
        let mut found: Vec<(usize, usize)> = Vec::new();
        for (index, pattern) in self.patterns.iter().enumerate() {
            for at in memchr::memmem::find_iter(&self.buf, pattern) {
                if at + pattern.len() > kept {
                    found.push((at, index));
                }
            }
        }
        found.sort_unstable();
        for (at, pattern) in found {
            let hit = Hit {
                pattern,
                offset: self.base + at as u64,
                window: &self.buf,
                base: self.base,
            };
            if visit(&hit)? == Flow::Stop {
                return Ok(Flow::Stop);
            }
        }
        let drop = self.buf.len().saturating_sub(OVERLAP);
        self.buf.drain(..drop);
        self.base += drop as u64;
        Ok(Flow::Continue)
    }
}

/// Scans `r` from `from` to its end for `patterns`, calling `visit` with
/// each match in offset order. `visit` may read around the match through
/// [`Hit::bytes`] and the reader it is handed.
pub(crate) fn scan<R: ReadAt>(
    r: &mut R,
    from: u64,
    patterns: &[&[u8]],
    mut visit: impl FnMut(&Hit<'_>, &mut R) -> Result<Flow>,
) -> Result<()> {
    let mut matcher = Matcher::new(patterns, from);
    let mut chunk = vec![0u8; WINDOW];
    let mut at = from;
    loop {
        let n = r.read_at(at, &mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        at += n as u64;
        if matcher.feed(&chunk[..n], &mut |hit| visit(hit, &mut *r))? == Flow::Stop {
            return Ok(());
        }
        if n < WINDOW {
            return Ok(());
        }
    }
}

/// The offset of the last `needle` that ends at or before `end`, read
/// backwards in windows.
pub(crate) fn rfind<R: ReadAt>(r: &mut R, end: u64, needle: &[u8]) -> Result<Option<u64>> {
    let keep = needle.len().saturating_sub(1);
    let mut hi = end;
    let mut buf = vec![0u8; WINDOW + keep];
    loop {
        if hi == 0 {
            return Ok(None);
        }
        let lo = hi.saturating_sub(WINDOW as u64);
        let want = (hi - lo) as usize + keep.min((end - hi) as usize);
        let n = r.read_at(lo, &mut buf[..want])?;
        if let Some(at) = memchr::memmem::rfind(&buf[..n], needle) {
            return Ok(Some(lo + at as u64));
        }
        hi = lo;
    }
}

/// The lines of a reader, split at CR, LF and CRLF, read on demand in
/// windows: a caller that stops early has read only what it used.
pub(crate) struct Lines<R: Read> {
    reader: R,
    buf: Vec<u8>,
    at: usize,
    filled: usize,
    ended: bool,
    failed: Option<std::io::Error>,
    /// A line given back by [`Lines::unread`], returned next.
    pushed: Option<String>,
}

impl<R: Read> Lines<R> {
    pub(crate) fn new(reader: R) -> Self {
        Self { reader, buf: vec![0u8; WINDOW], at: 0, filled: 0, ended: false, failed: None, pushed: None }
    }

    /// Gives back the line just read, so the next call returns it again.
    pub(crate) fn unread(&mut self, line: String) {
        self.pushed = Some(line);
    }

    /// The read error that ended the lines, if one did.
    pub(crate) fn error(&mut self) -> Option<std::io::Error> {
        self.failed.take()
    }

    /// The byte at `at`, reading on when the window is spent.
    fn peek(&mut self) -> Option<u8> {
        if self.at == self.filled {
            if self.ended {
                return None;
            }
            match self.reader.read(&mut self.buf) {
                Ok(0) => {
                    self.ended = true;
                    return None;
                },
                Ok(n) => {
                    self.at = 0;
                    self.filled = n;
                },
                Err(e) => {
                    self.ended = true;
                    self.failed = Some(e);
                    return None;
                },
            }
        }
        Some(self.buf[self.at])
    }
}

impl<R: Read> Iterator for Lines<R> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if let Some(line) = self.pushed.take() {
            return Some(line);
        }
        let mut line: Vec<u8> = Vec::new();
        while let Some(byte) = self.peek() {
            self.at += 1;
            match byte {
                b'\n' => return Some(String::from_utf8_lossy(&line).into_owned()),
                b'\r' => {
                    if self.peek() == Some(b'\n') {
                        self.at += 1;
                    }
                    return Some(String::from_utf8_lossy(&line).into_owned());
                },
                other => line.push(other),
            }
        }
        // A last line with no line ending.
        (!line.is_empty()).then(|| String::from_utf8_lossy(&line).into_owned())
    }
}

/// The two digit runs and where they start, in front of an `obj` keyword.
pub(crate) struct RawHeader {
    pub(crate) start: u64,
    pub(crate) number: Vec<u8>,
    pub(crate) generation: Vec<u8>,
}

/// How the numbers of an object header are separated from each other and
/// from `obj`.
#[derive(Clone, Copy)]
pub(crate) enum Spacing {
    /// One or more whitespace bytes.
    Any,
    /// Exactly one space.
    Single,
}

/// What may stand in front of an object header.
#[derive(Clone, Copy)]
pub(crate) enum Before {
    /// Anything.
    Anything,
    /// The start of the input or a line ending.
    LineStart,
    /// The start of the input, a line ending or a space.
    LineStartOrSpace,
}

/// The longest stretch read in front of `obj` to find its header. A number
/// written with more digits than fit is not a header.
const LOOKBACK: usize = 64;

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// The `N G` in front of the `obj` keyword at `hit`, or `None` when the
/// bytes there are not a header under `spacing` and `before`.
pub(crate) fn header_before<R: ReadAt>(
    hit: &Hit<'_>,
    r: &mut R,
    spacing: Spacing,
    before: Before,
) -> Result<Option<RawHeader>> {
    let from = hit.offset.saturating_sub(LOOKBACK as u64);
    let back = hit.bytes(r, from, (hit.offset - from) as usize)?;
    let mut i = back.len();

    let separator = |i: &mut usize| -> bool {
        match spacing {
            Spacing::Any => {
                let end = *i;
                while *i > 0 && is_space(back[*i - 1]) {
                    *i -= 1;
                }
                *i < end
            },
            Spacing::Single => {
                if *i > 0 && back[*i - 1] == b' ' {
                    *i -= 1;
                    true
                } else {
                    false
                }
            },
        }
    };
    let digits = |i: &mut usize| -> Option<(usize, usize)> {
        let end = *i;
        while *i > 0 && back[*i - 1].is_ascii_digit() {
            *i -= 1;
        }
        (*i < end).then_some((*i, end))
    };

    if !separator(&mut i) {
        return Ok(None);
    }
    let Some((gen_from, gen_to)) = digits(&mut i) else {
        return Ok(None);
    };
    if !separator(&mut i) {
        return Ok(None);
    }
    let Some((num_from, num_to)) = digits(&mut i) else {
        return Ok(None);
    };
    // The digit run (or what stands before it) runs on past what was read.
    if i == 0 && from > 0 {
        return Ok(None);
    }
    let preceded_by = (i > 0).then(|| back[i - 1]);
    let allowed = match before {
        Before::Anything => true,
        Before::LineStart => preceded_by.map_or(true, |b| b == b'\n' || b == b'\r'),
        Before::LineStartOrSpace => {
            preceded_by.map_or(true, |b| b == b'\n' || b == b'\r' || b == b' ')
        },
    };
    if !allowed {
        return Ok(None);
    }
    Ok(Some(RawHeader {
        start: from + num_from as u64,
        number: back[num_from..num_to].to_vec(),
        generation: back[gen_from..gen_to].to_vec(),
    }))
}

/// The first byte at or after `from` that is not ASCII whitespace; `None`
/// when only whitespace (or nothing) follows.
pub(crate) fn first_non_space<R: ReadAt>(
    hit: &Hit<'_>,
    r: &mut R,
    mut from: u64,
) -> Result<Option<u8>> {
    loop {
        let chunk = hit.bytes(r, from, 256)?;
        if chunk.is_empty() {
            return Ok(None);
        }
        if let Some(byte) = chunk.iter().find(|b| !b.is_ascii_whitespace()) {
            return Ok(Some(*byte));
        }
        from += chunk.len() as u64;
    }
}

/// The offset of every `N G obj` header in the source that a lost object can
/// be found by: the number and generation are separated by single spaces, the
/// header starts a line, and `obj` is followed by a line ending, a space, a
/// tab, `<` or the end. The first header of a number wins.
pub(crate) fn object_offsets(source: &Source) -> Result<HashMap<u32, u64>> {
    let mut offsets = HashMap::new();
    scan(&mut OfSource(source), 0, &[pat::OBJ], |hit, r| {
        let Some(header) = header_before(hit, r, Spacing::Single, Before::LineStart)? else {
            return Ok(Flow::Continue);
        };
        let Some(number) = std::str::from_utf8(&header.number)
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            return Ok(Flow::Continue);
        };
        let valid_end = match hit.bytes(r, hit.offset + 3, 1)?.first() {
            None => true,
            Some(c) => matches!(c, b'\n' | b'\r' | b' ' | b'\t' | b'<'),
        };
        if valid_end {
            offsets.entry(number).or_insert(header.start);
        }
        Ok(Flow::Continue)
    })?;
    Ok(offsets)
}

/// Bytes read after an object header to see its dictionary for the
/// object-stream sweep.
const DICT_PEEK_BYTES: usize = 2048;

/// The `(number, offset)` of every object whose dictionary names
/// `/Type /ObjStm`, found by the raw header pattern: the header starts a line
/// or follows a space, and the marker lies within [`DICT_PEEK_BYTES`] of `obj`.
pub(crate) fn objstm_candidates(
    source: &Source,
    has_marker: fn(&[u8]) -> bool,
) -> Result<Vec<(u32, u64)>> {
    let mut out = Vec::new();
    scan(&mut OfSource(source), 0, &[pat::OBJ], |hit, r| {
        let Some(header) = header_before(hit, r, Spacing::Single, Before::LineStartOrSpace)? else {
            return Ok(Flow::Continue);
        };
        let Some(number) = std::str::from_utf8(&header.number)
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            return Ok(Flow::Continue);
        };
        if std::str::from_utf8(&header.generation)
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .is_none()
        {
            return Ok(Flow::Continue);
        }
        let dict = hit.bytes(r, hit.offset, DICT_PEEK_BYTES)?;
        if has_marker(&dict) {
            out.push((number, header.start));
        }
        Ok(Flow::Continue)
    })?;
    Ok(out)
}

/// The `trailer` keyword a scan has met whose dictionary is not parsed yet:
/// the dictionary lies between it and the next `trailer`, `startxref` or
/// `%%EOF`, so it is parsed when that next word is met.
#[derive(Default)]
pub(crate) struct Trailers {
    pending: Option<u64>,
}

impl Trailers {
    /// Takes a hit on one of [`TRAILER_WORDS`]: the trailer waiting before it,
    /// if there is one, is parsed from the bytes between, and `accept` is
    /// called with where that trailer started and what it parsed to. A hit on
    /// `trailer` itself (`is_trailer`) becomes the one waiting.
    pub(crate) fn hit<R: ReadAt>(
        &mut self,
        hit: &Hit<'_>,
        r: &mut R,
        is_trailer: bool,
        accept: &mut dyn FnMut(u64, Result<Object>),
    ) -> Result<()> {
        if let Some(start) = self.pending.take() {
            if let Some(parsed) = parse_trailer_span(r, start, hit.offset)? {
                accept(start, parsed);
            }
        }
        if is_trailer {
            self.pending = Some(hit.offset);
        }
        Ok(())
    }

    /// Ends the scan: a trailer waiting runs to the end of the input.
    pub(crate) fn finish<R: ReadAt>(
        &mut self,
        r: &mut R,
        accept: &mut dyn FnMut(u64, Result<Object>),
    ) -> Result<()> {
        if let Some(start) = self.pending.take() {
            let end = r.len()?;
            if let Some(parsed) = parse_trailer_span(r, start, end)? {
                accept(start, parsed);
            }
        }
        Ok(())
    }
}

/// The words that end a trailer dictionary, `trailer` first.
pub(crate) const TRAILER_WORDS: [&[u8]; 3] = [pat::TRAILER, pat::STARTXREF, pat::EOF_MARK];

/// Parses the dictionary after the `trailer` keyword at `start`, among the
/// bytes up to `end`. `None` when `trailer` is not followed by `<<`.
fn parse_trailer_span<R: ReadAt>(
    r: &mut R,
    start: u64,
    end: u64,
) -> Result<Option<Result<Object>>> {
    let from = start + pat::TRAILER.len() as u64;
    let span = end.saturating_sub(from);
    let mut want = 4096u64;
    loop {
        let len = want.min(span) as usize;
        let mut buf = vec![0u8; len];
        let n = r.read_at(from, &mut buf)?;
        buf.truncate(n);
        let complete = len as u64 == span || n < len;
        let body: &[u8] = {
            let skip = buf.iter().take_while(|b| is_space(**b)).count();
            &buf[skip..]
        };
        if body.len() < 2 && !complete {
            want *= 2;
            continue;
        }
        if !body.starts_with(b"<<") {
            return Ok(None);
        }
        // A dictionary cut off by the end of the bytes read parses to the
        // entries so far with nothing left over: more bytes may close it.
        match parse_object(&buf) {
            Ok((rest, object)) if !rest.is_empty() || complete => {
                return Ok(Some(Ok(object)))
            },
            Ok(_) => want *= 2,
            Err(e) if complete => {
                return Ok(Some(Err(Error::ParseError {
                    offset: from as usize,
                    reason: e.to_string(),
                })))
            },
            Err(_) => want *= 2,
        }
    }
}

/// Where the first `endobj` at or after `from` ends; the end of the input when
/// there is none.
fn object_end<R: ReadAt>(r: &mut R, from: u64) -> Result<u64> {
    let mut end = None;
    scan(r, from, &[b"endobj"], |hit, _| {
        end = Some(hit.offset + 6);
        Ok(Flow::Stop)
    })?;
    match end {
        Some(end) => Ok(end),
        None => r.len(),
    }
}

/// What the start of an object parsed to.
enum Head {
    /// The object, whole.
    Whole(Object),
    /// A stream: its dictionary, its data not read.
    Stream(HashMap<String, Object>),
    /// More bytes are needed, or the bytes are not an object.
    Short(Error),
}

fn short(reason: String) -> Head {
    Head::Short(Error::ParseError { offset: 0, reason })
}

/// Parses `N G obj` and the object after it from `buf`, which holds the
/// object's first bytes; `complete` when `buf` runs to where the object must
/// end. A stream's data is not parsed.
fn parse_head(buf: &[u8], complete: bool) -> Head {
    let Ok((rest, _)) = token(buf) else {
        return short("failed to parse object number".to_string());
    };
    let Ok((rest, _)) = token(rest) else {
        return short("failed to parse generation".to_string());
    };
    let Ok((rest, _)) = token(rest) else {
        return short("failed to parse 'obj' keyword".to_string());
    };
    match token(rest) {
        Ok((after, Token::DictStart)) => {
            let (remaining, dict) = match parse_dictionary(after) {
                Ok(parsed) => parsed,
                Err(e) => return short(format!("failed to parse object: {}", e)),
            };
            let Object::Dictionary(dict) = dict else {
                return short("failed to parse object: not a dictionary".to_string());
            };
            let tail = remaining.iter().take_while(|b| is_space(**b)).count();
            if !complete && remaining.len() - tail < b"stream".len() {
                return short("the object runs past the bytes read".to_string());
            }
            match token(remaining) {
                Ok((_, Token::StreamStart)) => Head::Stream(dict),
                _ => Head::Whole(Object::Dictionary(dict)),
            }
        },
        _ => match parse_object(rest) {
            Ok((left, object)) if !left.is_empty() || complete => Head::Whole(object),
            Ok(_) => short("the object runs past the bytes read".to_string()),
            Err(e) => short(format!("failed to parse object: {}", e)),
        },
    }
}

/// Loads the object at `offset` for the reconstruction of a damaged file.
///
/// An object other than a stream is read whole. A stream is its dictionary:
/// its data is read only for an object stream, the one kind the
/// reconstruction unpacks, and then at its stored size; any other stream
/// comes back with no data.
pub(crate) fn load_object<R: ReadAt>(r: &mut R, offset: u64) -> Result<Object> {
    let mut limit: Option<u64> = None;
    let mut want = 4096u64;
    loop {
        let to = limit.map_or(u64::MAX, |l| l.max(offset));
        let len = want.min(to - offset) as usize;
        let mut buf = vec![0u8; len];
        let n = r.read_at(offset, &mut buf)?;
        buf.truncate(n);
        let complete = n < len || offset + n as u64 >= to;
        match parse_head(&buf, complete) {
            Head::Whole(object) => return Ok(object),
            Head::Stream(dict) => {
                let object_stream =
                    dict.get("Type").and_then(|t| t.as_name()) == Some("ObjStm");
                if !object_stream {
                    return Ok(Object::stream(dict, Vec::new()));
                }
                let end = object_end(r, offset)?;
                let mut content = vec![0u8; (end - offset) as usize];
                let n = r.read_at(offset, &mut content)?;
                content.truncate(n);
                let (rest, _) = token(&content[..]).map_err(|e| Error::ParseError {
                    offset: 0,
                    reason: format!("failed to parse object number: {}", e),
                })?;
                let (rest, _) = token(rest).map_err(|e| Error::ParseError {
                    offset: 0,
                    reason: format!("failed to parse generation: {}", e),
                })?;
                let (rest, _) = token(rest).map_err(|e| Error::ParseError {
                    offset: 0,
                    reason: format!("failed to parse 'obj' keyword: {}", e),
                })?;
                let (_, object) = parse_object(rest).map_err(|e| Error::ParseError {
                    offset: 0,
                    reason: format!("failed to parse object: {}", e),
                })?;
                return Ok(object);
            },
            Head::Short(e) if complete => return Err(e),
            Head::Short(_) => {
                if limit.is_none() {
                    limit = Some(object_end(r, offset)?);
                }
                want *= 2;
            },
        }
    }
}
