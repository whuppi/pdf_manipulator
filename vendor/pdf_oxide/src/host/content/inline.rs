//! Where the data of an inline image ends.
//!
//! The data is binary and may contain `EI`, so its end is found by the
//! first of these rules that gives a position with `EI` after it (white-space,
//! `EI`, then white-space, a delimiter or the end of the source):
//!
//! 1. `/L` (`/Length`): that many bytes.
//! 2. No filter: `ceil(W * BPC * components / 8) * H` bytes; `/IM true` is one
//!    bit and one component, a named colour space takes its component count
//!    from the [`Resolver`].
//! 3. Filtered: where the outermost filter ends. `ASCIIHexDecode` ends at
//!    `>`, `ASCII85Decode` at `~>`, `RunLengthDecode` at the byte 128,
//!    `FlateDecode` where the zlib stream ends. `LZWDecode` has no rule here:
//!    weezl reads ahead of the end code, so its `consumed_in` at the end is
//!    exact only when it is fed one byte at a time. A chain whose last filter
//!    is `DCTDecode`, `JPXDecode` or `CCITTFaxDecode` has none either.
//! 4. Otherwise, and when every rule above failed: the first white-space,
//!    `EI`, white-space-or-delimiter, after which the first byte that is not
//!    white-space is `(` or `<`, or the next 32 bytes (fewer at the end of
//!    the source) are all printable ASCII or white-space.
//!
//! This replaces the rule of `parse_content_stream` (the first
//! white-space, `EI`, white-space-or-delimiter), which ends a binary image
//! at any ` EI ` inside its data.
//!
//! The data streams past: the window keeps only the bytes a rule still
//! needs to look at, and one image's whole data only when the caller asks
//! to collect it. When a rule with a known end is pending, the bytes from
//! the first fallback candidate on are held until it resolves, at most the
//! image's own size.

use super::lexer::{is_delim, is_space, Lexer};
use crate::cancel::{poll, Stride, STRIDE};
use crate::error::Result;
use crate::object::Object;
use flate2::{Decompress, FlushDecompress, Status};
use std::collections::HashMap;
use std::io::{self, Read};

/// Bytes after `EI` the fallback rule looks at.
const LOOKAHEAD: usize = 32;

/// Bytes of the data a codec rule reads between two polls.
const CHUNK: usize = STRIDE as usize;

/// Component count of a named colour space (the name is a key of the page's
/// `/ColorSpace` resource); `None` when it does not resolve.
pub type Resolver = Box<dyn Fn(&str) -> Option<usize> + Send>;

/// How the end of the data is found when the dictionary gives no length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndRule {
    /// Rule 4 only.
    Fallback,
    /// Ends at `>`.
    AsciiHex,
    /// Ends at `~>`.
    Ascii85,
    /// Ends at the byte 128.
    RunLength,
    /// Ends where the zlib stream ends.
    Flate,
}

/// What an inline image's dictionary says about where its data ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSpec {
    /// `/L`.
    pub length: Option<u64>,
    /// The size of unfiltered data computed from the dictionary.
    pub raw_length: Option<u64>,
    /// How the outermost filter ends.
    pub rule: EndRule,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    AsciiHex,
    Ascii85,
    Lzw,
    Flate,
    RunLength,
    Ccitt,
    Dct,
    Jpx,
    Other,
}

fn entry<'a>(dict: &'a HashMap<String, Object>, short: &str, long: &str) -> Option<&'a Object> {
    dict.get(short).or_else(|| dict.get(long))
}

fn unsigned(object: Option<&Object>) -> Option<u64> {
    u64::try_from(object?.as_integer()?).ok()
}

fn filter_of(name: &str) -> Filter {
    match name {
        "AHx" | "ASCIIHexDecode" => Filter::AsciiHex,
        "A85" | "ASCII85Decode" => Filter::Ascii85,
        "LZW" | "LZWDecode" => Filter::Lzw,
        "Fl" | "FlateDecode" => Filter::Flate,
        "RL" | "RunLengthDecode" => Filter::RunLength,
        "CCF" | "CCITTFaxDecode" => Filter::Ccitt,
        "DCT" | "DCTDecode" => Filter::Dct,
        "JPXDecode" => Filter::Jpx,
        _ => Filter::Other,
    }
}

