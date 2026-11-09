//! The filters as readers, against the whole-buffer decoders they replaced.
//!
//! `legacy_decoders` holds those decoders as they were. Every case runs the
//! reader chain with its input handed over in pieces of 1 to 65536 bytes
//! and compares with the legacy chain over the whole input.

use super::legacy_decoders as legacy;
use crate::decoders::stream::{chain, read_whole, Pieces, TEST_ALLOCATION_LIMIT};
use crate::decoders::DecodeParams;
use crate::error::{Error, Result};
use std::io::{Read, Write};
use std::sync::atomic::Ordering;

const PIECES: [usize; 7] = [1, 2, 3, 7, 64, 1024, 65536];

pub(super) use crate::host::heap::{PROCESS_LIVE as HELD, PROCESS_PEAK as PEAK};

/// Runs `f` and returns what it returned with the most bytes this thread held
/// above where it began. Unlike `HELD` and `PEAK` it is blind to every other test
/// thread, so a bound on it is tight.
pub(super) fn thread_growth<T>(f: impl FnOnce() -> T) -> (T, usize) {
    crate::host::heap::growth(f)
}

fn run(filters: &[&str], params: Option<&DecodeParams>, input: &[u8], piece: usize) -> Result<Vec<u8>> {
    let names: Vec<String> = filters.iter().map(|f| f.to_string()).collect();
    let source = move || -> Result<Box<dyn Read + '_>> { Ok(Box::new(Pieces { rest: input, piece })) };
    read_whole(chain(&names, params, source)?, 0)
}

/// The committed pipeline: each whole-buffer decoder in turn, then the predictor.
fn legacy_chain(filters: &[&str], params: Option<&DecodeParams>, input: &[u8]) -> Result<Vec<u8>> {
    let mut current = input.to_vec();
    for f in filters {
        current = match *f {
            "FlateDecode" => legacy::flate(&current)?,
            "ASCIIHexDecode" => legacy::ascii_hex(&current)?,
            "ASCII85Decode" => legacy::ascii85(&current)?,
            "LZWDecode" => legacy::lzw(&current, params.map_or(true, |p| p.early_change))?,
            "RunLengthDecode" => legacy::runlength(&current)?,
            _ => current,
        };
    }
    match params {
        Some(p) if p.predictor != 1 => legacy::decode_predictor(&current, p),
        _ => Ok(current),
    }
}

/// The reader chain at every piece size equals the legacy chain (both errors count as equal).
fn assert_equal(what: &str, filters: &[&str], params: Option<&DecodeParams>, input: &[u8]) {
    let expected = legacy_chain(filters, params, input);
    for piece in PIECES {
        let got = run(filters, params, input, piece);
        match (&expected, &got) {
            (Ok(a), Ok(b)) => {
                assert!(a == b, "{what}: pieces of {piece}: legacy {} bytes, reader {} bytes", a.len(), b.len())
            },
            (Err(_), Err(_)) => {},
            _ => panic!("{what}: pieces of {piece}: legacy {expected:?}, reader {got:?}"),
        }
    }
}

// ── samples and encoders ──

fn sample(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let run = (x % 7) as usize;
        let byte = if x & 0x100 == 0 { b'a' + (x % 26) as u8 } else { (x >> 8) as u8 };
        for _ in 0..=run {
            out.push(byte);
        }
    }
    out.truncate(len);
    out
}

fn content_stream(len: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while out.len() < len {
        out.extend_from_slice(format!("BT /F1 12 Tf 1 0 0 1 {} {} Tm (line {i}) Tj ET\n", 10 + i % 90, 700 - i % 600).as_bytes());
        i += 1;
    }
    out.truncate(len);
    out
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn raw_deflate(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for group in data.chunks(4) {
        if group.len() == 4 && group == [0, 0, 0, 0] {
            out.push(b'z');
            continue;
        }
        let mut padded = [0u8; 4];
        padded[..group.len()].copy_from_slice(group);
        let mut v = u32::from_be_bytes(padded);
        let mut digits = [0u8; 5];
        for d in digits.iter_mut().rev() {
            *d = (v % 85) as u8 + b'!';
            v /= 85;
        }
        out.extend_from_slice(&digits[..group.len() + 1]);
    }
    out.extend_from_slice(b"~>");
    out
}

fn ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut out = String::new();
    for (i, b) in data.iter().enumerate() {
        out.push_str(&format!("{b:02X}"));
        if i % 20 == 19 {
            out.push('\n');
        }
    }
    out.push('>');
    out.into_bytes()
}

