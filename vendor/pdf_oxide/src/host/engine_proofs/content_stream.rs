//! The streaming content lexer and `Operators` against `parse_content_stream`.
//!
//! A source that returns a few bytes per read, and a window of 16 bytes, put
//! every token boundary on a refill. Equality is after merging adjacent `TJ`
//! operators, and after dropping the stray `I` operator `parse_content_stream`
//! leaves behind each inline image (its `EI` skip stops one byte short; a
//! fixture pins that).

use crate::content::{parse_content_stream, Operator};
use crate::document::PdfDocument;
use crate::host::content::operators::TJ_CHUNK;
use crate::host::content::{Lexer, Mode, Operators};
use super::support::corpus_pdfs;
use crate::object::Object;
use std::io::{self, Read};

const CHUNKS: [usize; 7] = [1, 2, 3, 7, 64, 1024, 65536];
pub(super) const WINDOWS: [usize; 2] = [16, 64 * 1024];
/// Runs on the first 64 KiB of a corpus stream: the chunk sizes that read a byte or two at a time.
const HEAD_RUNS: [(usize, usize); 4] = [(1, 16), (1, 65536), (2, 16), (2, 65536)];
/// Runs on a whole corpus stream: every other chunk size once, the windows alternating.
const WHOLE_RUNS: [(usize, usize); 5] = [(3, 16), (7, 65536), (64, 16), (1024, 65536), (65536, 16)];
const CORPUS_HEAD: usize = 64 * 1024;

/// A source that returns at most `n` bytes per read.
pub(super) struct Trickle<'a> {
    pub(super) data: &'a [u8],
    pub(super) n: usize,
}

impl Read for Trickle<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let take = self.n.min(out.len()).min(self.data.len());
        out[..take].copy_from_slice(&self.data[..take]);
        self.data = &self.data[take..];
        Ok(take)
    }
}

fn operators_of(data: &[u8], n: usize, window: usize) -> Vec<Operator> {
    let lexer = Lexer::with_window(Trickle { data, n }, window);
    Operators::from_lexer(lexer, Mode::Full).map(|op| op.unwrap()).collect()
}

pub(super) fn merge_tj(ops: Vec<Operator>) -> Vec<Operator> {
    let mut out: Vec<Operator> = Vec::with_capacity(ops.len());
    for op in ops {
        if let (Operator::TJ { array }, Some(Operator::TJ { array: last })) = (&op, out.last_mut())
        {
            last.extend(array.iter().cloned());
            continue;
        }
        out.push(op);
    }
    out
}

fn is_stray_i(op: &Operator) -> bool {
    matches!(op, Operator::Other { name, operands } if name == "I" && operands.is_empty())
}

/// `parse_content_stream` as the reference: TJ merged, the stray `I` after
/// each inline image dropped. Returns the operators and how many `I` it dropped.
pub(super) fn reference(data: &[u8]) -> (Vec<Operator>, usize) {
    let mut out: Vec<Operator> = Vec::new();
    let mut dropped = 0;
    for op in parse_content_stream(data).unwrap() {
        if is_stray_i(&op) && matches!(out.last(), Some(Operator::InlineImage { .. })) {
            dropped += 1;
            continue;
        }
        out.push(op);
    }
    (merge_tj(out), dropped)
}

pub(super) fn short(op: &Operator) -> String {
    let mut text = format!("{op:?}");
    if text.len() > 160 {
        text.truncate(160);
        text.push_str("...");
    }
    text
}

#[derive(Default)]
struct Totals {
    streams: usize,
    operators: usize,
    runs: usize,
    strays: usize,
    excluded: usize,
    mismatches: Vec<String>,
}

/// Why `Operators` is meant to differ from `parse_content_stream` on this
/// input, if it is (the module docs list each divergence; a fixture pins it).
pub(super) fn known_divergence(data: &[u8]) -> Option<&'static str> {
    known_divergence_given(data, &reference(data).0)
}

/// [`known_divergence`] for a caller that already has the reference.
pub(super) fn known_divergence_given(data: &[u8], expected: &[Operator]) -> Option<&'static str> {
    if data.iter().any(|&b| matches!(b, b'%' | 0x00 | 0x0C)) {
        return Some("comment, NUL or form feed read as white-space");
    }
    let mut ops = Operators::new(data, Mode::Full);
    ops.by_ref().for_each(drop);
    if ops.damage().inline_unended {
        return Some("an inline image that never ends ends the stream; its data is not read again");
    }
    let text = format!("{expected:?}");
    if text.contains("Reference(") {
        return Some("n n R inside an array or dictionary is not a reference");
    }
    let named = |name: &str| text.contains(&format!("Other {{ name: \"{name}\""));
    if named("R") {
        return Some("n n R is two integers and the operator R");
    }
    if named("true") || named("false") || named("null") {
        return Some("true, false and null are operands");
    }
    None
}