fn filters(object: Option<&Object>) -> Vec<Filter> {
    match object {
        Some(Object::Name(name)) => vec![filter_of(name)],
        Some(Object::Array(items)) => items
            .iter()
            .map(|item| item.as_name().map_or(Filter::Other, filter_of))
            .collect(),
        _ => Vec::new(),
    }
}

fn components_of_name(name: &str, resolver: Option<&Resolver>) -> Option<usize> {
    match name {
        "G" | "DeviceGray" | "CalGray" | "I" | "Indexed" | "Separation" => Some(1),
        "RGB" | "DeviceRGB" | "CalRGB" | "Lab" => Some(3),
        "CMYK" | "DeviceCMYK" => Some(4),
        other => resolver.and_then(|resolve| resolve(other)),
    }
}

fn components_of_array(items: &[Object], resolver: Option<&Resolver>) -> Option<usize> {
    match items.first()?.as_name()? {
        "DeviceN" => Some(items.get(1)?.as_array()?.len()),
        "ICCBased" => {
            let n = items.get(1)?.as_dict()?.get("N")?.as_integer()?;
            usize::try_from(n).ok()
        },
        family => components_of_name(family, resolver),
    }
}

/// The component count of a colour space given as a name or an array.
pub(crate) fn components(colour_space: &Object, resolver: Option<&Resolver>) -> Option<usize> {
    match colour_space {
        Object::Name(name) => components_of_name(name, resolver),
        Object::Array(items) => components_of_array(items, resolver),
        _ => None,
    }
}

fn raw_length(dict: &HashMap<String, Object>, resolver: Option<&Resolver>) -> Option<u64> {
    let width = unsigned(entry(dict, "W", "Width"))?;
    let height = unsigned(entry(dict, "H", "Height"))?;
    let mask = matches!(entry(dict, "IM", "ImageMask"), Some(Object::Boolean(true)));
    let (bits, components) = if mask {
        (1, 1)
    } else {
        let bits = unsigned(entry(dict, "BPC", "BitsPerComponent"))?;
        let space = components(entry(dict, "CS", "ColorSpace")?, resolver)?;
        (bits, space as u64)
    };
    let row = width.checked_mul(bits)?.checked_mul(components)?.div_ceil(8);
    row.checked_mul(height)
}

impl ImageSpec {
    /// Reads the length, the filter and the geometry out of an inline image
    /// dictionary, in either key spelling.
    pub fn from_dict(dict: &HashMap<String, Object>, resolver: Option<&Resolver>) -> Self {
        let chain = filters(entry(dict, "F", "Filter"));
        let ends_in_codec = matches!(chain.last(), Some(Filter::Ccitt | Filter::Dct | Filter::Jpx));
        let rule = match chain.first() {
            _ if ends_in_codec => EndRule::Fallback,
            Some(Filter::AsciiHex) => EndRule::AsciiHex,
            Some(Filter::Ascii85) => EndRule::Ascii85,
            Some(Filter::RunLength) => EndRule::RunLength,
            Some(Filter::Flate) => EndRule::Flate,
            _ => EndRule::Fallback,
        };
        Self {
            length: unsigned(entry(dict, "L", "Length")),
            raw_length: if chain.is_empty() { raw_length(dict, resolver) } else { None },
            rule,
        }
    }
}

enum Verdict {
    Yes,
    No,
    Wait,
}

/// Whether `EI` stands at data offset `pos`: white-space, `EI`, then
/// white-space, a delimiter or the end of the source.
fn ei_at(w: &[u8], base: u64, pos: u64, eof: bool) -> Verdict {
    let at = (pos - base) as usize;
    let short = if eof { Verdict::No } else { Verdict::Wait };
    if at >= w.len() {
        return short;
    }
    if !is_space(w[at]) {
        return Verdict::No;
    }
    if at + 1 >= w.len() {
        return short;
    }
    if w[at + 1] != b'E' {
        return Verdict::No;
    }
    if at + 2 >= w.len() {
        return short;
    }
    if w[at + 2] != b'I' {
        return Verdict::No;
    }
    if at + 3 == w.len() {
        return if eof { Verdict::Yes } else { Verdict::Wait };
    }
    if is_space(w[at + 3]) || is_delim(w[at + 3]) {
        Verdict::Yes
    } else {
        Verdict::No
    }
}