fn run_length(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let mut run = 1;
        while i + run < data.len() && data[i + run] == data[i] && run < 128 {
            run += 1;
        }
        if run >= 2 {
            out.push((257 - run) as u8);
            out.push(data[i]);
            i += run;
        } else {
            let mut end = i + 1;
            while end < data.len() && end - i < 128 && (end + 1 >= data.len() || data[end] != data[end + 1]) {
                end += 1;
            }
            out.push((end - i - 1) as u8);
            out.extend_from_slice(&data[i..end]);
            i = end;
        }
    }
    out.push(128);
    out
}

fn lzw(data: &[u8], early_change: bool) -> Vec<u8> {
    use weezl::{encode::Encoder, BitOrder};
    let mut enc = if early_change {
        Encoder::with_tiff_size_switch(BitOrder::Msb, 8)
    } else {
        Encoder::new(BitOrder::Msb, 8)
    };
    enc.encode(data).unwrap()
}

fn params(predictor: i64, columns: usize, colors: usize, bpc: usize) -> DecodeParams {
    DecodeParams { predictor, columns, colors, bits_per_component: bpc, early_change: true }
}

/// Rows of `row` bytes whose first byte is a PNG tag cycling through all five.
fn png_rows(rows: usize, row: usize, seed: u64) -> Vec<u8> {
    let noise = sample(rows * row, seed);
    let mut out = Vec::new();
    for r in 0..rows {
        out.push((r % 5) as u8);
        out.extend_from_slice(&noise[r * row..r * row + row - 1]);
    }
    out
}

// ── the proofs ──

/// No Dart battery sees it: it feeds each filter's reader a source that answers in 1 to 65536 bytes a read, which no Dart source does, and compares with the committed whole-buffer decoder.
#[test]
fn each_filter_alone_equals_the_whole_buffer_decoder_at_every_chunk_size() {
    let data = sample(20_000, 7);
    assert_equal("Fl", &["FlateDecode"], None, &zlib(&data));
    assert_equal("A85", &["ASCII85Decode"], None, &ascii85(&data));
    assert_equal("AHx", &["ASCIIHexDecode"], None, &ascii_hex(&data));
    assert_equal("RL", &["RunLengthDecode"], None, &run_length(&data));
    for early in [true, false] {
        let p = DecodeParams { early_change: early, ..Default::default() };
        assert_equal(&format!("LZW early={early}"), &["LZWDecode"], Some(&p), &lzw(&data, early));
    }
    // The wrong switch reads wrong bytes without an error, so the two must differ.
    let early = lzw(&data, true);
    let late = DecodeParams { early_change: false, ..Default::default() };
    assert_ne!(run(&["LZWDecode"], Some(&late), &early, 64).ok(), Some(data.clone()));

    // Inputs the committed decoders answer with an error, or end early.
    assert_equal("A85 z in a group", &["ASCII85Decode"], None, b"!!z!!~>");
    assert_equal("A85 bad char", &["ASCII85Decode"], None, b"!!{!!~>");
    assert_equal("A85 one-digit tail", &["ASCII85Decode"], None, b"!!!!!!~>");
    assert_equal("A85 no end marker", &["ASCII85Decode"], None, b"87cURD]i,\"Ebo80");
    assert_equal("A85 after the marker", &["ASCII85Decode"], None, b"87cURD]i~>junk{{");
    assert_equal("AHx odd digit", &["ASCIIHexDecode"], None, b"48656C6C6>");
    assert_equal("AHx bad digit", &["ASCIIHexDecode"], None, b"48GG>");
    assert_equal("AHx past the marker", &["ASCIIHexDecode"], None, b"4865>6C6C6F");
    assert_equal("RL short literal", &["RunLengthDecode"], None, &[5, 1, 2, 3]);
    assert_equal("RL missing run byte", &["RunLengthDecode"], None, &[1, 1, 2, 250]);
    assert_equal("RL data after EOD", &["RunLengthDecode"], None, &[0, 65, 128, 0, 66]);
    assert_equal("LZW no end code", &["LZWDecode"], None, &lzw(&data, true)[..40]);
    assert_equal("LZW garbage", &["LZWDecode"], None, b"this is not lzw at all");
    assert_equal("LZW empty", &["LZWDecode"], None, b"");
}

