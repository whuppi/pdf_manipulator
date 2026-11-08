//! A stream's extent read without loading its data.

use crate::document::{PdfDocument, StreamHead};
use crate::object::ObjectRef;

/// A sound PDF whose object 4 is `object_4` (a whole indirect object),
/// with the integer 11 as object 5.
fn pdf_with(object_4: &[u8]) -> Vec<u8> {
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
        object_4.to_vec(),
        b"5 0 obj\n11\nendobj\n".to_vec(),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for o in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(o);
    }
    let xref_at = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for off in offsets {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n").as_bytes(),
    );
    pdf
}

fn head_of(object_4: &[u8]) -> Option<StreamHead> {
    let doc = PdfDocument::from_bytes(pdf_with(object_4)).unwrap();
    doc.stream_head(ObjectRef { id: 4, gen: 0 }).unwrap()
}

/// The bytes the head's extent names in the source, as stored.
fn stored(head: &StreamHead) -> Vec<u8> {
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut head.extent.stored_reader(), &mut out).unwrap();
    out
}

/// No Dart battery sees it: `stream_head` is a crate-internal read that no Dart call exposes.
#[test]
fn direct_length_gives_the_data_extent() {
    let head = head_of(b"4 0 obj\n<< /Length 5 /Subtype /Image >>\nstream\nhello\nendstream\nendobj\n");
    let head = head.expect("a plain stream");
    assert_eq!(head.data_len(), 5);
    assert_eq!(stored(&head), b"hello");
    assert_eq!(head.dict.get("Subtype").and_then(|o| o.as_name()), Some("Image"));
}

/// No Dart battery sees it: `stream_head` is a crate-internal read that no Dart call exposes.
#[test]
fn indirect_length_is_resolved() {
    let head = head_of(b"4 0 obj\n<< /Length 5 0 R >>\nstream\nabcdefghijk\nendstream\nendobj\n");
    let head = head.expect("a stream with an indirect length");
    assert_eq!(head.data_len(), 11);
    assert_eq!(stored(&head), b"abcdefghijk");
}

/// No Dart battery sees it: `stream_head` is a crate-internal read that no Dart call exposes.
#[test]
fn crlf_after_stream_is_skipped() {
    let head = head_of(b"4 0 obj\n<< /Length 3 >>\nstream\r\nxyz\r\nendstream\nendobj\n");
    let head = head.expect("a stream with CRLF");
    assert_eq!(stored(&head), b"xyz");
}

/// No Dart battery sees it: `stream_head` is a crate-internal read that no Dart call exposes.
#[test]
fn wrong_length_is_repaired_to_the_data() {
    let head = head_of(b"4 0 obj\n<< /Length 2 >>\nstream\nhello\nendstream\nendobj\n");
    let head = head.expect("a stream whose /Length lies");
    assert_eq!(stored(&head), b"hello");
}

/// No Dart battery sees it: `stream_head` is a crate-internal read that no Dart call exposes.
#[test]
fn non_stream_object_gives_none() {
    let head = head_of(b"4 0 obj\n<< /Type /Font >>\nendobj\n");
    assert!(head.is_none());
}

/// No Dart battery sees it: it asserts what the object cache holds, and Dart cannot read the cache.
#[test]
fn a_stream_head_caches_the_dictionary_and_not_the_data() {
    let pdf = pdf_with(b"4 0 obj\n<< /Length 5 >>\nstream\nhello\nendstream\nendobj\n");
    let doc = PdfDocument::from_bytes(pdf).unwrap();
    let r = ObjectRef { id: 4, gen: 0 };
    assert!(doc.stream_head(r).unwrap().is_some());
    assert!(doc.object_is_cached(r));
}

