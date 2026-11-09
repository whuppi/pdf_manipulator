//! A full rewrite re-deflates Flate streams and keeps a stream's source bytes
//! unless the new encoding is strictly smaller.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::host::deflate::MEMORY_STREAM;
use crate::object::{Object, ObjectRef};
use flate2::{write::ZlibEncoder, Compression};
use std::io::Write;

fn deflate(data: &[u8], level: u32) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(level));
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// Bytes that do not compress.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

/// Bytes that compress well but not to nothing.
fn text(len: usize) -> Vec<u8> {
    let words = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf"];
    let mut out = Vec::with_capacity(len + 8);
    let mut i = 0usize;
    while out.len() < len {
        out.extend_from_slice(words[(i * 7 + i / 3) % words.len()].as_bytes());
        out.push(b' ');
        i += 1;
    }
    out.truncate(len);
    out
}

/// A document whose catalog reaches stream object 4.
fn source_with_stream(dict: &str, data: &[u8]) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra 4 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
        stream_object(4, dict, data),
    ])
}

fn stream_of(doc: &PdfDocument) -> Object {
    doc.load_object(ObjectRef::new(4, 0)).unwrap()
}

fn saved(source: Vec<u8>) -> PdfDocument {
    let mut editor = DocumentEditor::from_bytes(source).unwrap();
    let bytes = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    PdfDocument::from_bytes(bytes).unwrap()
}

fn parts(object: &Object) -> (&std::collections::HashMap<String, Object>, Vec<u8>) {
    match object {
        Object::Stream { dict, body } => (dict, body.read_whole_raw().unwrap()),
        other => panic!("not a stream: {other:?}"),
    }
}