/// No Dart battery sees it: it chains filters over sources that answer in 1 to 65536 bytes a read.
#[test]
fn chained_filters_equal_the_whole_buffer_pipeline_at_every_chunk_size() {
    let text = content_stream(30_000);
    // A85 then Flate: the stored data is A85 of a Flate stream.
    assert_equal("A85+Fl", &["ASCII85Decode", "FlateDecode"], None, &ascii85(&zlib(&text)));
    assert_equal("AHx+LZW", &["ASCIIHexDecode", "LZWDecode"], None, &ascii_hex(&lzw(&text, true)));
    assert_equal("RL+Fl", &["RunLengthDecode", "FlateDecode"], None, &run_length(&zlib(&sample(9_000, 3))));
    assert_equal("Fl+Fl", &["FlateDecode", "FlateDecode"], None, &zlib(&zlib(&text)));

    // Flate with a predictor: 12 (Up) and 15 (per-row tags), one-byte and multi-byte pixels.
    for (predictor, colors, bpc, columns) in [(12, 1, 8, 100), (15, 1, 8, 100), (15, 3, 8, 37), (12, 3, 16, 21), (15, 4, 16, 19), (11, 3, 8, 50)] {
        let p = params(predictor, columns, colors, bpc);
        let row = p.bytes_per_row();
        let encoded = png_rows(60, row, 11 + predictor as u64);
        assert_equal(
            &format!("Fl+predictor {predictor} colors {colors} bpc {bpc}"),
            &["FlateDecode"],
            Some(&p),
            &zlib(&encoded),
        );
    }
    // TIFF predictor 2, and a predictor over a data length that is no whole row.
    let tiff = params(2, 40, 3, 8);
    assert_equal("Fl+TIFF predictor", &["FlateDecode"], Some(&tiff), &zlib(&sample(40 * 3 * 25, 5)));
    assert_equal("Fl+TIFF ragged", &["FlateDecode"], Some(&tiff), &zlib(&sample(40 * 3 * 25 + 7, 5)));
    let png = params(12, 10, 1, 8);
    assert_equal("Fl+PNG ragged", &["FlateDecode"], Some(&png), &zlib(&png_rows(9, 11, 2)[..95]));
    assert_equal("Fl+PNG bad tag", &["FlateDecode"], Some(&png), &zlib(&[9u8; 11 * 4]));
    assert_equal("no filter + predictor 3", &[], Some(&params(3, 10, 1, 8)), &[0u8; 20]);
}

/// No Dart battery sees it: an image codec's bytes are only observable as a decoded image, and what is proved here is the bytes that reach the codec.
#[test]
fn an_image_codec_in_the_chain_yields_the_bytes_as_encoded() {
    let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4, 5, 0xFF, 0xD9];
    for codec in ["DCTDecode", "JPXDecode", "JBIG2Decode", "CCITTFaxDecode"] {
        assert_equal(codec, &[codec], None, &jpeg);
        assert_equal(&format!("Fl+{codec}"), &["FlateDecode", codec], None, &zlib(&jpeg));
        assert_equal(&format!("A85+Fl+{codec}"), &["ASCII85Decode", "FlateDecode", codec], None, &ascii85(&zlib(&jpeg)));
        assert_eq!(run(&["FlateDecode", codec], None, &zlib(&jpeg), 3).unwrap(), jpeg);
    }
}

fn corrupt(mut data: Vec<u8>, at: usize, with: u8) -> Vec<u8> {
    data[at] ^= with;
    data
}