/// No Dart battery sees it: a file whose /Length lies still opens and its stream reads the same in Dart; what differs is the extent the engine keeps, which no Dart call reports.
#[test]
fn the_seven_length_fixtures_all_read_the_data() {
    let fixtures: [(&str, &[u8], &[u8]); 7] = [
        (
            "a direct /Length that holds",
            b"4 0 obj\n<< /Length 5 >>\nstream\nhello\nendstream\nendobj\n",
            b"hello",
        ),
        (
            "an indirect /Length that holds",
            b"4 0 obj\n<< /Length 5 0 R >>\nstream\nabcdefghijk\nendstream\nendobj\n",
            b"abcdefghijk",
        ),
        (
            "a /Length that is too short",
            b"4 0 obj\n<< /Length 2 >>\nstream\nhello\nendstream\nendobj\n",
            b"hello",
        ),
        (
            "a /Length that is too long",
            b"4 0 obj\n<< /Length 9 >>\nstream\nhello\nendstream\nendobj\n",
            b"hello",
        ),
        (
            "no /Length",
            b"4 0 obj\n<< /Subtype /Image >>\nstream\nhello\nendstream\nendobj\n",
            b"hello",
        ),
        (
            "an indirect /Length that names a dictionary",
            b"4 0 obj\n<< /Length 3 0 R >>\nstream\nhello\nendstream\nendobj\n",
            b"hello",
        ),
        (
            "a /Length past the end of the file",
            b"4 0 obj\n<< /Length 99999999 >>\nstream\nhello\nendstream\nendobj\n",
            b"hello",
        ),
    ];
    for (what, object, data) in fixtures {
        let head = head_of(object).unwrap_or_else(|| panic!("{what}: not a stream"));
        assert_eq!(stored(&head), data, "{what}");
        let doc = PdfDocument::from_bytes(pdf_with(object)).unwrap();
        let loaded = doc.load_object(ObjectRef { id: 4, gen: 0 }).unwrap();
        assert_eq!(loaded.decode_stream_data().unwrap(), data, "{what}");
    }
}

/// No Dart battery sees it: the end of line before `endstream` is part of the framing, and which of the three a repaired stream drops is invisible once the viewer renders it.
#[test]
fn a_repaired_stream_drops_one_end_of_line_before_endstream() {
    for (eol, what) in [(&b"\n"[..], "LF"), (b"\r\n", "CRLF"), (b"\r", "CR"), (b"", "none")] {
        let object = [
            b"4 0 obj\n<< /Length 1 >>\nstream\nhello".as_slice(),
            eol,
            b"endstream\nendobj\n",
        ]
        .concat();
        let head = head_of(&object).unwrap_or_else(|| panic!("{what}: not a stream"));
        assert_eq!(stored(&head), b"hello", "{what}");
    }
}

/// No Dart battery sees it: the placeholder an unterminated stream becomes is an object the engine keeps, and Dart reads only the pages built from it.
#[test]
fn a_stream_with_no_endstream_anywhere_is_null() {
    let object = b"4 0 obj\n<< /Length 5 >>\nstream\nhello\nendobj\n";
    assert!(head_of(object).is_none());
    let doc = PdfDocument::from_bytes(pdf_with(object)).unwrap();
    assert!(doc.load_object(ObjectRef { id: 4, gen: 0 }).unwrap().is_null());
}

/// No Dart battery sees it: it asserts what the object cache charges, and Dart cannot read the cache.
#[test]
fn a_stream_costs_the_object_cache_its_dictionary_only() {
    let data = vec![b'x'; 4 * 1024 * 1024];
    let object = [
        format!("4 0 obj\n<< /Length {} >>\nstream\n", data.len()).into_bytes(),
        data,
        b"\nendstream\nendobj\n".to_vec(),
    ]
    .concat();
    let doc = PdfDocument::from_bytes(pdf_with(&object)).unwrap();
    let before = doc.object_cache_bytes();
    let r = ObjectRef { id: 4, gen: 0 };
    assert!(doc.stream_head(r).unwrap().is_some());
    assert!(doc.object_is_cached(r));
    let charged = doc.object_cache_bytes() - before;
    assert!(charged < 4096, "a 4 MiB stream was charged {charged} bytes");
}
