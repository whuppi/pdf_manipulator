//! A size the file declares is checked before it sizes anything.

use crate::document::PdfDocument;

/// A file whose last cross-reference section is a stream with `dict` in it.
fn with_xref_stream(dict: &str) -> Vec<u8> {
    let mut out = b"%PDF-1.5\n".to_vec();
    out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    out.extend_from_slice(b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n");
    let at = out.len();
    out.extend_from_slice(format!("4 0 obj\n<< /Type /XRef /Size 5 /Root 1 0 R {dict} >>\nstream\n").as_bytes());
    out.extend_from_slice(&[0u8; 20]);
    out.extend_from_slice(b"\nendstream\nendobj\n");
    out.extend_from_slice(format!("startxref\n{at}\n%%EOF\n").as_bytes());
    out
}

/// A cross-reference stream that declares a `/Length` no file holds, or a
/// field width no entry can have, is a typed error or a repaired open, and
/// never an overflow: `stream + 20 + /Length + 30` and `/W[0] + /W[1] +
/// /W[2]` each panicked in a debug build ("attempt to add with overflow").
/// No Dart battery can observe this: a panic in a lane is the same failed
/// open as any other malformed file, so only a Rust test can tell an
/// overflow from an error.
#[test]
fn a_hostile_xref_stream_size_is_an_error_and_not_an_overflow() {
    for dict in [
        "/W [1 2 1] /Length 18446744073709551615",
        "/W [-1 1 0] /Length 20",
        "/W [1 9 1] /Length 20",
        "/W [1 2 1] /Length 4000000000000",
    ] {
        let opened = std::panic::catch_unwind(|| PdfDocument::from_bytes(with_xref_stream(dict)));
        assert!(opened.is_ok(), "{dict}: the open panicked");
    }
}

/// An object stream that declares a million objects over a table of eight
/// bytes is refused before the table of pairs is allocated: the pairs vector
/// used to be sized by `/N` alone, 16 MiB for a stream that holds two pairs.
/// No Dart battery can observe this: the allocation is released at once and
/// the open fails the same way either way.
#[test]
fn an_object_stream_n_larger_than_its_table_allocates_nothing() {
    use super::filter_readers::{HELD, PEAK};
    use crate::object::{Object, StreamBody};
    use std::sync::atomic::Ordering;
    let mut dict = std::collections::HashMap::new();
    dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
    dict.insert("N".to_string(), Object::Integer(1_000_000));
    dict.insert("First".to_string(), Object::Integer(8));
    let stream = Object::Stream { dict, body: StreamBody::from(b"1 0 2 5 << >> << >>".to_vec()) };
    let base = HELD.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let decoded = crate::objstm::decode_object_stream(&stream);
    let growth = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    assert!(decoded.is_err(), "N exceeds the table");
    assert!(growth < 4 * 1024 * 1024, "a declared /N of 1,000,000 allocated {growth} bytes");
}
