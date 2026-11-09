//! Object stream parsing (PDF 1.5+).
//!
//! Object streams (/Type /ObjStm) allow multiple objects to be compressed together
//! in a single stream for better compression ratios. This module handles parsing
//! these streams and extracting individual objects.
//!
//! # Format
//!
//! An object stream has this structure:
//! ```text
//! N 0 obj
//! << /Type /ObjStm
//!    /N 5              % Number of objects in stream
//!    /First 30         % Byte offset to first object's data
//!    /Filter /FlateDecode
//! >>
//! stream
//! 10 0 11 15 12 28 13 42 14 55    % Pairs: (obj_num, offset)
//! <dict>                           % Object 10 at offset 0
//! <array>                          % Object 11 at offset 15
//! ...
//! endstream
//! endobj
//! ```
//!
//! The first part contains N pairs of integers (object number, byte offset relative
//! to /First). The second part contains the actual object data.

use crate::error::{Error, Result};
use crate::object::Object;
use crate::parser::parse_object;
use std::collections::HashMap;

/// Parse an object stream and extract all objects.
///
// ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
/// The stream's own extent decides whether its data is decrypted (an object
/// stream is not, per ISO 32000-2 §7.6.3).
// ── end pdf_manipulator patch ──
///
/// # Arguments
///
/// * `stream_obj` - The object stream object (must be a Stream with /Type /ObjStm)
///
/// # Returns
///
/// A HashMap mapping object numbers to their parsed objects.
///
// ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
/// # Errors
///
/// Returns an error if:
/// - The object is not a stream
/// - The stream is not a valid object stream (/Type /ObjStm)
/// - Required dictionary entries (/N, /First) are missing
/// - Stream decoding/decryption fails
/// - Object parsing fails
///
// ── end pdf_manipulator patch ──
/// # Example
///
/// ```ignore
/// use pdf_oxide::objstm::parse_object_stream;
/// use pdf_oxide::object::Object;
///
/// // Assuming we have loaded an object stream
/// let objects = parse_object_stream(&stream_obj)?;
/// let obj_10 = objects.get(&10).unwrap();
/// # Ok::<(), pdf_oxide::error::Error>(())
/// ```
pub fn parse_object_stream(stream_obj: &Object) -> Result<HashMap<u32, Object>> {
    // ── pdf_manipulator patch: object stream members parse one at a time ──
    // The members are read through the stream's decoded reader, one after
    // the other in the order of the table; the objects they parse to are the
    // result, and the decoded stream is never held.
    let decoded = decode_object_stream(stream_obj)?;
    let mut result = HashMap::new();
    for &(obj_num, offset) in &decoded.pairs {
        if let Some(obj) = decoded.member_at(obj_num, offset) {
            result.insert(obj_num, obj);
        }
    }
    Ok(result)
    // ── end pdf_manipulator patch ──
}

