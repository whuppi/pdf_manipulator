//! Inline images, damage and the name tap of the streaming operator source.
//!
//! Every inline image fixture runs at chunk sizes 1, 2, 3, 7 and 64, with a
//! window of 16 bytes and one of 64 KiB, so the end of the data and its
//! lookahead cross a refill at some alignment.

use super::content_stream::{Repeat, Trickle, WINDOWS};
use crate::content::Operator;
use crate::host::content::inline::{EndRule, ImageSpec};
use crate::host::content::lexer::WINDOW;
use crate::host::content::{
    consumed_names, Damage, InlineImages, Lexer, Mode, Operators, Resolver, TokenKind,
};
use crate::object::Object;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use weezl::{decode::Decoder, encode::Encoder, BitOrder, LzwStatus};

const CHUNKS: [usize; 5] = [1, 2, 3, 7, 64];

fn resolver() -> Resolver {
    Box::new(|name| match name {
        "Cs1" => Some(1),
        "Cs3" => Some(3),
        _ => None,
    })
}

fn run_reader<R: Read>(reader: R, window: usize, images: InlineImages) -> (Vec<Operator>, Damage) {
    let lexer = Lexer::with_window(reader, window);
    let mut ops = Operators::from_lexer(lexer, Mode::Full)
        .with_inline_images(images)
        .with_resolver(resolver());
    let out: Vec<Operator> = ops.by_ref().map(|op| op.unwrap()).collect();
    (out, ops.damage())
}

fn run(data: &[u8], n: usize, window: usize, images: InlineImages) -> (Vec<Operator>, Damage) {
    run_reader(Trickle { data, n }, window, images)
}

fn int(n: i64) -> Object {
    Object::Integer(n)
}

fn name(s: &str) -> Object {
    Object::Name(s.into())
}

fn image(entries: &[(&str, Object)], data: &[u8]) -> Operator {
    let dict: HashMap<String, Object> =
        entries.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    Operator::InlineImage { dict: Box::new(dict), data: data.to_vec() }
}

/// `q BI <dict> ID <one space><data><tail> ...`; the tail starts the real end.
fn stream(dict: &str, data: &[u8], tail: &str) -> Vec<u8> {
    let mut out = format!("q BI {dict} ID ").into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(tail.as_bytes());
    out
}

/// The stream yields `q`, the image and `Q`, cleanly, at every chunk size and window.
fn assert_image(data: &[u8], expected: Operator) {
    let want = vec![Operator::SaveState, expected, Operator::RestoreState];
    for &n in &CHUNKS {
        for &window in &WINDOWS {
            let (got, damage) = run(data, n, window, InlineImages::Collect);
            assert_eq!(got, want, "chunk {n} window {window}");
            assert!(damage.is_clean(), "chunk {n} window {window}: {damage:?}");
        }
    }
}