/// The fallback rule's look at the bytes after an `EI`.
fn after_ei(rest: &[u8], eof: bool) -> Verdict {
    let mut seen_non_space = false;
    for &b in rest.iter().take(LOOKAHEAD) {
        if !seen_non_space && !is_space(b) {
            seen_non_space = true;
            if b == b'(' || b == b'<' {
                return Verdict::Yes;
            }
        }
        if !(is_space(b) || (0x20..0x7F).contains(&b)) {
            return Verdict::No;
        }
    }
    if rest.len() >= LOOKAHEAD || eof {
        Verdict::Yes
    } else {
        Verdict::Wait
    }
}

enum RunState {
    Length,
    Literal(u16),
    Repeat,
}

enum Codec {
    None,
    AsciiHex,
    Ascii85 { after_tilde: bool },
    RunLength(RunState),
    Flate(Box<Decompress>),
}

enum Feed {
    Pending,
    /// The codec ended this many bytes into the input.
    End(usize),
    Failed,
}

/// Feeds `input` to the codec, polling every [`CHUNK`] bytes of it.
fn feed(codec: &mut Codec, input: &[u8]) -> Result<Feed> {
    Ok(match codec {
        Codec::None => Feed::Failed,
        Codec::AsciiHex => {
            for (n, chunk) in input.chunks(CHUNK).enumerate() {
                poll()?;
                if let Some(i) = chunk.iter().position(|&b| b == b'>') {
                    return Ok(Feed::End(n * CHUNK + i + 1));
                }
            }
            Feed::Pending
        },
        Codec::Ascii85 { after_tilde } => {
            for (n, chunk) in input.chunks(CHUNK).enumerate() {
                poll()?;
                for (i, &b) in chunk.iter().enumerate() {
                    if *after_tilde && b == b'>' {
                        return Ok(Feed::End(n * CHUNK + i + 1));
                    }
                    *after_tilde = b == b'~';
                }
            }
            Feed::Pending
        },
        Codec::RunLength(state) => {
            for (c, chunk) in input.chunks(CHUNK).enumerate() {
                poll()?;
                for (i, &b) in chunk.iter().enumerate() {
                    *state = match state {
                        RunState::Length if b == 128 => return Ok(Feed::End(c * CHUNK + i + 1)),
                        RunState::Length if b < 128 => RunState::Literal(u16::from(b) + 1),
                        RunState::Length => RunState::Repeat,
                        RunState::Literal(1) | RunState::Repeat => RunState::Length,
                        RunState::Literal(n) => RunState::Literal(*n - 1),
                    };
                }
            }
            Feed::Pending
        },
        Codec::Flate(inflater) => {
            let start = inflater.total_in();
            let mut input = input;
            let mut out = [0u8; 4096];
            loop {
                poll()?;
                let (in_before, out_before) = (inflater.total_in(), inflater.total_out());
                match inflater.decompress(input, &mut out, FlushDecompress::None) {
                    Err(_) => return Ok(Feed::Failed),
                    Ok(Status::StreamEnd) => {
                        return Ok(Feed::End((inflater.total_in() - start) as usize))
                    },
                    Ok(_) => {
                        let used = (inflater.total_in() - in_before) as usize;
                        let produced = (inflater.total_out() - out_before) as usize;
                        input = &input[used..];
                        if (used == 0 && produced == 0) || (input.is_empty() && produced < out.len())
                        {
                            return Ok(Feed::Pending);
                        }
                    },
                }
            }
        },
    })
}

struct Candidate {
    /// Data offset of the white-space byte before `EI`, once known.
    at: Option<u64>,
    failed: bool,
}

enum Step {
    /// The white-space byte before the closing `EI` is at this data offset.
    End(u64),
    Unended,
    /// Needs more bytes; those before this data offset can be dropped.
    More { keep: u64 },
}

struct Scan {
    /// Data offset of the first byte of the window.
    base: u64,
    candidates: Vec<Candidate>,
    codec: Codec,
    codec_candidate: Option<usize>,
    /// Data offset the codec has been fed up to.
    fed: u64,
    fallback_next: u64,
    fallback_found: Option<u64>,
    /// Counts the bytes the fallback rule walks.
    stride: Stride<'static>,
}

