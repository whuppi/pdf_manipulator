//! The windowed scans against the whole-buffer scans they replaced.
//!
//! Every file-level scan of the engine reads the source in 64 KiB windows
//! that overlap by 16 bytes (`host::scan`), and an object stream gives up one
//! member at a time over a reader that decodes forward (`objstm`,
//! `host::forward`). The oracles here are the whole-buffer scans as they were:
//! the regex over the file, the byte loops over the file, the decode of the
//! whole stream.
//!
//! No Dart battery can observe this: the batteries open and read documents
//! end to end, and what reaches them is the same whether a file was scanned
//! in one buffer or in windows; only a pattern placed on a window's edge, a
//! reopen counter and a refused whole read can separate the two.

use super::support::stream_object;
use crate::decoders::stream::TEST_ALLOCATION_LIMIT;
use crate::document::PdfDocument;
use crate::extractors::warnings::WarningSink;
use crate::host::scan::{self, Flow, OfSource, OVERLAP, WINDOW};
use crate::object::Object;
use crate::objstm::decode_object_stream;
use crate::parser::parse_object;
use crate::source::{PdfReader, Source};
use crate::xref_reconstruction::reconstruct_xref;
use std::collections::HashMap;
use std::io::{BufReader, Cursor, Write};
use std::sync::Arc;

fn source_of(bytes: Vec<u8>) -> Source {
    Source::new(PdfReader::Memory(BufReader::new(Cursor::new(bytes))), Arc::new(WarningSink::new()))
}

const WORDS: [&[u8]; 6] = [b"obj", b"endobj", b"xref", b"trailer", b"startxref", b"%%EOF"];

/// Every match of `WORDS` in `bytes`, found by looking at every offset.
fn naive_hits(bytes: &[u8]) -> Vec<(usize, u64)> {
    let mut out = Vec::new();
    for at in 0..bytes.len() {
        for (index, word) in WORDS.iter().enumerate() {
            if bytes[at..].starts_with(word) {
                out.push((index, at as u64));
            }
        }
    }
    out.sort_by_key(|&(index, at)| (at, index));
    out
}

/// What the windowed scan finds in `bytes`, read through a `Source`.
fn scanned_hits(bytes: Vec<u8>) -> Vec<(usize, u64)> {
    let source = source_of(bytes);
    let mut found = Vec::new();
    scan::scan(&mut OfSource(&source), 0, &WORDS, |hit, _| {
        found.push((hit.pattern, hit.offset));
        Ok(Flow::Continue)
    })
    .unwrap();
    found.sort_by_key(|&(index, at)| (at, index));
    found
}

/// Proves the window overlap: a pattern at every offset around the edge of a
/// window (inside the first window, inside the overlap, split by the edge at
/// each of its splits, inside the second window) is found, once. No Dart
/// battery can place a keyword on the 65536th byte of a file it opens.
#[test]
fn the_scanner_finds_every_pattern_at_every_offset_of_a_window_edge() {
    for edge in [WINDOW, 2 * WINDOW] {
        for (index, word) in WORDS.iter().enumerate() {
            let first = edge - word.len() - OVERLAP - 2;
            for start in first..=edge + 2 {
                let mut bytes = vec![b'.'; 3 * WINDOW + 100];
                bytes[start..start + word.len()].copy_from_slice(word);
                // Everything but the pattern is filler, so a look at every offset
                // around it is a look at every match.
                let from = start - 32;
                let expected: Vec<(usize, u64)> = naive_hits(&bytes[from..start + word.len() + 32])
                    .into_iter()
                    .map(|(i, at)| (i, at + from as u64))
                    .collect();
                assert!(expected.contains(&(index, start as u64)));
                assert_eq!(
                    scanned_hits(bytes),
                    expected,
                    "{:?} at {start} (the edge is at {edge}): the scan must find it once",
                    String::from_utf8_lossy(word)
                );
            }
        }
    }
}

/// A file that ends exactly on a window, and one a byte past it, give the
/// same matches as a look at every offset. No Dart battery reaches a file
/// whose length is a multiple of the window.
#[test]
fn the_scanner_finds_patterns_at_the_end_of_a_source_that_ends_on_a_window() {
    for len in [WINDOW - 1, WINDOW, WINDOW + 1, 2 * WINDOW] {
        for word in WORDS {
            let mut bytes = vec![b'.'; len];
            let at = len - word.len();
            bytes[at..].copy_from_slice(word);
            assert_eq!(scanned_hits(bytes.clone()), naive_hits(&bytes), "{word:?} ends a source of {len}");
        }
    }
}