const TAIL: &str = "\nEI Q";

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_correct_length_ends_the_data_at_that_byte_though_the_data_holds_ei() {
    let data = b"ab EI cd";
    let dict = "/W 1 /H 1 /BPC 8 /CS /G /L 8";
    assert_image(
        &stream(dict, data, TAIL),
        image(
            &[("W", int(1)), ("H", int(1)), ("BPC", int(8)), ("CS", name("G")), ("L", int(8))],
            data,
        ),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_length_that_lies_short_or_long_falls_through_and_the_data_still_ends_right() {
    let data = b"ab EI c";
    for length in [1, 5, 9, 50, 4000] {
        let dict = format!("/W 7 /H 1 /BPC 8 /CS /G /L {length}");
        let mut padded = stream(&dict, data, TAIL);
        padded.extend_from_slice(&[b' '; 200]);
        let want = vec![
            Operator::SaveState,
            image(
                &[
                    ("W", int(7)),
                    ("H", int(1)),
                    ("BPC", int(8)),
                    ("CS", name("G")),
                    ("L", int(length)),
                ],
                data,
            ),
            Operator::RestoreState,
        ];
        for &n in &CHUNKS {
            for &window in &WINDOWS {
                let (got, damage) = run(&padded, n, window, InlineImages::Collect);
                assert_eq!(got, want, "L {length} chunk {n} window {window}");
                assert!(damage.is_clean());
            }
        }
    }
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn the_computed_length_ends_data_that_holds_a_line_with_ei() {
    let data = b"ab\nEI\ncd";
    assert_image(
        &stream("/W 8 /H 1 /BPC 8 /CS /G", data, TAIL),
        image(&[("W", int(8)), ("H", int(1)), ("BPC", int(8)), ("CS", name("G"))], data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn an_image_mask_is_one_bit_and_one_component() {
    let data = b" EI ";
    assert_image(
        &stream("/W 16 /H 2 /IM true", data, TAIL),
        image(&[("W", int(16)), ("H", int(2)), ("IM", Object::Boolean(true))], data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn indexed_and_resolved_colour_spaces_give_the_component_count() {
    let data = b"ab\nEI\ncd";
    assert_image(
        &stream("/W 8 /H 1 /BPC 8 /CS /I", data, TAIL),
        image(&[("W", int(8)), ("H", int(1)), ("BPC", int(8)), ("CS", name("I"))], data),
    );
    assert_image(
        &stream("/W 8 /H 1 /BPC 8 /CS /Cs1", data, TAIL),
        image(&[("W", int(8)), ("H", int(1)), ("BPC", int(8)), ("CS", name("Cs1"))], data),
    );
    // Three components through the resolver: 2 * 3 * 1 = 6 bytes.
    let data = b"a EI c";
    assert_image(
        &stream("/W 2 /H 1 /BPC 8 /CS /Cs3", data, TAIL),
        image(&[("W", int(2)), ("H", int(1)), ("BPC", int(8)), ("CS", name("Cs3"))], data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn array_colour_spaces_give_the_component_count() {
    // DeviceN with three names: 2 * 3 * 1 = 6 bytes.
    let data = b"a EI c";
    let cs = Object::Array(vec![
        name("DeviceN"),
        Object::Array(vec![name("A"), name("B"), name("C")]),
    ]);
    assert_image(
        &stream("/W 2 /H 1 /BPC 8 /CS [/DeviceN [/A /B /C]]", data, TAIL),
        image(&[("W", int(2)), ("H", int(1)), ("BPC", int(8)), ("CS", cs)], data),
    );
    // ICCBased with /N 4: 1 * 4 * 1 = 4 bytes.
    let data = b" EI ";
    let cs = Object::Array(vec![
        name("ICCBased"),
        Object::Dictionary([("N".to_string(), int(4))].into_iter().collect()),
    ]);
    assert_image(
        &stream("/W 1 /H 1 /BPC 8 /CS [/ICCBased << /N 4 >>]", data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("BPC", int(8)), ("CS", cs)], data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn both_key_spellings_mean_the_same() {
    let data = b"ab\nEI\ncd";
    assert_image(
        &stream(
            "/Width 8 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray",
            data,
            TAIL,
        ),
        image(
            &[
                ("Width", int(8)),
                ("Height", int(1)),
                ("BitsPerComponent", int(8)),
                ("ColorSpace", name("DeviceGray")),
            ],
            data,
        ),
    );
    assert_image(
        &stream("/W 8 /H 1 /BPC 8 /CS /G /Length 8", data, TAIL),
        image(
            &[
                ("W", int(8)),
                ("H", int(1)),
                ("BPC", int(8)),
                ("CS", name("G")),
                ("Length", int(8)),
            ],
            data,
        ),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn ascii_hex_ends_at_the_greater_than_sign() {
    let data = b"48 65 6C EI 6C 6F>";
    assert_image(
        &stream("/W 1 /H 1 /F /AHx", data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("F", name("AHx"))], data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn ascii85_ends_at_tilde_greater_than() {
    let data = b"87cU EI RD]i,\"Ebo80~>";
    assert_image(
        &stream("/W 1 /H 1 /F [/A85]", data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("F", Object::Array(vec![name("A85")]))], data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn run_length_ends_at_the_byte_128_after_a_literal_that_holds_ei() {
    let mut data = vec![43u8, b'\n', b'E', b'I', b' '];
    data.extend(std::iter::repeat(b'x').take(40));
    data.extend_from_slice(&[0xFD, b'y', 128]);
    assert_image(
        &stream("/W 1 /H 1 /F /RL", &data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("F", name("RL"))], &data),
    );
}

fn stored_zlib(data: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::none());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn flate_ends_where_the_zlib_stream_ends_though_it_holds_ei() {
    let mut text = b"abc\nEI ".to_vec();
    text.extend(std::iter::repeat(b'x').take(40));
    let data = stored_zlib(&text);
    assert_image(
        &stream("/W 1 /H 1 /F /Fl", &data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("F", name("Fl"))], &data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_compressed_flate_stream_ends_at_the_end_of_the_stream() {
    let text: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&text).unwrap();
    let data = encoder.finish().unwrap();
    assert_image(
        &stream("/W 1 /H 1 /F /Fl", &data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("F", name("Fl"))], &data),
    );
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn lzw_has_no_end_rule_and_ends_by_the_fallback() {
    let mut text = b"abc EI ".to_vec();
    text.extend((0..200u32).map(|i| (i % 13) as u8));
    let data = Encoder::with_tiff_size_switch(BitOrder::Msb, 8).encode(&text).unwrap();
    assert!(!data.windows(4).any(|w| w == b" EI "));
    assert_image(
        &stream("/W 1 /H 1 /F /LZW", &data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("F", name("LZW"))], &data),
    );
    let spec = ImageSpec::from_dict(
        &[("F".to_string(), name("LZW"))].into_iter().collect(),
        None,
    );
    assert_eq!(spec.rule, EndRule::Fallback);
}

/// No Dart battery sees it: it needs weezl's decoder state at the end code, which no Dart call exposes.
#[test]
fn weezl_reports_the_end_of_an_lzw_stream_exactly_only_when_fed_a_byte_at_a_time() {
    let mut state = 7u32;
    let mut overshoot = 0;
    for len in [0usize, 1, 3, 10, 100, 1000, 5000] {
        let data: Vec<u8> = (0..len)
            .map(|_| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                ((state >> 24) % 7) as u8
            })
            .collect();
        for early in [true, false] {
            let encoded = if early {
                Encoder::with_tiff_size_switch(BitOrder::Msb, 8).encode(&data).unwrap()
            } else {
                Encoder::new(BitOrder::Msb, 8).encode(&data).unwrap()
            };
            let mut stream = encoded.clone();
            stream.extend_from_slice(b"\nEI Q BT (Hi) Tj ET");
            for chunk in [1usize, 7, 100_000] {
                let mut decoder = if early {
                    Decoder::with_tiff_size_switch(BitOrder::Msb, 8)
                } else {
                    Decoder::new(BitOrder::Msb, 8)
                };
                let mut out = vec![0u8; 8192];
                let mut fed = 0;
                let mut done_at = None;
                'feed: while fed < stream.len() {
                    let end = (fed + chunk).min(stream.len());
                    let mut slice = &stream[fed..end];
                    loop {
                        let r = decoder.decode_bytes(slice, &mut out);
                        fed += r.consumed_in;
                        slice = &slice[r.consumed_in..];
                        if let Ok(LzwStatus::Done) = r.status {
                            done_at = Some(fed);
                            break 'feed;
                        }
                        if r.status.is_err() {
                            break 'feed;
                        }
                        if slice.is_empty() || (r.consumed_in == 0 && r.consumed_out == 0) {
                            break;
                        }
                    }
                }
                let done_at = done_at.expect("the end code was reached");
                if chunk == 1 {
                    assert_eq!(done_at, encoded.len(), "len {len} early {early}");
                } else if done_at != encoded.len() {
                    assert!(done_at > encoded.len());
                    overshoot += 1;
                }
            }
        }
    }
    assert!(overshoot > 0, "consumed_in was exact in every chunked run");
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_dct_image_ends_by_the_fallback_even_with_ei_and_binary_inside() {
    let data = [0xFF, 0xD8, b' ', b'E', b'I', b' ', 0x80, 0x81, 0xFF, 0xD9];
    assert_image(
        &stream("/W 1 /H 1 /F /DCT", &data, TAIL),
        image(&[("W", int(1)), ("H", int(1)), ("F", name("DCT"))], &data),
    );
    // A chain that ends in a codec has no end rule of its own.
    let spec = ImageSpec::from_dict(
        &[("F".to_string(), Object::Array(vec![name("A85"), name("DCT")]))]
            .into_iter()
            .collect(),
        None,
    );
    assert_eq!(spec.rule, EndRule::Fallback);
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_binary_string_after_the_real_ei_does_not_hide_it() {
    let data = [0xFF, 0xD8, 0x80, 0xFF, 0xD9];
    let mut bytes = stream("/W 1 /H 1 /F /DCT", &data, "\nEI (");
    bytes.extend_from_slice(&[0x80, 0x81]);
    bytes.extend_from_slice(b")Tj");
    let want = vec![
        Operator::SaveState,
        image(&[("W", int(1)), ("H", int(1)), ("F", name("DCT"))], &data),
        Operator::Tj { text: vec![0x80, 0x81] },
    ];
    for &n in &CHUNKS {
        for &window in &WINDOWS {
            let (got, damage) = run(&bytes, n, window, InlineImages::Collect);
            assert_eq!(got, want, "chunk {n} window {window}");
            assert!(damage.is_clean());
        }
    }
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn an_image_that_never_ends_sets_inline_unended_and_ends_the_stream() {
    for data in [&b"q BI /W 2 /H 2 ID AB"[..], &b"q BI /W 2 /H 2 /F /Fl ID AB EIX"[..]] {
        for &n in &CHUNKS {
            for &window in &WINDOWS {
                let (got, damage) = run(data, n, window, InlineImages::Collect);
                assert_eq!(got, vec![Operator::SaveState], "chunk {n} window {window}");
                assert!(damage.inline_unended, "chunk {n} window {window}");
            }
        }
    }
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn skip_yields_the_image_without_its_data() {
    let data = b"ab\nEI\ncd";
    let bytes = stream("/W 8 /H 1 /BPC 8 /CS /G", data, TAIL);
    for &n in &CHUNKS {
        let (got, damage) = run(&bytes, n, 16, InlineImages::Skip);
        assert_eq!(
            got[1],
            image(&[("W", int(8)), ("H", int(1)), ("BPC", int(8)), ("CS", name("G"))], b"")
        );
        assert_eq!(got.len(), 3);
        assert!(damage.is_clean());
    }
}

/// No Dart battery sees it: it needs a 32 MiB inline image and the lexer's window size, which no Dart call exposes.
#[test]
fn a_32_mib_inline_image_streams_past_in_the_starting_window() {
    const SIDE: u64 = 4096;
    const BYTES: u64 = SIDE * 8192;
    let head = format!("q BI /W {SIDE} /H 8192 /BPC 8 /CS /G ID ").into_bytes();
    let source = io::Cursor::new(head)
        .chain(Repeat { line: b"a", at: 0, lines: BYTES })
        .chain(&b"\nEI Q"[..]);
    let lexer = Lexer::new(source);
    let mut ops = Operators::from_lexer(lexer, Mode::Full).with_inline_images(InlineImages::Skip);
    let got: Vec<Operator> = ops.by_ref().map(|op| op.unwrap()).collect();
    assert_eq!(got.len(), 3);
    assert!(matches!(&got[1], Operator::InlineImage { data, .. } if data.is_empty()));
    assert_eq!(ops.lexer().window_capacity(), WINDOW);
    assert!(ops.damage().is_clean());
}

/// A source that fails once `ok` bytes were read.
struct FailAfter<'a> {
    data: &'a [u8],
    ok: usize,
    read: usize,
}

impl Read for FailAfter<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.read >= self.ok {
            return Err(io::Error::other("disk gone"));
        }
        let take = out.len().min(self.ok - self.read).min(self.data.len() - self.read);
        out[..take].copy_from_slice(&self.data[self.read..self.read + take]);
        self.read += take;
        Ok(take)
    }
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn a_read_error_ends_the_iterator_and_is_recorded() {
    let data = b"q 1 0 0 1 0 0 cm Q q Q q Q";
    let (got, damage) = run_reader(FailAfter { data, ok: 20, read: 0 }, 16, InlineImages::Collect);
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(damage.read_error.as_deref(), Some("disk gone"));
    assert!(!damage.is_clean());

    let img = b"q BI /W 8 /H 1 /BPC 8 /CS /G ID abcdefgh\nEI Q";
    let (got, damage) = run_reader(FailAfter { data: img, ok: 30, read: 0 }, 16, InlineImages::Collect);
    assert_eq!(got, vec![Operator::SaveState]);
    assert_eq!(damage.read_error.as_deref(), Some("disk gone"));
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn each_damage_counter_has_a_stream_that_sets_it() {
    let (_, clean) = run(b"q 1 0 0 1 0 0 cm Q", 3, 16, InlineImages::Collect);
    assert!(clean.is_clean());

    let (_, d) = run(b"q ) Q", 3, 16, InlineImages::Collect);
    assert_eq!(d.skipped, 1);
    assert_eq!(d, Damage { skipped: 1, ..Damage::default() });

    let mut many = "7 ".repeat(40).into_bytes();
    many.extend_from_slice(b"zz");
    let (_, d) = run(&many, 3, 16, InlineImages::Collect);
    assert_eq!(d, Damage { dropped_operands: 7, ..Damage::default() });

    let mut array = b"[".to_vec();
    for i in 0..5000 {
        array.extend_from_slice(format!("{i} ").as_bytes());
    }
    array.extend_from_slice(b"] 0 d");
    let (_, d) = run(&array, 64, 64 * 1024, InlineImages::Collect);
    assert_eq!(d, Damage { tj_misreads: 1, ..Damage::default() });

    let junk = vec![b')'; 3000];
    let (got, d) = run(&junk, 7, 16, InlineImages::Collect);
    assert!(got.is_empty());
    assert_eq!(d, Damage { skipped: 1024, gave_up_at: Some(1023), ..Damage::default() });

    let (_, d) = run(b"q (abc", 3, 16, InlineImages::Collect);
    assert_eq!(d.unterminated, Some(TokenKind::LiteralString));
    let (_, d) = run(b"q <41", 3, 16, InlineImages::Collect);
    assert_eq!(d.unterminated, Some(TokenKind::HexString));

    let (_, d) = run(b"q BI /W 2 ID AB", 3, 16, InlineImages::Collect);
    assert_eq!(d, Damage { inline_unended: true, ..Damage::default() });
}

/// Operators and what the name tap says of each, over one stream.
fn names_of(data: &[u8]) -> Vec<(Operator, Vec<String>, Vec<String>)> {
    let mut ops = Operators::new(data, Mode::Full);
    let mut out = Vec::new();
    while let Some(op) = ops.next() {
        let op = op.unwrap();
        let pending = ops.pending_names().to_vec();
        let used = consumed_names(&op).iter().map(|s| s.to_string()).collect();
        out.push((op, pending, used));
    }
    out
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn the_name_tap_reports_each_operator_arm_by_the_table() {
    let cases: [(&[u8], &[&str], &[&str]); 18] = [
        (b"/F1 12 Tf", &["F1"], &["F1"]),
        (b"/Im0 Do", &["Im0"], &["Im0"]),
        (b"/Cs1 cs", &["Cs1"], &["Cs1"]),
        (b"/Cs2 CS", &["Cs2"], &["Cs2"]),
        (b"/GS0 gs", &["GS0"], &["GS0"]),
        (b"/Sh0 sh", &["Sh0"], &["Sh0"]),
        (b"0.5 0.2 /P1 scn", &["P1"], &["P1"]),
        (b"0.5 0.2 /P2 SCN", &["P2"], &["P2"]),
        (b"0.5 0.2 scn", &[], &[]),
        (b"/Tag /Props BDC", &["Tag", "Props"], &["Props"]),
        (b"/Tag << /K /V >> BDC", &["Tag", "K", "V"], &[]),
        (b"/Tag BMC", &["Tag"], &[]),
        (b"/Tag MP", &["Tag"], &[]),
        (b"/Tag /Props DP", &["Tag", "Props"], &["Props"]),
        (b"1 2 d0", &[], &[]),
        (b"/N 3 foo", &["N"], &[]),
        (b"BI /W 1 /H 1 /CS /Cs1 ID x\nEI", &["W", "H", "CS", "Cs1"], &["Cs1"]),
        (b"BI /W 1 /H 1 /CS [/I /RGB 255 <00>] ID x\nEI", &["W", "H", "CS", "I", "RGB"], &["I", "RGB"]),
    ];
    for (data, pending, used) in cases {
        let got = names_of(data);
        assert_eq!(got.len(), 1, "{}", String::from_utf8_lossy(data));
        let (_, got_pending, got_used) = &got[0];
        assert_eq!(got_pending, &strings(pending), "pending of {}", String::from_utf8_lossy(data));
        assert_eq!(got_used, &strings(used), "consumed by {}", String::from_utf8_lossy(data));
    }
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a content stream.
#[test]
fn the_name_tap_starts_again_at_each_operator_and_forgets_dropped_attempts() {
    let got = names_of(b"/A gs /B /C foo /D Do");
    let pending: Vec<_> = got.iter().map(|(_, p, _)| p.clone()).collect();
    assert_eq!(pending, vec![strings(&["A"]), strings(&["B", "C"]), strings(&["D"])]);
    // The `)` damages `/Y [ /X )` and the bytes are read again from the next one, so
    // `Y` and `X` come back as operators and their names are not reported as names.
    let got = names_of(b"/Y [ /X ) Td /Z Do");
    let all: Vec<String> = got.iter().flat_map(|(_, p, _)| p.clone()).collect();
    assert_eq!(all, strings(&["Z"]));
}