/// The damaged-stream cases the whole-buffer decoder recovered from or refused.
fn damaged_cases() -> Vec<(&'static str, Vec<u8>)> {
    let text = content_stream(6_000);
    let binary = sample(4_000, 21);
    let z_text = zlib(&text);
    let z_binary = zlib(&binary);
    let mut cases: Vec<(&'static str, Vec<u8>)> = vec![
        ("empty input", Vec::new()),
        ("one byte", vec![0x78]),
        ("two bytes", vec![0x78, 0x9C]),
        ("truncated text stream", z_text[..z_text.len() / 2].to_vec()),
        ("truncated binary stream", z_binary[..z_binary.len() * 2 / 3].to_vec()),
        ("truncated to the header", z_text[..2].to_vec()),
        ("raw deflate, no zlib header", raw_deflate(&text)),
        ("zlib header with a bad method nibble", {
            let mut v = z_text.clone();
            v[0] = 0x79;
            v
        }),
        ("zlib header with a bad check bits", corrupt(z_text.clone(), 1, 0x1F)),
        ("two junk bytes before raw deflate", [vec![0x00, 0x00], raw_deflate(&text)].concat()),
        ("garbage total", sample(500, 99)),
        ("garbage that looks like a short stream", vec![0xDE, 0xAD, 0xBE, 0xEF, 1, 2, 3, 4, 5, 6]),
        ("corrupt in the middle of text", corrupt(z_text.clone(), z_text.len() / 2, 0x55)),
        ("corrupt late in text", corrupt(z_text.clone(), z_text.len() - 9, 0xFF)),
        ("corrupt in the middle of binary", corrupt(z_binary.clone(), z_binary.len() / 2, 0x55)),
        ("corrupt checksum", corrupt(z_text.clone(), z_text.len() - 1, 0x01)),
        ("trailing garbage after the stream", [z_text.clone(), vec![0xAB; 40]].concat()),
        ("a stored-block-only stream", {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::none());
            e.write_all(&text).unwrap();
            e.finish().unwrap()
        }),
    ];
    // The deflate data preceded by 1 to 19 junk bytes: the offset scan.
    for junk in [1usize, 3, 4, 7, 12, 19] {
        let name: &'static str = Box::leak(format!("raw deflate after {junk} junk bytes").into_boxed_str());
        cases.push((name, [vec![0x11; junk], raw_deflate(&text)].concat()));
    }
    cases
}

/// No Dart battery sees it: each case is a damaged compressed stream whose recovery is observable only as the bytes the decoder returns, compared here with the committed whole-buffer decoder at every chunk size.
///
/// Two cases are compared by prefix instead of by equality. When data goes
/// wrong in the middle of a stream, the committed decoder returned only the
/// bytes of the `read` calls that completed (flate2 drops what the failing
/// call decoded), so its prefix depends on `read_to_end`'s buffer sizes. The
/// reader keeps every byte decoded before the damage is detected: its output
/// starts with the committed one (which is true content), and for a corrupt
/// checksum equals the true decoded content whole.
#[test]
fn every_damaged_flate_stream_recovers_as_the_whole_buffer_decoder_did() {
    let text = content_stream(6_000);
    let mut differ = Vec::new();
    for (name, input) in damaged_cases() {
        let expected = legacy_chain(&["FlateDecode"], None, &input);
        for piece in PIECES {
            let got = run(&["FlateDecode"], None, &input, piece);
            let same = match (&expected, &got) {
                (Ok(a), Ok(b)) if name == "corrupt in the middle of text" => {
                    b.starts_with(a) && b.len() >= a.len() && text.starts_with(a)
                },
                (Ok(a), Ok(b)) if name == "corrupt checksum" => b.starts_with(a) && *b == text,
                (Ok(a), Ok(b)) => a == b,
                (Err(_), Err(_)) => true,
                _ => false,
            };
            if !same {
                differ.push(format!(
                    "{name} (pieces of {piece}): legacy {}, reader {}",
                    summary(&expected),
                    summary(&got)
                ));
                break;
            }
        }
    }
    assert!(differ.is_empty(), "differences:\n{}", differ.join("\n"));
}

fn summary(r: &Result<Vec<u8>>) -> String {
    match r {
        Ok(v) => format!("Ok({} bytes)", v.len()),
        Err(e) => format!("Err({})", e.to_string().chars().take(60).collect::<String>()),
    }
}

/// The offset of the NLEN field of stored block `index` of a zlib stream of stored blocks.
fn stored_block_nlen(zlib: &[u8], index: usize) -> usize {
    let mut at = 2;
    for _ in 0..index {
        let len = u16::from_le_bytes([zlib[at + 1], zlib[at + 2]]) as usize;
        at += 5 + len;
    }
    at + 3
}