/// Rows of `columns` bytes behind a PNG Up tag, as a /Predictor 12 stream holds them.
fn predictor_rows(columns: usize, rows: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for row in 0..rows {
        out.push(2);
        out.extend((0..columns).map(|c| ((row + c) % 5) as u8));
    }
    out
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a re-encoded stream from the source's own bytes.
#[test]
fn a_flate_stream_with_predictor_parms_is_redeflated_smaller_and_decodes_the_same() {
    let rows = predictor_rows(64, 400);
    let encoded = deflate(&rows, 1);
    let dict = "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns 64 >>";
    let source = source_with_stream(dict, &encoded);

    let before = PdfDocument::from_bytes(source.clone()).unwrap();
    let before_object = stream_of(&before);
    let (before_dict, _) = parts(&before_object);
    let after = saved(source);
    let after_object = stream_of(&after);
    let (after_dict, after_data) = parts(&after_object);

    assert!(after_data.len() < encoded.len(), "{} bytes, source {}", after_data.len(), encoded.len());
    assert_eq!(after_dict.get("DecodeParms"), before_dict.get("DecodeParms"));
    assert_eq!(after_dict.get("Filter"), before_dict.get("Filter"));
    assert_eq!(
        after_object.decode_stream_data().unwrap(),
        before_object.decode_stream_data().unwrap()
    );
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a copy from a re-encode.
#[test]
fn a_raw_stream_that_does_not_compress_is_copied_raw_without_a_filter() {
    let data = noise(3000);
    let after = saved(source_with_stream("", &data));
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);

    assert!(!dict.contains_key("Filter"), "a filter was added: {dict:?}");
    assert_eq!(bytes, data.as_slice());
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a deflated stream from a raw one.
#[test]
fn a_raw_stream_that_compresses_is_deflated() {
    let data = text(3000);
    let after = saved(source_with_stream("", &data));
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);

    assert_eq!(dict.get("Filter"), Some(&Object::Name("FlateDecode".to_string())));
    assert!(bytes.len() < data.len());
    assert_eq!(object.decode_stream_data().unwrap(), data);
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a copy from a re-encode.
#[test]
fn a_flate_stream_the_new_level_cannot_shrink_keeps_its_source_bytes() {
    let data = text(3000);
    let encoded = deflate(&data, 8);
    let after = saved(source_with_stream("/Filter /FlateDecode", &encoded));
    let object = stream_of(&after);
    let (_, bytes) = parts(&object);

    assert_eq!(bytes, encoded.as_slice());
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a copy from a re-encode.
#[test]
fn a_stream_with_two_filters_keeps_its_bytes() {
    let hex: String = deflate(&text(3000), 1).iter().map(|b| format!("{b:02X}")).collect();
    let data = format!("{hex}>").into_bytes();
    let dict = "/Filter [/ASCIIHexDecode /FlateDecode]";
    let after = saved(source_with_stream(dict, &data));
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);

    assert_eq!(bytes, data.as_slice());
    assert_eq!(
        dict.get("Filter"),
        Some(&Object::Array(vec![
            Object::Name("ASCIIHexDecode".to_string()),
            Object::Name("FlateDecode".to_string()),
        ]))
    );
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a re-encoded stream from the source's own bytes.
#[test]
fn a_stream_over_the_memory_limit_is_counted_then_written_or_copied() {
    let len = MEMORY_STREAM as usize * 3;

    // Noise keeps even the best encoding over the limit; the text is what
    // a higher level still shrinks.
    let data = [noise(MEMORY_STREAM as usize + 50_000), text(len)].concat();
    let slack = deflate(&data, 1);
    assert!(slack.len() as u64 > MEMORY_STREAM, "the fixture must take the counting path");
    let after = saved(source_with_stream("/Filter /FlateDecode", &slack));
    let object = stream_of(&after);
    let (_, bytes) = parts(&object);
    assert!(bytes.len() < slack.len(), "{} bytes, source {}", bytes.len(), slack.len());
    assert_eq!(object.decode_stream_data().unwrap(), data);

    let best = deflate(&data, 8);
    let after = saved(source_with_stream("/Filter /FlateDecode", &best));
    assert_eq!(parts(&stream_of(&after)).1, best.as_slice());

    let raw = text(len);
    let after = saved(source_with_stream("", &raw));
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);
    assert_eq!(dict.get("Filter"), Some(&Object::Name("FlateDecode".to_string())));
    assert!(bytes.len() < raw.len());
    assert_eq!(object.decode_stream_data().unwrap(), raw);

    let noisy = noise(len);
    let after = saved(source_with_stream("", &noisy));
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);
    assert!(!dict.contains_key("Filter"));
    assert_eq!(bytes, noisy.as_slice());

    let stored = deflate(&noisy, 9);
    let after = saved(source_with_stream("/Filter /FlateDecode", &stored));
    assert_eq!(parts(&stream_of(&after)).1, stored.as_slice());
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a copy from a re-encode.
#[test]
fn a_flate_stream_that_does_not_inflate_keeps_its_source_bytes() {
    let mut broken = deflate(&text(3000), 1);
    broken.truncate(broken.len() / 2);
    let after = saved(source_with_stream("/Filter /FlateDecode", &broken));
    assert_eq!(parts(&stream_of(&after)).1, broken.as_slice());

    let mut broken_big = deflate(&text(MEMORY_STREAM as usize * 3), 1);
    broken_big.truncate(broken_big.len() - 100);
    let after = saved(source_with_stream("/Filter /FlateDecode", &broken_big));
    assert_eq!(parts(&stream_of(&after)).1, broken_big.as_slice());
}

/// No Dart battery sees it: `deflate_level` is an engine field, and the Dart option is checked before it reaches the engine, so an out-of-range level never arrives here from Dart.
#[test]
fn a_higher_level_saves_smaller_and_a_level_above_nine_is_refused() {
    let data = text(60_000);
    let source = source_with_stream("", &data);
    let size = |level: u32| {
        let mut editor = DocumentEditor::from_bytes(source.clone()).unwrap();
        editor
            .save_to_bytes_with_options(SaveOptions { deflate_level: level, ..SaveOptions::full_rewrite() })
            .unwrap()
            .len()
    };
    assert!(size(9) < size(1), "level 9 {} bytes, level 1 {}", size(9), size(1));
    assert!(size(1) < size(0), "level 1 {} bytes, level 0 {}", size(1), size(0));

    let mut editor = DocumentEditor::from_bytes(source).unwrap();
    let refused =
        editor.save_to_bytes_with_options(SaveOptions { deflate_level: 10, ..SaveOptions::full_rewrite() });
    assert!(matches!(refused, Err(crate::error::Error::InvalidOperation(_))), "{refused:?}");
}

/// Text operators with numbers, as a page's content stream holds them.
fn content_like(len: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut out = Vec::with_capacity(len + 40);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let line = format!(
            "BT /F{} {} Tf {}.{} {}.{} Td ({}) Tj ET\n",
            state % 5,
            8 + state % 7,
            (state >> 8) % 600,
            (state >> 20) % 100,
            (state >> 28) % 800,
            (state >> 40) % 100,
            ["alpha", "bravo", "charlie", "delta", "echo"][(state >> 50) as usize % 5],
        );
        out.extend_from_slice(line.as_bytes());
    }
    out.truncate(len);
    out
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell one deflate of the same data from another.
#[test]
fn the_largest_level_saves_a_content_stream_smaller_than_zlib_level_nine() {
    let data = content_like(20_000);
    let zlib = deflate(&data, 9);
    let after = saved(source_with_stream("/Filter /FlateDecode", &zlib));
    let object = stream_of(&after);
    let (_, bytes) = parts(&object);

    assert!(bytes.len() < zlib.len(), "{} bytes, zlib level 9 {}", bytes.len(), zlib.len());
    assert_eq!(object.decode_stream_data().unwrap(), data);
}

/// A one-page document whose `/Contents` is stream object 4.
fn source_with_page_content(dict: &str, data: &[u8]) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(4, dict, data),
    ])
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a re-encoded page content from the source's own bytes.
#[test]
fn a_page_content_stream_is_redeflated_smaller_and_decodes_the_same() {
    let data = text(40_000);
    let weak = deflate(&data, 1);
    let raw_source = source_with_page_content("", &data);
    let weak_source = source_with_page_content("/Filter /FlateDecode", &weak);

    let after = saved(weak_source);
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);
    assert!(bytes.len() < weak.len(), "{} bytes, source {}", bytes.len(), weak.len());
    assert_eq!(dict.get("Filter"), Some(&Object::Name("FlateDecode".to_string())));
    assert_eq!(object.decode_stream_data().unwrap(), data);

    let after = saved(raw_source);
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);
    assert_eq!(dict.get("Filter"), Some(&Object::Name("FlateDecode".to_string())));
    assert!(bytes.len() < data.len());
    assert_eq!(object.decode_stream_data().unwrap(), data);
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a copy from a re-encode.
#[test]
fn a_page_content_stream_the_save_cannot_shrink_keeps_its_source_bytes() {
    let best = deflate(&text(40_000), 8);
    let after = saved(source_with_page_content("/Filter /FlateDecode", &best));
    assert_eq!(parts(&stream_of(&after)).1, best.as_slice());
}