impl Scan {
    fn new(spec: &ImageSpec) -> Self {
        let mut candidates = Vec::new();
        for length in [spec.length, spec.raw_length].into_iter().flatten() {
            candidates.push(Candidate { at: Some(length), failed: false });
        }
        let codec = match spec.rule {
            EndRule::Fallback => Codec::None,
            EndRule::AsciiHex => Codec::AsciiHex,
            EndRule::Ascii85 => Codec::Ascii85 { after_tilde: false },
            EndRule::RunLength => Codec::RunLength(RunState::Length),
            EndRule::Flate => Codec::Flate(Box::new(Decompress::new(true))),
        };
        let codec_candidate = if matches!(codec, Codec::None) {
            None
        } else {
            candidates.push(Candidate { at: None, failed: false });
            Some(candidates.len() - 1)
        };
        Self {
            base: 0,
            candidates,
            codec,
            codec_candidate,
            fed: 0,
            fallback_next: 0,
            fallback_found: None,
            stride: Stride::job(),
        }
    }

    fn step(&mut self, w: &[u8], eof: bool) -> Result<Step> {
        self.stride.tick()?;
        if let Some(index) = self.codec_candidate {
            let candidate = &mut self.candidates[index];
            if candidate.at.is_none() && !candidate.failed {
                let from = (self.fed - self.base) as usize;
                match feed(&mut self.codec, &w[from..])? {
                    Feed::End(n) => candidate.at = Some(self.fed + n as u64),
                    Feed::Failed => candidate.failed = true,
                    Feed::Pending => {
                        self.fed = self.base + w.len() as u64;
                        candidate.failed = eof;
                    },
                }
            }
        }

        while self.fallback_found.is_none() {
            self.stride.tick()?;
            let at = (self.fallback_next - self.base) as usize;
            if at >= w.len() {
                break;
            }
            match ei_at(w, self.base, self.fallback_next, eof) {
                Verdict::No => self.fallback_next += 1,
                Verdict::Wait => break,
                Verdict::Yes => match after_ei(&w[at + 3..], eof) {
                    Verdict::Yes => self.fallback_found = Some(self.fallback_next),
                    Verdict::No => self.fallback_next += 1,
                    Verdict::Wait => break,
                },
            }
        }

        for index in 0..self.candidates.len() {
            if self.candidates[index].failed {
                continue;
            }
            let Some(at) = self.candidates[index].at else {
                return Ok(self.more());
            };
            match ei_at(w, self.base, at, eof) {
                Verdict::Yes => return Ok(Step::End(at)),
                Verdict::No => self.candidates[index].failed = true,
                Verdict::Wait => return Ok(self.more()),
            }
        }

        if let Some(at) = self.fallback_found {
            return Ok(Step::End(at));
        }
        Ok(if eof { Step::Unended } else { self.more() })
    }

    fn more(&self) -> Step {
        let mut keep = self.fallback_found.unwrap_or(self.fallback_next);
        for candidate in &self.candidates {
            if let (false, Some(at)) = (candidate.failed, candidate.at) {
                keep = keep.min(at);
            }
        }
        Step::More { keep: keep.max(self.base) }
    }
}

impl<R: Read> Lexer<R> {
    /// Reads the data of an inline image, after its `ID` and the one
    /// white-space byte that follows it, and consumes it with the `EI`.
    /// Returns the data when `collect` is set and an empty vector otherwise,
    /// or `None` when the source ends before the data does. The outer `Err`
    /// is a cancel, polled every [`STRIDE`] bytes the scan walks; the inner
    /// one a read that failed.
    pub(super) fn read_inline_data(
        &mut self,
        spec: &ImageSpec,
        collect: bool,
    ) -> Result<io::Result<Option<Vec<u8>>>> {
        let mut scan = Scan::new(spec);
        loop {
            match scan.step(self.window(), self.at_eof())? {
                Step::End(at) => {
                    let data = if collect { self.window()[..at as usize].to_vec() } else { Vec::new() };
                    self.discard((at + 3 - scan.base) as usize);
                    return Ok(Ok(Some(data)));
                },
                Step::Unended => {
                    let rest = self.window().len();
                    self.discard(rest);
                    return Ok(Ok(None));
                },
                Step::More { keep } => {
                    if !collect && keep > scan.base {
                        self.discard((keep - scan.base) as usize);
                        scan.base = keep;
                    }
                    if let Err(e) = self.refill() {
                        return Ok(Err(e));
                    }
                },
            }
        }
    }
}
