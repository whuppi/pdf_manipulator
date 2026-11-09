//! Flate re-encoding for the full-rewrite save.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A save re-deflates a stream at the save's level and keeps the result only
//! when it is strictly smaller than the bytes the source held. Only the zlib
//! or LZW layer is undone and the zlib layer redone: predictor bytes stay as
//! they are, so the stream's `/DecodeParms` remain valid. Every function
//! here streams in [`CHUNK`] pieces, so a stream that inflates to many times
//! its size is never held decoded.

use crate::object::Object;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read, Write};

/// The largest level a save accepts.
pub(crate) const MAX_LEVEL: u32 = 9;

/// A stream whose source data is at most this long is re-encoded into memory
/// and compared; a longer one is first re-encoded into a byte counter.
pub(crate) const MEMORY_STREAM: u64 = 256 * 1024;

const CHUNK: usize = 64 * 1024;

/// How a stream is encoded, as far as a re-deflate is concerned.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// No `/Filter`: the data is the content itself.
    Raw,
    /// A single `FlateDecode`, bare or in a one-element array.
    Flate,
    /// A single `LZWDecode`, bare or in a one-element array, whose
    /// parameters are written in the dictionary itself (an indirect
    /// `/DecodeParms` could name a different `/EarlyChange`). The save
    /// writes it as Flate.
    Lzw { early_change: bool },
    /// Any other filter or a chain of filters: copied as it is.
    Other,
}

pub(crate) fn encoding(dict: &HashMap<String, Object>) -> Encoding {
    let name = |n: &str| match n {
        "FlateDecode" => Encoding::Flate,
        "LZWDecode" => Encoding::Lzw {
            early_change: early_change(dict),
        },
        _ => Encoding::Other,
    };
    let encoding = match dict.get("Filter") {
        None | Some(Object::Null) => Encoding::Raw,
        Some(Object::Name(n)) => name(n),
        Some(Object::Array(a)) => match a.as_slice() {
            [] => Encoding::Raw,
            [Object::Name(n)] => name(n),
            _ => Encoding::Other,
        },
        Some(_) => Encoding::Other,
    };
    match (encoding, dict.get("DecodeParms")) {
        (Encoding::Lzw { .. }, Some(Object::Reference(_))) => Encoding::Other,
        (Encoding::Lzw { .. }, Some(Object::Array(a)))
            if !matches!(a.as_slice(), [Object::Dictionary(_) | Object::Null]) =>
        {
            Encoding::Other
        },
        (encoding, _) => encoding,
    }
}

/// The `/EarlyChange` of a stream's inline `/DecodeParms`; 1 when it names none.
fn early_change(dict: &HashMap<String, Object>) -> bool {
    let parms = match dict.get("DecodeParms") {
        Some(Object::Dictionary(d)) => Some(d),
        Some(Object::Array(a)) => a.first().and_then(Object::as_dict),
        _ => None,
    };
    parms
        .and_then(|d| d.get("EarlyChange"))
        .and_then(Object::as_integer)
        .map_or(true, |v| v != 0)
}

/// Rewrites the `/Filter` of a stream that was `Lzw` as a `FlateDecode`,
/// in the form it had, and drops its `/EarlyChange`, which Flate has no use
/// for. The other parameters (the predictor's) stay valid.
pub(crate) fn lzw_to_flate(dict: &mut HashMap<String, Object>) {
    let flate = Object::Name("FlateDecode".to_string());
    let is_array = matches!(dict.get("Filter"), Some(Object::Array(_)));
    dict.insert(
        "Filter".to_string(),
        if is_array {
            Object::Array(vec![flate])
        } else {
            flate
        },
    );
    let strip = |parms: &mut HashMap<String, Object>| {
        parms.remove("EarlyChange");
        parms.is_empty()
    };
    let empty = match dict.get_mut("DecodeParms") {
        Some(Object::Dictionary(d)) => strip(d),
        Some(Object::Array(a)) => match a.first_mut() {
            Some(Object::Dictionary(d)) => strip(d),
            _ => false,
        },
        _ => false,
    };
    if empty {
        dict.remove("DecodeParms");
    }
}

/// The zlib level a save level runs at. The largest level runs zlib-rs level
/// 8: level 9 hashes three bytes and so takes length-3 matches, which cost
/// more than the literals they replace in text-like data (page content), and
/// level 8 hashes four. The zlib of `dart:io` at level 9 takes none either,
/// and level 8 here matches its output byte for byte on content streams.
pub(crate) fn zlib_level(level: u32) -> u32 {
    if level == MAX_LEVEL {
        level - 1
    } else {
        level
    }
}

/// Deflates `input`, which is encoded as `source`, into `out` at `level`:
/// first inflating it for Flate, or decoding it for LZW (a stream that stops
/// without its end code is an error, not a short stream). An error from
/// `input` (a corrupt stream, a failed read) and one from `out` look alike
/// here; callers use an `out` that cannot fail when they must tell them apart.
pub(crate) fn recode<'a, R: Read + 'a>(
    input: R,
    source: Encoding,
    level: u32,
    out: impl Write,
) -> io::Result<()> {
    let mut input: Box<dyn Read + 'a> = match source {
        Encoding::Flate => Box::new(ZlibDecoder::new(input)),
        Encoding::Lzw { early_change } => {
            let once = RefCell::new(Some(input));
            let params = crate::decoders::DecodeParams {
                early_change,
                ..Default::default()
            };
            crate::decoders::stream::chain(&["LZWDecode".to_string()], Some(&params), move || {
                let taken = once.borrow_mut().take();
                taken
                    .map(|r| Box::new(r) as Box<dyn Read + 'a>)
                    .ok_or_else(|| {
                        crate::error::Error::Decode("an LZW input is read once".to_string())
                    })
            })
            .map_err(io::Error::other)?
        },
        _ => Box::new(input),
    };
    let mut enc = ZlibEncoder::new(out, Compression::new(zlib_level(level)));
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        enc.write_all(&buf[..n])?;
    }
    enc.finish()?;
    Ok(())
}

/// The deflated form of `data`, or `None` when the stream is not eligible,
/// does not inflate, or the result is not strictly smaller than `data`.
pub(crate) fn smaller(data: &[u8], encoding: Encoding, level: u32) -> Option<Vec<u8>> {
    if encoding == Encoding::Other {
        return None;
    }
    let mut out = Vec::new();
    recode(data, encoding, level, &mut out).ok()?;
    (out.len() < data.len()).then_some(out)
}