fn lzw(data: &[u8], early_change: bool) -> Vec<u8> {
    use weezl::{encode::Encoder, BitOrder};
    let mut encoder = if early_change {
        Encoder::with_tiff_size_switch(BitOrder::Msb, 8)
    } else {
        Encoder::new(BitOrder::Msb, 8)
    };
    encoder.encode(data).unwrap()
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a Flate stream from the LZW one it replaced.
#[test]
fn an_lzw_stream_is_written_as_flate_smaller_and_decodes_the_same() {
    let data = text(40_000);
    let encoded = lzw(&data, true);
    let source = source_with_stream("/Filter /LZWDecode", &encoded);
    let after = saved(source);
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);

    assert!(bytes.len() < encoded.len(), "{} bytes, source {}", bytes.len(), encoded.len());
    assert_eq!(dict.get("Filter"), Some(&Object::Name("FlateDecode".to_string())));
    assert_eq!(object.decode_stream_data().unwrap(), data);
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a Flate stream from the LZW one it replaced.
#[test]
fn an_lzw_stream_keeps_its_predictor_and_loses_only_its_early_change() {
    let rows = predictor_rows(64, 400);
    let encoded = lzw(&rows, false);
    let dict = "/Filter [/LZWDecode] /DecodeParms [<< /Predictor 12 /Columns 64 /EarlyChange 0 >>]";
    let source = source_with_stream(dict, &encoded);
    let before = PdfDocument::from_bytes(source.clone()).unwrap().load_object(ObjectRef::new(4, 0)).unwrap();
    let after = saved(source);
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);

    assert!(bytes.len() < encoded.len(), "{} bytes, source {}", bytes.len(), encoded.len());
    assert_eq!(dict.get("Filter"), Some(&Object::Array(vec![Object::Name("FlateDecode".to_string())])));
    let Some(Object::Array(parms)) = dict.get("DecodeParms") else { panic!("{dict:?}") };
    let Some(Object::Dictionary(parms)) = parms.first() else { panic!("{dict:?}") };
    assert_eq!(parms.get("Predictor"), Some(&Object::Integer(12)));
    assert_eq!(parms.get("Columns"), Some(&Object::Integer(64)));
    assert!(!parms.contains_key("EarlyChange"), "{parms:?}");
    assert_eq!(object.decode_stream_data().unwrap(), before.decode_stream_data().unwrap());
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a copy from a re-encode.
#[test]
fn an_lzw_stream_that_does_not_decode_or_has_indirect_parameters_keeps_its_bytes() {
    let data = text(3000);
    let mut broken = lzw(&data, true);
    broken.truncate(broken.len() / 2);
    let after = saved(source_with_stream("/Filter /LZWDecode", &broken));
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);
    assert_eq!(bytes, broken.as_slice());
    assert_eq!(dict.get("Filter"), Some(&Object::Name("LZWDecode".to_string())));

    let encoded = lzw(&data, true);
    let source = pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra 4 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
        stream_object(4, "/Filter /LZWDecode /DecodeParms 5 0 R", &encoded),
        b"5 0 obj\n<< /EarlyChange 1 >>\nendobj\n".to_vec(),
    ]);
    let after = saved(source);
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);
    assert_eq!(bytes, encoded.as_slice());
    assert_eq!(dict.get("Filter"), Some(&Object::Name("LZWDecode".to_string())));
}