/// Parse the pairs section of an object stream.
///
/// The pairs section contains N pairs of integers: (object_number, offset).
/// The offset is relative to the start of the objects data section.
///
/// # Arguments
///
/// * `data` - The pairs section data (before /First offset)
/// * `count` - Expected number of pairs (from /N)
///
/// # Returns
///
/// A vector of (object_number, offset) tuples.
fn parse_object_number_pairs(data: &[u8], count: usize) -> Result<Vec<(u32, usize)>> {
    // ── pdf_manipulator patch: the declared /N is checked against the table that holds it, and allocated fallibly ──
    // A pair is two integers and a separator, at least 3 bytes.
    if count > data.len() / 3 + 1 {
        return Err(Error::InvalidPdf(format!(
            "object stream declares {count} objects but its table holds {} bytes",
            data.len()
        )));
    }
    let mut pairs = Vec::new();
    pairs.try_reserve_exact(count).map_err(|_| {
        Error::InvalidPdf(format!("cannot allocate the table of {count} objects"))
    })?;
    // ── end pdf_manipulator patch ──
    let mut remaining = data;

    for i in 0..count {
        // Skip whitespace
        remaining = skip_whitespace(remaining);

        // Parse object number
        let (rest, obj_num_str) =
            read_integer_string(remaining).ok_or_else(|| Error::ParseError {
                offset: 0,
                reason: format!("failed to parse object number for pair {}", i),
            })?;

        let obj_num: u32 = obj_num_str.parse().map_err(|_| Error::ParseError {
            offset: 0,
            reason: format!("invalid object number: {}", obj_num_str),
        })?;

        remaining = skip_whitespace(rest);

        // Parse offset
        let (rest, offset_str) =
            read_integer_string(remaining).ok_or_else(|| Error::ParseError {
                offset: 0,
                reason: format!("failed to parse offset for pair {}", i),
            })?;

        let offset: usize = offset_str.parse().map_err(|_| Error::ParseError {
            offset: 0,
            reason: format!("invalid offset: {}", offset_str),
        })?;

        pairs.push((obj_num, offset));
        remaining = rest;
    }

    Ok(pairs)
}

/// Skip PDF whitespace characters.
///
/// PDF whitespace: null (0), tab (9), LF (10), FF (12), CR (13), space (32)
fn skip_whitespace(data: &[u8]) -> &[u8] {
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            0 | 9 | 10 | 12 | 13 | 32 => i += 1,
            _ => break,
        }
    }
    &data[i..]
}

/// Read an integer string from the input.
///
/// Reads consecutive digit characters (with optional leading sign).
/// Returns the remaining input and the integer string.
fn read_integer_string(data: &[u8]) -> Option<(&[u8], String)> {
    if data.is_empty() {
        return None;
    }

    let mut i = 0;

    // Optional sign
    if data[i] == b'+' || data[i] == b'-' {
        i += 1;
    }

    // Must have at least one digit
    let start = i;
    while i < data.len() && data[i].is_ascii_digit() {
        i += 1;
    }

    if i == start {
        return None; // No digits found
    }

    let int_str = String::from_utf8_lossy(&data[..i]).to_string();
    Some((&data[i..], int_str))
}

// ── pdf_manipulator patch: object stream members parse one at a time ──
/// An object stream opened: the offset of each member and the means to read
/// one. `member` decodes forward to a single member and parses just that
/// object, so answering one request never builds the whole stream's objects
/// (a stream of large arrays holds hundreds of megabytes parsed, a few
/// megabytes decoded) and never holds its decoded bytes. A member before the
/// reader's place reopens the stream: a stream decodes forward only.
pub struct DecodedObjStm {
    /// Where the members begin in the decoded data.
    first: usize,
    /// The table's pairs, in the table's order.
    pairs: Vec<(u32, usize)>,
    /// Each member's offset (relative to `first`); the last pair of a number
    /// wins.
    offsets: HashMap<u32, usize>,
    /// The distinct offsets, ascending: a member ends where the next begins.
    starts: Vec<usize>,
    bytes: crate::host::forward::Forward,
}

impl std::fmt::Debug for DecodedObjStm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DecodedObjStm {{ members: {} }}", self.pairs.len())
    }
}

impl DecodedObjStm {
    /// Times a member read went back and opened the stream again.
    #[cfg(test)]
    pub(crate) fn reopens(&self) -> usize {
        self.bytes.reopens()
    }

    /// Parses member `obj_num`; `None` when the stream does not hold it or
    /// its bytes do not parse.
    pub fn member(&self, obj_num: u32) -> Option<Object> {
        let offset = *self.offsets.get(&obj_num)?;
        self.member_at(obj_num, offset)
    }