/// No Dart battery sees it: the stream decodes cleanly past the probe window and is damaged after it, and what differs is a decoder's recovery policy, observable only as bytes.
#[test]
fn damage_after_the_probe_window_is_judged_as_the_first_strategy_judges_it() {
    // A flipped payload byte in binary data: deflate sees nothing, only the
    // Adler-32 fails. The committed decoder reads such a stream whole (its
    // strategy 3 skips the checksum); the reader ends cleanly with every byte.
    let high_bit: Vec<u8> =
        (0..400_000u32).map(|i| 0x80 | ((i.wrapping_mul(2654435761) >> 13) as u8 & 0x7F)).collect();
    for (what, binary) in [("random bytes", sample(400_000, 77)), ("high-bit bytes", high_bit.clone())] {
        let clean = zlib(&binary);
        let bad = corrupt(clean.clone(), clean.len() * 2 / 3, 0xA5);
        let committed = legacy_chain(&["FlateDecode"], None, &bad);
        let full = committed.as_ref().unwrap_or_else(|e| panic!("{what}: committed answer {e}"));
        assert!(full.len() > 64 * 1024, "{what}: the output outgrows the window");
        for piece in PIECES {
            assert_eq!(&run(&["FlateDecode"], None, &bad, piece).unwrap(), full, "{what}: pieces of {piece}");
        }
    }

    // Damage deflate itself sees, after the window, on binary data: a stored
    // block's NLEN no longer matches its LEN. Both are a typed error.
    let mut stored = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::none());
    stored.write_all(&high_bit).unwrap();
    let stored = stored.finish().unwrap();
    let bad_stored = corrupt(stored.clone(), stored_block_nlen(&stored, 3), 0x5A);
    let committed = legacy_chain(&["FlateDecode"], None, &bad_stored);
    eprintln!("stored-block damage: committed {}", summary(&committed));
    assert!(committed.is_err(), "the committed decoder's answer: {}", summary(&committed));
    for piece in PIECES {
        let got = run(&["FlateDecode"], None, &bad_stored, piece);
        assert!(matches!(got, Err(Error::Decode(_))), "pieces of {piece}: {}", summary(&got));
    }

    // Plausible prefix (content stream), clean for over 64 KiB, then corrupt: the prefix is kept.
    let text = content_stream(300_000);
    let z = zlib(&text);
    let bad_text = corrupt(z.clone(), z.len() * 2 / 3, 0xA5);
    let legacy_text = legacy_chain(&["FlateDecode"], None, &bad_text);
    let reader_text = run(&["FlateDecode"], None, &bad_text, 65536);
    eprintln!("plausible prefix: legacy {}, reader {}", summary(&legacy_text), summary(&reader_text));
    let (Ok(a), Ok(b)) = (&legacy_text, &reader_text) else { panic!("both keep a prefix") };
    assert!(b.starts_with(a) || a.starts_with(b), "one prefix extends the other");
    for piece in PIECES {
        assert_eq!(run(&["FlateDecode"], None, &bad_text, piece).unwrap(), *b, "pieces of {piece}");
    }
}

/// A source that makes `total` bytes of content-stream text, Flate-compressed
/// and ASCII85-encoded as it is read, and never holds more than a window.
struct Generator {
    text_left: usize,
    encoder: Option<flate2::write::ZlibEncoder<Vec<u8>>>,
    pending: Vec<u8>,
    at: usize,
    tail: Vec<u8>,
    done: bool,
    counter: u64,
}

impl Generator {
    fn new(total: usize) -> Self {
        Self {
            text_left: total,
            encoder: Some(flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast())),
            pending: Vec::new(),
            at: 0,
            tail: Vec::new(),
            done: false,
            counter: 0,
        }
    }
}