/// No Dart battery sees it: Dart reads a stream decoded, and a decoded read cannot tell a counted, streamed re-encode from the source's own bytes.
#[test]
fn an_lzw_stream_over_the_memory_limit_is_counted_then_written_as_flate() {
    let data = [noise(MEMORY_STREAM as usize + 50_000), text(MEMORY_STREAM as usize * 3)].concat();
    let encoded = lzw(&data, true);
    assert!(encoded.len() as u64 > MEMORY_STREAM, "the fixture must take the counting path");
    let after = saved(source_with_stream("/Filter /LZWDecode", &encoded));
    let object = stream_of(&after);
    let (dict, bytes) = parts(&object);

    assert!(bytes.len() < encoded.len(), "{} bytes, source {}", bytes.len(), encoded.len());
    assert_eq!(dict.get("Filter"), Some(&Object::Name("FlateDecode".to_string())));
    assert_eq!(object.decode_stream_data().unwrap(), data);
}

/// A document whose catalog reaches object 4, which the editor then stages as the stream `dict` and `data`.
fn editor_staging(dict: &str, data: Vec<u8>) -> DocumentEditor {
    let mut editor = DocumentEditor::from_bytes(source_with_stream("", b"x")).unwrap();
    let parsed = PdfDocument::from_bytes(source_with_stream(dict, b"x")).unwrap();
    let Object::Stream { mut dict, .. } = stream_of(&parsed) else { panic!("not a stream") };
    dict.insert("Length".to_string(), Object::Integer(data.len() as i64));
    editor.insert_modified(4, Object::stream(dict, bytes::Bytes::from(data)));
    editor
}

/// No Dart battery sees it: Dart cannot stage a stream of a given size, and the proof is the heap one save raises, which no wire op reports.
#[test]
fn a_staged_stream_over_the_memory_limit_is_deflated_without_a_copy_of_it() {
    let len = 16 * 1024 * 1024;
    let data = text(len);
    let mut editor = editor_staging("", data.clone());
    let (bytes, grew) =
        crate::host::heap::growth(|| editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap());
    assert!(grew < len / 4, "the save held {grew} bytes more for a staged stream of {len}");

    let after = PdfDocument::from_bytes(bytes).unwrap();
    let object = stream_of(&after);
    let (dict, stored) = parts(&object);
    assert_eq!(dict.get("Filter"), Some(&Object::Name("FlateDecode".to_string())));
    assert!(stored.len() < len / 4, "{} bytes stored", stored.len());
    assert_eq!(object.decode_stream_data().unwrap(), data);
}

/// No Dart battery sees it: Dart cannot stage a stream of a given size, and a decoded read cannot tell a copy from a re-encode.
#[test]
fn a_staged_stream_over_the_memory_limit_that_does_not_shrink_is_written_as_it_is() {
    let data = noise(MEMORY_STREAM as usize * 3);
    let mut editor = editor_staging("", data.clone());
    let bytes = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    let after = PdfDocument::from_bytes(bytes).unwrap();
    let object = stream_of(&after);
    let (dict, stored) = parts(&object);
    assert!(!dict.contains_key("Filter"));
    assert_eq!(stored, data.as_slice());

    let flate = deflate(&noise(MEMORY_STREAM as usize * 3), 9);
    let mut editor = editor_staging("/Filter /FlateDecode", flate.clone());
    let bytes = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    let after = PdfDocument::from_bytes(bytes).unwrap();
    assert_eq!(parts(&stream_of(&after)).1, flate.as_slice());
}