impl Totals {
    /// Compares one stream at every chunk size and window.
    fn compare(&mut self, label: &str, data: &[u8]) {
        let every: Vec<(usize, usize)> =
            CHUNKS.iter().flat_map(|&n| WINDOWS.iter().map(move |&w| (n, w))).collect();
        let operators = self.compare_at(label, data, &every);
        self.streams += 1;
        self.operators += operators;
    }

    /// Compares `Operators` with the reference at the given (chunk size,
    /// window) pairs; returns the operators the reference has.
    fn compare_at(&mut self, label: &str, data: &[u8], runs: &[(usize, usize)]) -> usize {
        let (expected, dropped) = reference(data);
        self.strays += dropped;
        let results: Vec<Vec<Operator>> = std::thread::scope(|scope| {
            let handles: Vec<_> = runs
                .iter()
                .map(|&(n, window)| scope.spawn(move || merge_tj(operators_of(data, n, window))))
                .collect();
            handles.into_iter().map(|handle| handle.join().unwrap()).collect()
        });
        for (&(n, window), got) in runs.iter().zip(results) {
            self.runs += 1;
            if got == expected {
                continue;
            }
            let at = got
                .iter()
                .zip(&expected)
                .position(|(a, b)| a != b)
                .unwrap_or(got.len().min(expected.len()));
            self.mismatches.push(format!(
                "{label} chunk {n} window {window}: {} operators, expected {}; first difference at {at}: got {}, expected {}",
                got.len(),
                expected.len(),
                got.get(at).map_or("none".into(), short),
                expected.get(at).map_or("none".into(), short),
            ));
        }
        expected.len()
    }

    fn report(&self, what: &str) {
        println!(
            "{what}: {} streams, {} operators, {} runs, {} stray I dropped from the reference, {} mismatches",
            self.streams,
            self.operators,
            self.runs,
            self.strays,
            self.mismatches.len()
        );
        for m in &self.mismatches {
            println!("MISMATCH {m}");
        }
    }
}

fn assert_parity(label: &str, data: &[u8]) {
    let mut totals = Totals::default();
    totals.compare(label, data);
    assert!(totals.mismatches.is_empty(), "{:#?}", totals.mismatches);
}