/// The lines of a reader are the same whatever window a CR LF falls on. No
/// Dart battery can place a line ending on the 65536th byte of an xref table.
#[test]
fn lines_split_the_same_wherever_the_window_edge_falls() {
    for pad in WINDOW - 3..=WINDOW + 3 {
        let mut text = vec![b'a'; pad];
        text.extend_from_slice(b"\r\nb\rc\nd");
        let lines: Vec<String> = scan::Lines::new(Cursor::new(text)).collect();
        assert_eq!(lines, vec!["a".repeat(pad), "b".into(), "c".into(), "d".into()], "pad {pad}");
    }
}

/// `startxref` is found from the end wherever it sits against a window edge,
/// behind padding as long as several windows. No Dart battery can pad a file
/// to a chosen length.
#[test]
fn the_last_startxref_is_found_behind_padding_of_any_length() {
    for pad in (WINDOW - 20..=WINDOW + 20).chain([3 * WINDOW + 5]) {
        let mut bytes = b"%PDF-1.4\nstartxref\n1\n%%EOF\nstartxref\n123\n%%EOF\n".to_vec();
        bytes.resize(bytes.len() + pad, 0);
        let found = crate::xref::find_xref_offset(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(found, 123, "padding of {pad} bytes");
    }
}

/// The trailer is found to the first `trailer` of a reader that cannot seek,
/// wherever the keyword falls against a window edge. No Dart battery can
/// choose the byte a keyword starts at.
#[test]
fn parse_trailer_finds_the_keyword_on_a_window_edge() {
    for before in WINDOW - 9..=WINDOW + 1 {
        let mut bytes = vec![b'0'; before];
        bytes.extend_from_slice(b"trailer\n<< /Size 3 /Root 1 0 R >>\nstartxref\n9\n%%EOF\n");
        let trailer = crate::document::parse_trailer(&mut Cursor::new(bytes)).unwrap();
        let dict = trailer.as_dict().unwrap();
        assert_eq!(dict.get("Size"), Some(&Object::Integer(3)), "keyword at {before}");
        assert!(dict.contains_key("Root"));
    }
}

/// What the whole-buffer reconstruction found: `(number -> (offset, generation))`.
fn old_reconstruction(contents: &[u8]) -> HashMap<u32, (u64, u16)> {
    let re = regex::bytes::Regex::new(r"(\d+)\s+(\d+)\s+obj").unwrap();
    let mut out = HashMap::new();
    for capture in re.captures_iter(contents) {
        let whole = capture.get(0).unwrap();
        let number: Option<u32> = std::str::from_utf8(capture.get(1).unwrap().as_bytes())
            .ok()
            .and_then(|s| s.parse().ok());
        let generation: Option<u16> = std::str::from_utf8(capture.get(2).unwrap().as_bytes())
            .ok()
            .and_then(|s| s.parse().ok());
        let (Some(number), Some(generation)) = (number, generation) else { continue };
        let rest = &contents[whole.end()..];
        if let Some(next) = rest.iter().find(|b| !b.is_ascii_whitespace()) {
            let valid = matches!(next, b'<' | b'[' | b'(' | b'/' | b't' | b'f' | b'n' | b'-')
                || next.is_ascii_digit();
            if !valid {
                continue;
            }
        }
        out.insert(number, (whole.start() as u64, generation));
    }
    out
}

/// What the whole-buffer offset map of a lost object found.
fn old_object_offsets(content: &[u8]) -> HashMap<u32, u64> {
    let mut offsets = HashMap::new();
    let mut pos = 0;
    while pos < content.len() {
        let valid_start = pos == 0 || content[pos - 1] == b'\n' || content[pos - 1] == b'\r';
        if !valid_start || !content[pos].is_ascii_digit() {
            pos += 1;
            continue;
        }
        let start = pos;
        while pos < content.len() && content[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos >= content.len() || content[pos] != b' ' {
            continue;
        }
        let Ok(number) = std::str::from_utf8(&content[start..pos]).unwrap_or("").parse::<u32>() else {
            continue;
        };
        pos += 1;
        while pos < content.len() && content[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos >= content.len() || content[pos] != b' ' {
            continue;
        }
        pos += 1;
        if pos + 3 <= content.len() && &content[pos..pos + 3] == b"obj" {
            let after = pos + 3;
            let valid_end = after >= content.len()
                || matches!(content[after], b'\n' | b'\r' | b' ' | b'\t' | b'<');
            if valid_end {
                offsets.entry(number).or_insert(start as u64);
                pos = after;
                continue;
            }
        }
        pos = start + 1;
    }
    offsets
}

/// What the whole-buffer sweep for object streams found.
fn old_objstm_candidates(content: &[u8]) -> Vec<(u32, u64)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < content.len() {
        let valid_start = pos == 0
            || content[pos - 1] == b'\n'
            || content[pos - 1] == b'\r'
            || content[pos - 1] == b' ';
        if !valid_start || !content[pos].is_ascii_digit() {
            pos += 1;
            continue;
        }
        let header_start = pos;
        let num_start = pos;
        while pos < content.len() && content[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos >= content.len() || content[pos] != b' ' {
            pos = header_start + 1;
            continue;
        }
        let Some(number) = std::str::from_utf8(&content[num_start..pos])
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            pos = header_start + 1;
            continue;
        };
        pos += 1;
        let gen_start = pos;
        while pos < content.len() && content[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos >= content.len() || content[pos] != b' ' {
            pos = header_start + 1;
            continue;
        }
        if std::str::from_utf8(&content[gen_start..pos])
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .is_none()
        {
            pos = header_start + 1;
            continue;
        }
        pos += 1;
        if pos + 3 > content.len() || &content[pos..pos + 3] != b"obj" {
            pos = header_start + 1;
            continue;
        }
        let window = &content[pos..(pos + 2048).min(content.len())];
        let text = String::from_utf8_lossy(window);
        if text.contains("/Type /ObjStm") || text.contains("/Type/ObjStm") {
            out.push((number, header_start as u64));
        }
        pos = header_start + 1;
    }
    out
}

/// A damaged file: no xref table, a `startxref` that points nowhere, and in
/// it every kind of header the scans must agree on. `edge` puts object 3's
/// header that many bytes before the end of the first window.
struct Damaged {
    bytes: Vec<u8>,
    /// The headers the reconstruction must find, written down as they were laid out.
    expected: HashMap<u32, (u64, u16)>,
}

fn damaged(edge: usize, trailer: bool) -> Damaged {
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut expected = HashMap::new();
    let mut put = |out: &mut Vec<u8>, number: u32, generation: u16, text: &[u8]| {
        expected.insert(number, (out.len() as u64, generation));
        out.extend_from_slice(text);
    };
    put(&mut out, 1, 0, b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    put(&mut out, 2, 0, b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    // A comment of the length that puts the next header `edge` bytes short of the window's end.
    let pad = WINDOW - edge - out.len() - 2;
    out.push(b'%');
    out.extend(std::iter::repeat(b'x').take(pad));
    out.push(b'\n');
    assert_eq!(out.len(), WINDOW - edge);
    put(
        &mut out,
        3,
        0,
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R \
          /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let text = b"BT /F1 12 Tf 20 100 Td (Hello) Tj ET";
    put(&mut out, 4, 0, &stream_object(4, "", text));
    put(&mut out, 5, 0, b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");
    // Words that look like headers and are not, or are only to a lax reader.
    out.extend_from_slice(b"% 8 0 obj junk that is not an object\n");
    put(&mut out, 9, 0, b"9 0 obj [1 2]\nendobj\n");
    put(&mut out, 10, 0, b"10 0\r\nobj\r\n<< /K 1 >>\nendobj\n");
    put(&mut out, 11, 7, b"11   7 \t obj /Name\nendobj\n");
    // A second 9 0 obj: the later header wins.
    put(&mut out, 9, 0, b"9 0 obj [3 4]\nendobj\n");
    if trailer {
        out.extend_from_slice(b"trailer\n<< /Size 12 /Root 1 0 R >>\n");
    }
    out.extend_from_slice(b"startxref\n7\n%%EOF\n");
    Damaged { bytes: out, expected }
}

fn entries(xref: &crate::xref::CrossRefTable) -> HashMap<u32, (u64, u16)> {
    xref.all_object_numbers()
        .map(|n| {
            let e = xref.get(n).unwrap();
            (n, (e.offset, e.generation))
        })
        .collect()
}

/// A damaged file recovers the same objects through the windowed scan as
/// through the whole-buffer scan, with each header laid out at every offset
/// from 24 bytes short of a window's end to the end. No Dart battery can lay
/// a header on a chosen byte of a file it repairs.
#[test]
fn a_damaged_file_recovers_the_objects_the_whole_buffer_scan_found() {
    for edge in 1..=OVERLAP + 8 {
        for trailer in [true, false] {
            let file = damaged(edge, trailer);
            let (xref, found_trailer, _) = reconstruct_xref(&mut Cursor::new(file.bytes.clone())).unwrap();
            let got = entries(&xref);
            assert_eq!(got, file.expected, "edge {edge}: the offsets laid out in the file");
            assert_eq!(got, old_reconstruction(&file.bytes), "edge {edge}: the whole-buffer scan");
            let root = found_trailer.as_dict().unwrap().get("Root").cloned();
            assert_eq!(
                root,
                Some(Object::Reference(crate::object::ObjectRef::new(1, 0))),
                "edge {edge}, trailer {trailer}"
            );
        }
    }
}

/// The recovered document reads back: the pages and the text the file kept.
/// No Dart battery separates the scan that rebuilt the table from the scan
/// that used to.
#[test]
fn a_damaged_file_reads_back_through_the_recovered_table() {
    for edge in [1, OVERLAP, OVERLAP + 8] {
        let file = damaged(edge, true);
        let doc = PdfDocument::from_bytes(file.bytes).unwrap();
        assert!(doc.xref_repaired(), "the startxref points nowhere");
        assert_eq!(doc.page_count().unwrap(), 1);
        assert!(doc.extract_text(0).unwrap().contains("Hello"), "edge {edge}");
    }
}

/// The offset map of a lost object, and the sweep for object streams, find
/// what their whole-buffer loops found, with a header on every offset of the edge.
/// No Dart battery can lay a header on a chosen byte.
#[test]
fn the_offset_map_and_the_object_stream_sweep_agree_with_the_whole_buffer_loops() {
    for edge in 1..=OVERLAP + 8 {
        let mut file = damaged(edge, true).bytes;
        let member = b"6 0 obj\n<< /Type /ObjStm /N 1 /First 4 /Length 8 >>\nstream\n7 0 << >>\nendstream\nendobj\n";
        file.extend_from_slice(b" 12 0 obj\n<< /Type/ObjStm /N 0 /First 0 /Length 0 >>\nendobj\n");
        // On the edge of the *second* window too.
        let to_edge = 2 * WINDOW - edge;
        file.resize(to_edge, b' ');
        file.extend_from_slice(member);
        let source = source_of(file.clone());
        assert_eq!(scan::object_offsets(&source).unwrap(), old_object_offsets(&file), "edge {edge}");
        let mut got = scan::objstm_candidates(&source, crate::document::contains_objstm_marker).unwrap();
        got.sort();
        let mut want = old_objstm_candidates(&file);
        want.sort();
        assert_eq!(got, want, "edge {edge}");
        assert!(!want.is_empty());
    }
}

/// The members of an object stream: object `n` is `[n n*7]`, laid out in an
/// order that is not the order of their numbers, with a table that lists
/// them by number (so every read of the table's order goes back).
fn members(count: u32) -> (Vec<u8>, Vec<u8>, Vec<(u32, Object)>) {
    // A fixed shuffle: a multiplier coprime to `count` visits every slot once.
    let slot = |i: u32| (i as u64 * 7919 % count as u64) as u32;
    let mut by_slot = vec![0u32; count as usize];
    for i in 0..count {
        by_slot[slot(i) as usize] = i;
    }
    let mut data = Vec::new();
    let mut offset_of = HashMap::new();
    for number in &by_slot {
        offset_of.insert(*number, data.len());
        data.extend_from_slice(
            format!("[{number} {}]\n", *number as u64 * 7).as_bytes(),
        );
    }
    let mut table = Vec::new();
    for number in 0..count {
        table.extend_from_slice(format!("{} {} ", number + 100, offset_of[&number]).as_bytes());
    }
    let expected = (0..count)
        .map(|n| {
            let text = format!("[{n} {}]", n as u64 * 7);
            (n + 100, parse_object(text.as_bytes()).unwrap().1)
        })
        .collect();
    (table, data, expected)
}

/// The object stream of `table` and `data`, stored as it is: the proofs that
/// count reopens do not wait on a debug build's inflate.
fn object_stream(table: &[u8], data: &[u8], count: u32) -> Object {
    let mut dict = HashMap::new();
    dict.insert("Type".to_string(), Object::Name("ObjStm".into()));
    dict.insert("N".to_string(), Object::Integer(count as i64));
    dict.insert("First".to_string(), Object::Integer(table.len() as i64));
    Object::stream(dict, [table, data].concat())
}

/// An object stream of 10,000 members read in a random order gives every
/// member right. No Dart battery can name the order a document asks its
/// objects in.
#[test]
fn an_object_stream_of_10000_members_read_at_random_gives_every_member() {
    const COUNT: u32 = 10_000;
    let (table, data, expected) = members(COUNT);
    let stream = object_stream(&table, &data, COUNT);
    let decoded = decode_object_stream(&stream).unwrap();
    // A linear congruential order over all the members.
    let mut order: Vec<usize> = (0..COUNT as usize).collect();
    let mut state = 0x2545F491u64;
    for i in (1..order.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        order.swap(i, (state >> 33) as usize % (i + 1));
    }
    for i in order {
        let (number, want) = &expected[i];
        assert_eq!(decoded.member(*number).as_ref(), Some(want), "member {number}");
    }
    assert!(decoded.member(99).is_none(), "a number the table does not list");
}

/// `parse_object_stream` gives every member of a stream whose table lists
/// them out of the order they lie in, so reading the table's order reopens
/// the reader again and again. No Dart battery calls it: the recovery of a
/// damaged file does, with the members of whatever stream survived.
#[test]
fn parse_object_stream_gives_every_member_whatever_the_order_of_its_table() {
    const COUNT: u32 = 500;
    let (table, data, expected) = members(COUNT);
    let all = crate::objstm::parse_object_stream(&object_stream(&table, &data, COUNT)).unwrap();
    assert_eq!(all, expected.into_iter().collect::<HashMap<_, _>>());
}

/// A member read reopens the reader rather than holding the decoded stream:
/// members in rising order never reopen it, one before the reader's place
/// does, and a 64 MiB object stream is read while a whole read of it is
/// refused (so any read of it whole is a typed error, not a pass). No Dart
/// battery can refuse the engine an allocation or count its reopens.
#[test]
fn a_member_read_reopens_the_reader_and_never_holds_the_decoded_stream() {
    const MEMBERS: usize = 64;
    const MEMBER_LEN: usize = 1024 * 1024;
    // 64 members of 1 MiB: a literal string each, `(m<k> ` then filler.
    let mut table = Vec::new();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut body = Vec::new();
    for k in 0..MEMBERS {
        table.extend_from_slice(format!("{} {} ", 500 + k, k * MEMBER_LEN).as_bytes());
        let mut text = format!("(m{k} ").into_bytes();
        text.resize(MEMBER_LEN - 1, b'x');
        text.push(b')');
        body.push(text);
    }
    z.write_all(&table).unwrap();
    for text in &body {
        z.write_all(text).unwrap();
    }
    let mut dict = HashMap::new();
    dict.insert("Type".to_string(), Object::Name("ObjStm".into()));
    dict.insert("N".to_string(), Object::Integer(MEMBERS as i64));
    dict.insert("First".to_string(), Object::Integer(table.len() as i64));
    dict.insert("Filter".to_string(), Object::Name("FlateDecode".into()));
    let stream = Object::stream(dict, z.finish().unwrap());
    drop(body);

    // 4 MiB is the most a whole read may take; the decoded stream is 64 MiB.
    TEST_ALLOCATION_LIMIT.with(|l| l.set(Some(4 * 1024 * 1024)));
    let read = |decoded: &crate::objstm::DecodedObjStm, k: usize| -> Object {
        decoded.member(500 + k as u32).unwrap_or_else(|| panic!("member {k} read nothing"))
    };
    let check = |obj: &Object, k: usize| {
        let Object::String(bytes) = obj else { panic!("member {k} is {obj:?}") };
        assert_eq!(bytes.len(), MEMBER_LEN - 2, "member {k}");
        assert!(bytes.starts_with(format!("m{k} ").as_bytes()), "member {k} is another member's bytes");
    };
    assert!(
        stream.decode_stream_data().is_err(),
        "the refusal is live: a whole read of the 64 MiB stream is a typed error"
    );
    let decoded = decode_object_stream(&stream).expect("the table of a 64 MiB stream reads");
    for k in [0, 1, 2, 40, 41, 63] {
        check(&read(&decoded, k), k);
    }
    assert_eq!(decoded.reopens(), 0, "members in rising order are read on from where the reader stands");
    check(&read(&decoded, 5), 5);
    assert_eq!(decoded.reopens(), 1, "a member before the reader's place opens the stream again");
    check(&read(&decoded, 5), 5);
    check(&read(&decoded, 6), 6);
    assert_eq!(decoded.reopens(), 2, "the same member again is behind the reader, which stands at its end");
    check(&read(&decoded, 62), 62);
    check(&read(&decoded, 7), 7);
    assert_eq!(decoded.reopens(), 3);
    TEST_ALLOCATION_LIMIT.with(|l| l.set(None));
}

/// A document whose object stream holds the page tree opens, counts its
/// pages and reads a page's text through members read one at a time. No Dart
/// battery can tell a member parsed from the reader from one parsed from the
/// whole decoded stream; only the member values it reads back can.
#[test]
fn a_document_packed_in_an_object_stream_reads_through_single_members() {
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 5 0 R \
                /Resources << /Font << /F1 4 0 R >> >> >>";
    let members: [(u32, &str); 3] = [
        (1, "<< /Type /Catalog /Pages 2 0 R >>"),
        (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        (3, page),
    ];
    let mut table = String::new();
    let mut data = String::new();
    for (n, text) in members {
        table.push_str(&format!("{n} {} ", data.len()));
        data.push_str(text);
        data.push('\n');
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    z.write_all(table.as_bytes()).unwrap();
    z.write_all(data.as_bytes()).unwrap();
    let packed = z.finish().unwrap();

    let mut pdf = b"%PDF-1.5\n".to_vec();
    let mut offsets = HashMap::new();
    let mut add = |pdf: &mut Vec<u8>, n: u32, body: Vec<u8>| {
        offsets.insert(n, pdf.len());
        pdf.extend_from_slice(&body);
    };
    add(
        &mut pdf,
        6,
        stream_object(
            6,
            &format!("/Type /ObjStm /N 3 /First {} /Filter /FlateDecode", table.len()),
            &packed,
        ),
    );
    add(&mut pdf, 4, b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec());
    add(&mut pdf, 5, stream_object(5, "", b"BT /F1 12 Tf 20 100 Td (Packed) Tj ET"));
    let xref_at = pdf.len();
    // A cross-reference stream: type, field 2, field 3 in 1 + 4 + 2 bytes.
    let mut rows = Vec::new();
    let mut row = |kind: u8, a: u32, b: u16| {
        rows.push(kind);
        rows.extend_from_slice(&a.to_be_bytes());
        rows.extend_from_slice(&b.to_be_bytes());
    };
    row(0, 0, 65535);
    row(2, 6, 0);
    row(2, 6, 1);
    row(2, 6, 2);
    row(1, offsets[&4] as u32, 0);
    row(1, offsets[&5] as u32, 0);
    row(1, offsets[&6] as u32, 0);
    row(1, xref_at as u32, 0);
    pdf.extend_from_slice(&stream_object(
        7,
        "/Type /XRef /Size 8 /W [1 4 2] /Root 1 0 R",
        &rows,
    ));
    pdf.extend_from_slice(format!("startxref\n{xref_at}\n%%EOF\n").as_bytes());

    let doc = PdfDocument::from_bytes(pdf).unwrap();
    assert!(!doc.xref_repaired(), "the cross-reference stream is sound");
    assert_eq!(doc.page_count().unwrap(), 1);
    assert!(doc.extract_text(0).unwrap().contains("Packed"));
}