    fn member_at(&self, obj_num: u32, offset: usize) -> Option<Object> {
        let next = self.starts.partition_point(|&s| s <= offset);
        let len = self.starts.get(next).map_or(u64::MAX, |&end| (end - offset) as u64);
        let bytes = match self.bytes.read_at((self.first + offset) as u64, len) {
            Ok(bytes) => bytes,
            Err(e) => {
                log::warn!("Failed to read object {} from stream at offset {}: {}", obj_num, offset, e);
                return None;
            },
        };
        if bytes.is_empty() {
            log::warn!(
                "Object {} offset {} is beyond stream data length",
                obj_num,
                offset
            );
            return None;
        }
        match parse_object(&bytes) {
            Ok((_remaining, obj)) => Some(obj),
            Err(e) => {
                log::warn!(
                    "Failed to parse object {} from stream at offset {}: {:?}",
                    obj_num,
                    offset,
                    e
                );
                None
            },
        }
    }
}

/// Opens an object stream and reads its offset table, parsing no member.
/// The argument and errors are those of `parse_object_stream`.
pub fn decode_object_stream(stream_obj: &Object) -> Result<DecodedObjStm> {
    use std::io::Read;

    let (dict, body) = match stream_obj {
        Object::Stream { dict, body } => (dict, body),
        _ => return Err(Error::InvalidPdf("object stream is not a Stream object".to_string())),
    };
    if let Some(type_name) = dict.get("Type").and_then(|t| t.as_name()) {
        if type_name != "ObjStm" {
            return Err(Error::InvalidPdf(format!(
                "expected /Type /ObjStm, got /Type /{}",
                type_name
            )));
        }
    }
    let n = dict
        .get("N")
        .and_then(|o| o.as_integer())
        .ok_or_else(|| Error::InvalidPdf("object stream missing /N entry".to_string()))?;
    let first = dict
        .get("First")
        .and_then(|o| o.as_integer())
        .ok_or_else(|| Error::InvalidPdf("object stream missing /First entry".to_string()))?;
    if !(0..=1_000_000).contains(&n) {
        return Err(Error::InvalidPdf(format!("invalid object stream /N value: {}", n)));
    }
    if !(0..=10_000_000).contains(&first) {
        return Err(Error::InvalidPdf(format!("invalid object stream /First value: {}", first)));
    }
    let first = first as usize;

    let open: Box<dyn Fn() -> Result<Box<dyn Read>> + Send + Sync> = {
        let dict = dict.clone();
        let body = body.clone();
        Box::new(move || {
            let owned = body.clone();
            crate::decoders::stream::reader_in(&dict, body.origin(), move || owned.owned_reader())
        })
    };
    let mut reader = open()?;
    let mut table = Vec::new();
    reader
        .by_ref()
        .take(first as u64)
        .read_to_end(&mut table)
        .map_err(crate::encryption::stream::from_io)?;
    if table.len() < first {
        return Err(Error::InvalidPdf(format!(
            "object stream data too short: {} bytes, expected at least {}",
            table.len(),
            first
        )));
    }
    let pairs = parse_object_number_pairs(&table, n as usize)?;
    let offsets: HashMap<u32, usize> = pairs.iter().copied().collect();
    let mut starts: Vec<usize> = pairs.iter().map(|&(_, offset)| offset).collect();
    starts.sort_unstable();
    starts.dedup();
    Ok(DecodedObjStm {
        first,
        pairs,
        offsets,
        starts,
        bytes: crate::host::forward::Forward::new(open, Some((reader, first as u64))),
    })
}
// ── end pdf_manipulator patch ──

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::collections::HashMap;

    #[test]
    fn test_skip_whitespace() {
        assert_eq!(skip_whitespace(b"   hello"), b"hello");
        assert_eq!(skip_whitespace(b"\t\n\r hello"), b"hello");
        assert_eq!(skip_whitespace(b"hello"), b"hello");
        assert_eq!(skip_whitespace(b""), b"");
    }

    #[test]
    fn test_read_integer_string() {
        assert_eq!(read_integer_string(b"123 rest"), Some((&b" rest"[..], "123".to_string())));
        assert_eq!(read_integer_string(b"-456 rest"), Some((&b" rest"[..], "-456".to_string())));
        assert_eq!(read_integer_string(b"+789"), Some((&b""[..], "+789".to_string())));
        assert_eq!(read_integer_string(b"notanumber"), None);
        assert_eq!(read_integer_string(b""), None);
    }

    #[test]
    fn test_parse_object_number_pairs() {
        let data = b"10 0 11 15 12 28";
        let pairs = parse_object_number_pairs(data, 3).unwrap();

        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], (10, 0));
        assert_eq!(pairs[1], (11, 15));
        assert_eq!(pairs[2], (12, 28));
    }

    #[test]
    fn test_parse_object_number_pairs_with_whitespace() {
        let data = b"  10   0   11  15  12   28  ";
        let pairs = parse_object_number_pairs(data, 3).unwrap();

        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], (10, 0));
        assert_eq!(pairs[1], (11, 15));
        assert_eq!(pairs[2], (12, 28));
    }

    #[test]
    fn test_parse_object_stream_basic() {
        // Create a simple object stream with two objects
        // Object 10: integer 42
        // Object 11: name /Test

        let pairs_data = b"10 0 11 3"; // obj 10 at offset 0, obj 11 at offset 3
        let objects_data = b"42 /Test"; // "42 " is 3 bytes, then "/Test"

        let mut combined = Vec::new();
        combined.extend_from_slice(pairs_data);
        combined.push(b' '); // separator
        combined.extend_from_slice(objects_data);

        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
        dict.insert("N".to_string(), Object::Integer(2));
        dict.insert("First".to_string(), Object::Integer(9)); // Length of pairs section
        dict.insert("Length".to_string(), Object::Integer(combined.len() as i64));

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        let stream = Object::stream(dict, Bytes::from(combined));
        // ── end pdf_manipulator patch ──

        let objects = parse_object_stream(&stream).unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects.get(&10).unwrap().as_integer(), Some(42));
        assert_eq!(objects.get(&11).unwrap().as_name(), Some("Test"));
    }

    #[test]
    fn test_parse_object_stream_not_stream() {
        let obj = Object::Integer(42);
        let result = parse_object_stream(&obj);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_object_stream_missing_type() {
        let mut dict = HashMap::new();
        dict.insert("N".to_string(), Object::Integer(1));
        dict.insert("First".to_string(), Object::Integer(5));

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        let stream = Object::stream(dict, Bytes::from(b"1 0 42".to_vec()));
        // ── end pdf_manipulator patch ──

        // Should still work - Type is optional if we trust the caller
        let result = parse_object_stream(&stream);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_object_stream_missing_n() {
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
        dict.insert("First".to_string(), Object::Integer(5));

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        let stream = Object::stream(dict, Bytes::from(b"1 0 42".to_vec()));
        // ── end pdf_manipulator patch ──

        let result = parse_object_stream(&stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_object_stream_missing_first() {
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
        dict.insert("N".to_string(), Object::Integer(1));

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        let stream = Object::stream(dict, Bytes::from(b"1 0 42".to_vec()));
        // ── end pdf_manipulator patch ──

        let result = parse_object_stream(&stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_object_stream_invalid_n() {
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
        dict.insert("N".to_string(), Object::Integer(-1));
        dict.insert("First".to_string(), Object::Integer(5));

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        let stream = Object::stream(dict, Bytes::from(b"1 0 42".to_vec()));
        // ── end pdf_manipulator patch ──

        let result = parse_object_stream(&stream);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_object_stream_data_too_short() {
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
        dict.insert("N".to_string(), Object::Integer(1));
        dict.insert("First".to_string(), Object::Integer(100)); // Too large

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        let stream = Object::stream(dict, Bytes::from(b"1 0 42".to_vec()));
        // ── end pdf_manipulator patch ──

        let result = parse_object_stream(&stream);
        assert!(result.is_err());
    }
}