/// The `b"..."` literals of the parser's own test module.
fn parser_test_samples() -> Vec<Vec<u8>> {
    let src = include_str!("../../content/parser.rs");
    let tests = src[src.find("mod tests").unwrap()..].as_bytes();
    let mut samples = Vec::new();
    let mut i = 0;
    while i + 1 < tests.len() {
        let lone_b = tests[i] == b'b' && (i == 0 || !tests[i - 1].is_ascii_alphanumeric());
        if !(lone_b && tests[i + 1] == b'"') {
            i += 1;
            continue;
        }
        i += 2;
        let mut lit = Vec::new();
        while i < tests.len() && tests[i] != b'"' {
            if tests[i] != b'\\' {
                lit.push(tests[i]);
                i += 1;
                continue;
            }
            i += 1;
            match tests[i] {
                b'n' => lit.push(b'\n'),
                b'r' => lit.push(b'\r'),
                b't' => lit.push(b'\t'),
                b'0' => lit.push(0),
                b'x' => {
                    let hex = std::str::from_utf8(&tests[i + 1..i + 3]).unwrap();
                    lit.push(u8::from_str_radix(hex, 16).unwrap());
                    i += 2;
                },
                b'\n' => {
                    while tests[i + 1].is_ascii_whitespace() {
                        i += 1;
                    }
                },
                other => lit.push(other),
            }
            i += 1;
        }
        i += 1;
        samples.push(lit);
    }
    samples
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn operators_equal_parse_content_stream_on_every_page_of_every_fixture_pdf() {
    let mut totals = Totals::default();
    let mut pdfs = 0;
    for path in corpus_pdfs() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Ok(doc) = PdfDocument::open(&path) else {
            println!("skipped, does not open: {name}");
            continue;
        };
        pdfs += 1;
        for page in 0..doc.page_count().unwrap_or(0) {
            if let Ok(content) = doc.get_page_content_data(page) {
                let label = format!("{name} page {page}");
                let head = &content[..content.len().min(CORPUS_HEAD)];
                totals.compare_at(&format!("{label} head"), head, &HEAD_RUNS);
                let operators = totals.compare_at(&label, &content, &WHOLE_RUNS);
                totals.operators += operators;
                totals.streams += 1;
            }
        }
    }
    println!("{pdfs} pdfs");
    totals.report("fixture pdfs");
    assert!(totals.streams > 0, "no content stream compared");
    assert!(totals.mismatches.is_empty());
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn operators_equal_parse_content_stream_on_the_parser_test_samples() {
    let samples = parser_test_samples();
    let mut totals = Totals::default();
    for (i, sample) in samples.iter().enumerate() {
        let label = format!("sample {i} {:?}", String::from_utf8_lossy(sample));
        if let Some(cause) = known_divergence(sample) {
            println!("EXCLUDED {label}: {cause}");
            totals.excluded += 1;
            continue;
        }
        totals.compare(&label, sample);
    }
    totals.report("parser test samples");
    println!("{} samples excluded for a documented divergence", totals.excluded);
    assert!(samples.len() > 100, "the extractor found {} samples", samples.len());
    assert!(totals.mismatches.is_empty());
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn colour_operators_with_four_operands_equal_parse_content_stream() {
    assert_parity("RG", b"1 0 0 RG 0.5 g 0 0 1 rg 1 0 0 0 k");
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn an_integer_pair_before_r_is_two_integers_and_the_operator_r() {
    let ops = operators_of(b"1 0 R Tj", 3, 16);
    assert_eq!(
        ops,
        vec![
            Operator::Other {
                name: "R".into(),
                operands: Box::new(vec![Object::Integer(1), Object::Integer(0)])
            },
            Operator::Tj { text: Vec::new() },
        ]
    );
    // parse_content_stream reads the same bytes as a reference operand.
    assert_eq!(parse_content_stream(b"1 0 R Tj").unwrap(), vec![Operator::Tj { text: Vec::new() }]);
    let ops = operators_of(b"0 0 R", 1, 16);
    assert_eq!(ops.len(), 1);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn true_false_and_null_are_operands() {
    let ops = operators_of(b"true false null foo /IM true BDC", 2, 16);
    assert_eq!(
        ops[0],
        Operator::Other {
            name: "foo".into(),
            operands: Box::new(vec![Object::Boolean(true), Object::Boolean(false), Object::Null])
        }
    );
    assert_eq!(
        ops[1],
        Operator::BeginMarkedContentDict {
            tag: "IM".into(),
            properties: Box::new(Object::Boolean(true))
        }
    );
    // parse_content_stream reads each of the three as an operator named for it.
    let reference = parse_content_stream(b"true false null BT").unwrap();
    assert_eq!(reference.len(), 4);
    assert!(matches!(&reference[0], Operator::Other { name, .. } if name == "true"));
    assert_eq!(operators_of(b"true false null BT", 1, 16), vec![Operator::BeginText]);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn an_operator_keeps_its_33_newest_operands_and_the_window_stays_small() {
    use crate::host::content::operators::MAX_OPERANDS;
    const NUMBERS: u64 = 10_000_000;
    let source = Repeat { line: b"7 ", at: 0, lines: NUMBERS }.chain(&b"zz"[..]);
    let mut ops = Operators::new(source, Mode::Full);
    let op = ops.next().unwrap().unwrap();
    assert_eq!(
        op,
        Operator::Other {
            name: "zz".into(),
            operands: Box::new(vec![Object::Integer(7); MAX_OPERANDS])
        }
    );
    assert_eq!(ops.dropped_operands(), NUMBERS - MAX_OPERANDS as u64);
    assert!(ops.next().is_none());
    assert_eq!(ops.lexer().window_capacity(), crate::host::content::lexer::WINDOW);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_long_array_that_is_not_a_tj_array_is_counted_as_damage() {
    let mut data = b"[".to_vec();
    for i in 0..5000 {
        data.extend_from_slice(format!("{i} ").as_bytes());
    }
    data.extend_from_slice(b"] 0 d");
    let mut ops = Operators::new(&data[..], Mode::Full);
    let out: Vec<Operator> = ops.by_ref().map(|op| op.unwrap()).collect();
    assert!(matches!(out[0], Operator::TJ { .. }), "{}", short(&out[0]));
    assert_eq!(ops.tj_misreads(), 1);

    let tj = tj_stream(10_000);
    let mut ops = Operators::new(&tj[..], Mode::Full);
    assert_eq!(ops.by_ref().count(), 5);
    assert_eq!(ops.tj_misreads(), 0);
}

fn tj_stream(elements: usize) -> Vec<u8> {
    let mut data = b"BT [".to_vec();
    for i in 0..elements {
        if i % 2 == 0 {
            data.extend_from_slice(format!("(w{i})").as_bytes());
        } else {
            data.extend_from_slice(format!(" -{i} ").as_bytes());
        }
    }
    data.extend_from_slice(b"] TJ ET");
    data
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_ten_thousand_element_tj_comes_out_as_three_operators_that_merge_to_the_original() {
    let data = tj_stream(10_000);
    let ops = operators_of(&data, 1024, 64 * 1024);
    let lengths: Vec<usize> = ops
        .iter()
        .filter_map(|op| match op {
            Operator::TJ { array } => Some(array.len()),
            _ => None,
        })
        .collect();
    assert_eq!(lengths, vec![TJ_CHUNK, TJ_CHUNK, 10_000 - 2 * TJ_CHUNK]);
    assert_parity("tj 10000", &data);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_tj_of_exactly_one_chunk_is_one_operator_and_one_more_element_is_two() {
    let one = operators_of(&tj_stream(TJ_CHUNK), 7, 16);
    assert_eq!(one.iter().filter(|op| matches!(op, Operator::TJ { .. })).count(), 1);
    let two = operators_of(&tj_stream(TJ_CHUNK + 1), 7, 16);
    assert_eq!(two.iter().filter(|op| matches!(op, Operator::TJ { .. })).count(), 2);
    assert_parity("tj 4096", &tj_stream(TJ_CHUNK));
    assert_parity("tj 4097", &tj_stream(TJ_CHUNK + 1));
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_string_literal_spanning_many_chunks_is_one_operand() {
    let mut data = b"BT (".to_vec();
    for i in 0..3000 {
        data.extend_from_slice(match i % 5 {
            0 => b"a(b)c",
            1 => b"\\)",
            2 => b"\\\n",
            3 => b"\\101",
            _ => b"plain text",
        });
    }
    data.extend_from_slice(b") Tj ET");
    assert_parity("long string", &data);
    let ops = operators_of(&data, 3, 16);
    assert!(matches!(&ops[1], Operator::Tj { text } if text.len() > 10_000));
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn names_with_hash_escapes_decode_as_parse_object_decodes_them() {
    assert_parity("names", b"/A#20B gs /F#31 12 Tf /Span << /A#20B /C#20D /E#2f 1 >> BDC EMC");
    let ops = operators_of(b"/A#20B gs", 1, 16);
    assert_eq!(ops, vec![Operator::SetExtGState { dict_name: "A B".into() }]);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn nested_arrays_and_dictionaries_in_bdc_equal_parse_content_stream() {
    assert_parity(
        "bdc",
        b"/P << /MCID 0 /K [ 1 2 << /X [ (s) /N <48> ] /Y << /Z -1.5 >> >> ] /T true /F null /B bare >> BDC \
          /OC /MC0 BDC [ [ ] [ [ 1 ] ] ] 0 d << >> DP EMC EMC",
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_comment_at_a_chunk_boundary_is_skipped() {
    let mut data = b"q 1 0 0 1 0 0 %".to_vec();
    data.extend(std::iter::repeat(b'c').take(200));
    data.extend_from_slice(b"\r\ncm\x00Q\x0c%trailing without a newline");
    let expected = vec![
        Operator::SaveState,
        Operator::Cm { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 },
        Operator::RestoreState,
    ];
    for &n in &CHUNKS {
        for &window in &WINDOWS {
            assert_eq!(operators_of(&data, n, window), expected, "chunk {n} window {window}");
        }
    }
    // parse_content_stream reads the comment's word as an operator and loses the operands.
    assert_ne!(parse_content_stream(&data).unwrap(), expected);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_hex_string_split_across_chunks_equals_parse_content_stream() {
    assert_parity("hex", b"<48 65 6C\n6C 6F> Tj <4> Tj <> Tj [ <41> -3 <4 2> ] TJ");
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn an_inline_image_whose_ei_straddles_a_chunk_boundary_ends_at_the_same_byte() {
    for pad in 0..70 {
        let mut data = b"q BI /W 2 /H 2 /BPC 8 /CS /G ID ".to_vec();
        data.extend(std::iter::repeat(b'x').take(pad));
        data.extend_from_slice(b"xEIx\nEI Q BT (Hi) Tj ET");
        assert_parity(&format!("inline pad {pad}"), &data);
    }
    let ops = operators_of(b"q BI /W 2 /H 2 ID AB EI Q", 1, 16);
    assert!(matches!(&ops[1], Operator::InlineImage { data, .. } if data == b"AB"));
    assert_eq!(ops.len(), 3);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn parse_content_stream_leaves_a_stray_i_after_an_inline_image() {
    let ops = parse_content_stream(b"BI /W 1 /H 1 ID A EI Q").unwrap();
    assert!(matches!(ops[0], Operator::InlineImage { .. }));
    assert!(is_stray_i(&ops[1]));
    assert_eq!(ops[2], Operator::RestoreState);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn damage_is_skipped_and_counted_and_a_run_of_it_ends_the_stream() {
    assert_parity("stray bytes", b"q ) ) Q } 1 2 ] 3 4 Td >> BT");
    let mut ops = Operators::new(&b"q ) ) Q"[..], Mode::Full);
    assert_eq!(ops.by_ref().count(), 2);
    assert_eq!(ops.lexer().skipped(), 2);
    assert_eq!(ops.lexer().gave_up_at(), None);

    let mut junk = vec![b')'; 5000];
    junk.extend_from_slice(b" q");
    let mut ops = Operators::new(&junk[..], Mode::Full);
    assert_eq!(ops.by_ref().count(), 0);
    assert_eq!(ops.lexer().gave_up_at(), Some(1023));
    assert!(parse_content_stream(&junk).unwrap().is_empty());
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn damaged_streams_are_skipped_the_way_parse_content_stream_skips_them() {
    const PARTS: [&str; 40] = [
        "q", "Q", "BT", "ET", "1", "0", "-2.5", ".5", "/F1", "/A#20B", "(abc)", "(a(b)c)", "<4142>",
        "<41", "(open", ")", "]", "[", ">>", "<<", "{", "}", "Tj", "TJ", "Td", "cm", "re", "BDC",
        "EMC", "d", "Do", "gs", "BI", "ID", "EI", "[ (a) -3 ]", "<< /K 1 >>", "<zz>", "'", "*",
    ];
    let mut state = 20_241_030u64;
    let mut next = |bound: usize| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((state >> 33) as usize) % bound
    };
    let mut totals = Totals::default();
    for stream in 0..400 {
        let mut data = Vec::new();
        for _ in 0..next(60) + 1 {
            data.extend_from_slice(PARTS[next(PARTS.len())].as_bytes());
            data.extend_from_slice([&b" "[..], &b"\n"[..], &b""[..]][next(3)]);
        }
        if known_divergence(&data).is_none() {
            totals.compare(&format!("random {stream} {:?}", String::from_utf8_lossy(&data)), &data);
        } else {
            totals.excluded += 1;
        }
    }
    totals.report("random damaged streams");
    println!("{} random streams excluded for a documented divergence", totals.excluded);
    assert!(totals.streams > 100);
    assert!(totals.mismatches.is_empty());
}

/// A source of `lines` repetitions of one line, never held.
pub(super) struct Repeat {
    pub(super) line: &'static [u8],
    pub(super) at: usize,
    pub(super) lines: u64,
}

impl Read for Repeat {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < out.len() && self.lines > 0 {
            let take = (self.line.len() - self.at).min(out.len() - written);
            out[written..written + take].copy_from_slice(&self.line[self.at..self.at + take]);
            written += take;
            self.at += take;
            if self.at == self.line.len() {
                self.at = 0;
                self.lines -= 1;
            }
        }
        Ok(written)
    }
}

/// No Dart battery sees it: it needs a 96 MiB content stream and the lexer's window size, which no Dart call exposes.
#[test]
fn a_96_mib_content_stream_is_iterated_in_the_starting_window() {
    const LINE: &[u8] = b"(48656c6c6f) Tj\n";
    let lines = 96 * 1024 * 1024 / LINE.len() as u64;
    let mut ops = Operators::new(Repeat { line: LINE, at: 0, lines }, Mode::Full);
    let mut count = 0u64;
    for op in ops.by_ref() {
        assert!(matches!(op.unwrap(), Operator::Tj { text } if text == b"48656c6c6f"));
        count += 1;
    }
    println!("{count} operators, window {} bytes", ops.lexer().window_capacity());
    assert_eq!(count, lines);
    assert!(ops.lexer().window_capacity() <= 128 * 1024);
    assert_eq!(ops.lexer().skipped(), 0);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_cancelled_check_stops_the_iterator_within_4096_operators() {
    use crate::cancel::CancelCheck;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    let data = b"q\n".repeat(10_000);
    let cancel = CancelCheck::from_flags(
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(true)),
    );
    let mut ops = Operators::new(&data[..], Mode::Full).with_cancel(cancel);
    let mut seen = 0;
    let err = loop {
        match ops.next() {
            Some(Ok(_)) => seen += 1,
            Some(Err(e)) => break e,
            None => panic!("not cancelled"),
        }
    };
    assert!(err.to_string().contains("cancelled"));
    assert!(seen < 4096, "{seen}");
    assert!(ops.next().is_none());
}