impl Read for Generator {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.at == self.pending.len() {
            if self.done {
                return Ok(0);
            }
            self.pending.clear();
            self.at = 0;
            let compressed;
            if self.text_left > 0 {
                let n = self.text_left.min(64 * 1024);
                let mut block = Vec::with_capacity(n);
                while block.len() < n {
                    block.extend_from_slice(
                        format!("BT /F1 12 Tf 1 0 0 1 {} {} Tm (row {}) Tj ET\n", self.counter % 90, self.counter % 700, self.counter)
                            .as_bytes(),
                    );
                    self.counter += 1;
                }
                block.truncate(n);
                self.text_left -= n;
                let enc = self.encoder.as_mut().unwrap();
                enc.write_all(&block)?;
                enc.flush()?;
                compressed = std::mem::take(enc.get_mut());
            } else {
                compressed = self.encoder.take().unwrap().finish()?;
            }
            self.tail.extend_from_slice(&compressed);
            let whole = self.tail.len() / 4 * 4;
            let body: Vec<u8> = self.tail.drain(..whole).collect();
            self.pending = ascii85(&body);
            self.pending.truncate(self.pending.len() - 2); // the `~>` goes at the very end
            if self.text_left == 0 && self.encoder.is_none() {
                let rest = std::mem::take(&mut self.tail);
                self.pending.extend_from_slice(&ascii85(&rest));
                self.done = true;
            }
        }
        let n = out.len().min(self.pending.len() - self.at);
        out[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// No Dart battery sees it: the memory a read holds is the engine's own allocator counter, and Dart sees only the bytes that came out.
#[test]
fn a_64_mib_stream_through_ascii85_and_flate_is_never_held() {
    const TOTAL: usize = 64 * 1024 * 1024;
    let names = vec!["ASCII85Decode".to_string(), "FlateDecode".to_string()];
    let source = || -> Result<Box<dyn Read>> { Ok(Box::new(Generator::new(TOTAL))) };
    let mut reader = chain(&names, None, source).unwrap();
    let mut buf = vec![0u8; 64 * 1024];
    // One window in, so the decoder's tables and buffers exist before the baseline.
    let mut counted = reader.read(&mut buf).unwrap();
    let base = HELD.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    loop {
        let n = reader.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        counted += n;
    }
    let growth = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    assert_eq!(counted, TOTAL);
    assert!(growth < 1024 * 1024, "reading 64 MiB grew the process by {growth} bytes");
    eprintln!("64 MiB through A85+Fl: peak growth {growth} bytes");
}

/// A zlib stream of `gib` GiB of zeros, built by repeating one synced block of
/// 1 MiB of zeros: the deflate blocks of zeros are position independent.
fn zeros_bomb(gib: usize) -> Vec<u8> {
    use flate2::{Compress, Compression, FlushCompress};
    let mib = vec![0u8; 1 << 20];
    let mut c = Compress::new(Compression::best(), false);
    let mut block = vec![0u8; 8192];
    let produced = {
        let before = c.total_out();
        c.compress(&mib, &mut block, FlushCompress::Full).unwrap();
        (c.total_out() - before) as usize
    };
    block.truncate(produced);
    let mut raw = Vec::new();
    for _ in 0..gib * 1024 {
        raw.extend_from_slice(&block);
    }
    raw.extend_from_slice(&[0x03, 0x00]); // a final empty fixed block
    let n = (gib as u64) << 30;
    let adler = (((n % 65521) as u32) << 16) | 1;
    [vec![0x78, 0x9C], raw, adler.to_be_bytes().to_vec()].concat()
}

/// No Dart battery sees it: it refuses the allocator on purpose, a hook that exists only in the engine's test build.
#[test]
fn a_flate_bomb_read_whole_is_a_typed_error_and_never_an_abort() {
    let inner = zeros_bomb(8);
    let outer = zlib(&inner);
    eprintln!("bomb: outer stream {} bytes, inner {} bytes, decoded 8 GiB", outer.len(), inner.len());
    // The inner stream alone decodes to 8 GiB of zeros; read counting, not holding, its first 256 MiB.
    let names = vec!["FlateDecode".to_string()];
    let source = || -> Result<Box<dyn Read + '_>> { Ok(Box::new(&inner[..])) };
    let mut r = chain(&names, None, source).unwrap().take(256 * 1024 * 1024);
    assert_eq!(std::io::copy(&mut r, &mut std::io::sink()).unwrap(), 256 * 1024 * 1024);

    // Read whole through the two-stage chain with the allocator refusing past 64 MiB.
    TEST_ALLOCATION_LIMIT.with(|l| l.set(Some(64 * 1024 * 1024)));
    let refused = run(&["FlateDecode", "FlateDecode"], None, &outer, 65536);
    TEST_ALLOCATION_LIMIT.with(|l| l.set(None));
    match refused {
        Err(Error::Decode(msg)) => assert!(msg.contains("could not allocate"), "{msg}"),
        other => panic!("expected a typed decode error, got {}", summary(&other)),
    }
}
