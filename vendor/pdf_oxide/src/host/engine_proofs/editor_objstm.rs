//! Object streams in a rewrite: reachability and decode counts.

use super::editor_nested::nested_resources_pdf_bytes;
use super::support::{contains, pdf_from_objects, stream_object};
use crate::editor::{DocumentEditor, SaveOptions};
use crate::error::Error;
use crate::object::{Object, ObjectRef};
use std::collections::HashMap;

/// A page tree and an object stream of three objects. Object 5 is
/// bigger than a 4 KiB object cache and names objects 6 and 7, which
/// name the streams 9 and 10. The catalog reaches 5 as `/Extra`.
fn objstm_pdf_bytes() -> Vec<u8> {
    let pad = "x".repeat(8000);
    let bodies = [
        format!("<< /Pad ({pad}) /A 6 0 R /B 7 0 R >>"),
        "<< /S 9 0 R >>".to_string(),
        "<< /S 10 0 R >>".to_string(),
    ];
    let mut header = String::new();
    let mut at = 0;
    for (id, body) in [5, 6, 7].iter().zip(&bodies) {
        header.push_str(&format!("{id} {at} "));
        at += body.len() + 1;
    }
    let data = [header.clone(), bodies.iter().map(|b| format!("{b}\n")).collect::<String>()].concat();

    let mut out = b"%PDF-1.5\n".to_vec();
    let mut offsets = HashMap::new();
    let mut put = |out: &mut Vec<u8>, id: u32, object: Vec<u8>| {
        offsets.insert(id, out.len());
        out.extend_from_slice(&object);
    };
    put(&mut out, 1, b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra 5 0 R >>\nendobj\n".to_vec());
    put(&mut out, 2, b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec());
    put(&mut out, 3, b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec());
    put(
        &mut out,
        8,
        stream_object(8, &format!("/Type /ObjStm /N 3 /First {}", header.len()), data.as_bytes()),
    );
    put(&mut out, 9, stream_object(9, "", b"REACHEDNINE"));
    put(&mut out, 10, stream_object(10, "", b"REACHEDTEN"));

    let xref_at = out.len();
    let mut entries = Vec::new();
    let mut entry = |kind: u8, field: u32, last: u16| {
        entries.push(kind);
        entries.extend_from_slice(&field.to_be_bytes());
        entries.extend_from_slice(&last.to_be_bytes());
    };
    entry(0, 0, 65535);
    for id in 1..=3 {
        entry(1, offsets[&id] as u32, 0);
    }
    entry(0, 0, 0);
    for index in 0..3 {
        entry(2, 8, index);
    }
    for id in [8, 9, 10] {
        entry(1, offsets[&id] as u32, 0);
    }
    entry(1, xref_at as u32, 0);
    out.extend_from_slice(
        format!("11 0 obj\n<< /Type /XRef /Size 12 /W [1 4 2] /Root 1 0 R /Length {} >>\nstream\n", entries.len())
            .as_bytes(),
    );
    out.extend_from_slice(&entries);
    out.extend_from_slice(format!("\nendstream\nendobj\nstartxref\n{xref_at}\n%%EOF\n").as_bytes());
    out
}

/// A document whose object cache holds 4 KiB, less than object 5.
fn objstm_doc_with_a_small_cache() -> crate::document::PdfDocument {
    crate::document::PdfDocument::from_external_reader_bounded(
        Box::new(std::io::Cursor::new(objstm_pdf_bytes())),
        4096,
    )
    .unwrap()
}

/// No Dart battery sees it: it builds an object-stream source behind a 4 KiB object cache, a Rust constructor Dart does not offer.
#[test]
fn collect_refs_of_returns_the_same_refs_cold_and_warm() {
    let cold = objstm_doc_with_a_small_cache();
    let mut cold_refs = Vec::new();
    cold.collect_refs_of(5, &mut cold_refs);

    let warm = crate::document::PdfDocument::from_bytes(objstm_pdf_bytes()).unwrap();
    warm.load_object(ObjectRef::new(5, 0)).unwrap();
    let mut warm_refs = Vec::new();
    warm.collect_refs_of(5, &mut warm_refs);

    cold_refs.sort_unstable();
    warm_refs.sort_unstable();
    assert_eq!(warm_refs, vec![6, 7]);
    assert_eq!(cold_refs, warm_refs);
}

/// No Dart battery sees it: it builds an object-stream source behind a 4 KiB object cache, a Rust constructor Dart does not offer.
#[test]
fn a_full_rewrite_keeps_what_an_object_too_big_for_the_cache_reaches() {
    let mut editor = DocumentEditor::from_document(objstm_doc_with_a_small_cache()).unwrap();
    let options = SaveOptions { compress: false, ..SaveOptions::full_rewrite() };
    let bytes = editor.save_to_bytes_with_options(options).unwrap();

    let saved = crate::document::PdfDocument::from_bytes(bytes.clone()).unwrap();
    for id in [5, 6, 7, 9, 10] {
        let object = saved.load_object(ObjectRef::new(id, 0)).unwrap();
        assert!(!matches!(object, Object::Null), "object {id} was not written");
    }
    assert!(contains(&bytes, b"REACHEDNINE"), "stream 9 was not written");
    assert!(contains(&bytes, b"REACHEDTEN"), "stream 10 was not written");
}

/// No Dart battery sees it: it builds an object-stream source behind a 4 KiB object cache, a Rust constructor Dart does not offer.
#[test]
fn stream_head_finds_streams_whatever_the_size_of_their_dictionary() {
    let small = b"hello stream".to_vec();
    let wide = b"wide dictionary stream".to_vec();
    let doc = crate::document::PdfDocument::from_bytes(pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec(),
        stream_object(4, "", &small),
        // A dictionary wider than the first read of the source.
        stream_object(5, &format!("/Pad ({})", "p".repeat(5000)), &wide),
    ]))
    .unwrap();

    for (id, data) in [(4, &small), (5, &wide)] {
        let head = doc.stream_head(ObjectRef::new(id, 0)).unwrap().expect("a plain stream");
        assert_eq!(head.data_len() as usize, data.len(), "object {id}");
        let mut copied = Vec::new();
        std::io::Read::read_to_end(&mut head.extent.stored_reader(), &mut copied).unwrap();
        assert_eq!(&copied, data, "object {id}");
    }
    for id in [1, 3] {
        assert!(doc.stream_head(ObjectRef::new(id, 0)).unwrap().is_none(), "object {id}");
    }
}

/// Members per object stream of `two_objstm_pdf_bytes`.
const TWO_STREAM_MEMBERS: u32 = 8;

/// A page tree and two Flate object streams of `TWO_STREAM_MEMBERS` 3 KB
/// dictionaries each, ids 10..18 in stream 100 and 18..26 in stream 101,
/// so each stream decodes to more than a 4 KiB object cache holds. The
/// catalog's /Extra names the members alternating between the streams.
fn two_objstm_pdf_bytes() -> Vec<u8> {
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    let n = TWO_STREAM_MEMBERS;
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut offsets = HashMap::new();
    let mut put = |out: &mut Vec<u8>, id: u32, object: Vec<u8>| {
        offsets.insert(id, out.len());
        out.extend_from_slice(&object);
    };
    let extra: String = (0..n)
        .flat_map(|i| [10 + i, 10 + n + i])
        .map(|id| format!("{id} 0 R "))
        .collect();
    put(&mut out, 1, format!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra [{extra}] >>\nendobj\n").into_bytes());
    put(&mut out, 2, b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec());
    put(&mut out, 3, b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec());
    for (stream_id, first_member) in [(100u32, 10u32), (101, 10 + n)] {
        let mut header = String::new();
        let mut bodies = String::new();
        for id in first_member..first_member + n {
            header.push_str(&format!("{id} {} ", bodies.len()));
            bodies.push_str(&format!("<< /Pad ({}) /Tag (MEMBER{id}) >>\n", "x".repeat(3000)));
        }
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(format!("{header}{bodies}").as_bytes()).unwrap();
        let packed = encoder.finish().unwrap();
        put(
            &mut out,
            stream_id,
            stream_object(
                stream_id,
                &format!("/Type /ObjStm /N {n} /First {} /Filter /FlateDecode", header.len()),
                &packed,
            ),
        );
    }

    let xref_at = out.len();
    let size = 103u32;
    let mut entries = Vec::new();
    for id in 0..size - 1 {
        let (kind, field, last) = match id {
            1..=3 | 100 | 101 => (1u8, offsets[&id] as u32, 0u16),
            10..=25 => {
                let stream = if id < 10 + n { 100 } else { 101 };
                (2, stream, ((id - 10) % n) as u16)
            },
            _ => (0, 0, if id == 0 { 65535 } else { 0 }),
        };
        entries.push(kind);
        entries.extend_from_slice(&field.to_be_bytes());
        entries.extend_from_slice(&last.to_be_bytes());
    }
    entries.push(1);
    entries.extend_from_slice(&(xref_at as u32).to_be_bytes());
    entries.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(
        format!(
            "{} 0 obj\n<< /Type /XRef /Size {size} /W [1 4 2] /Root 1 0 R /Length {} >>\nstream\n",
            size - 1,
            entries.len()
        )
        .as_bytes(),
    );
    out.extend_from_slice(&entries);
    out.extend_from_slice(format!("\nendstream\nendobj\nstartxref\n{xref_at}\n%%EOF\n").as_bytes());
    out
}

fn two_objstm_doc_with_a_small_cache() -> crate::document::PdfDocument {
    crate::document::PdfDocument::from_external_reader_bounded(
        Box::new(std::io::Cursor::new(two_objstm_pdf_bytes())),
        4096,
    )
    .unwrap()
}

/// No Dart battery sees it: it counts object-stream decodes (`objstm_inflations`), an internal no Dart call returns.
#[test]
fn a_full_rewrite_decodes_each_object_stream_at_most_twice() {
    let mut editor = DocumentEditor::from_document(two_objstm_doc_with_a_small_cache()).unwrap();
    let bytes = editor
        .save_to_bytes_with_options(SaveOptions { object_streams: false, ..SaveOptions::full_rewrite() })
        .unwrap();

    // Once for the reachability walk, once for the sweep that writes.
    let decoded = editor.source().objstm_inflations();
    assert!(decoded <= 4, "two object streams were decoded {decoded} times");

    let saved = crate::document::PdfDocument::from_bytes(bytes.clone()).unwrap();
    for id in 10..10 + 2 * TWO_STREAM_MEMBERS {
        assert!(
            contains(&bytes, format!("(MEMBER{id})").as_bytes()),
            "member {id} was not written"
        );
        assert!(!matches!(saved.load_object(ObjectRef::new(id, 0)).unwrap(), Object::Null));
    }
}

/// No Dart battery sees it: it counts object-stream decodes (`objstm_inflations`), an internal no Dart call returns.
#[test]
fn the_decoded_object_stream_is_kept_until_released() {
    let doc = two_objstm_doc_with_a_small_cache();
    let member = |id: u32| doc.load_object(ObjectRef::new(id, 0)).unwrap();

    member(10);
    member(11);
    member(12);
    assert_eq!(doc.objstm_inflations(), 1, "members of one stream share one decode");

    doc.release_objstm_memo();
    member(13);
    assert_eq!(doc.objstm_inflations(), 2, "a released memo decodes again");

    member(10 + TWO_STREAM_MEMBERS);
    assert_eq!(doc.objstm_inflations(), 3, "the memo holds one stream");
}

/// No Dart battery sees it: it marks an object as withheld, which only the Rust API can do.
#[test]
fn the_rewrite_fails_when_a_written_object_names_one_it_did_not_write() {
    let mut editor = DocumentEditor::from_bytes(nested_resources_pdf_bytes()).unwrap();
    // The sweep skips this id, which page 4 still names.
    editor.withhold_from_rewrite(8);
    let err = editor.save_to_bytes().unwrap_err();
    assert!(
        matches!(&err, Error::InvalidPdf(m) if m == "the rewrite would leave object 8 referenced by 4 unwritten"),
        "{err}"
    );
}

/// No Dart battery sees it: it builds an object-stream source behind a 4 KiB object cache, a Rust constructor Dart does not offer.
#[test]
fn a_reference_the_source_itself_leaves_dangling_does_not_fail_the_rewrite() {
    let source = pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Gone 99 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec(),
    ]);
    let mut editor = DocumentEditor::from_bytes(source).unwrap();
    editor.save_to_bytes().unwrap();
}

/// The number of `/Type /ObjStm` streams in a saved file.
fn object_stream_count(bytes: &[u8]) -> usize {
    bytes.windows(7).filter(|w| *w == b"/ObjStm").count()
}

/// A document of `count` link annotations (about 230 bytes serialized each:
/// the same keys, a rectangle and a target that vary) the catalog reaches
/// through `/Extra`.
fn many_small_objects(count: u32) -> Vec<u8> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = |bound: u64| -> u64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 20) % bound
    };
    let first = 4;
    let extra: String = (first..first + count).map(|id| format!("{id} 0 R ")).collect();
    let mut objects = vec![
        format!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra [{extra}] >>\nendobj\n").into_bytes(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n".to_vec(),
    ];
    for id in first..first + count {
        let (x, y) = (next(500), next(700));
        let target = format!("https://example.org/articles/{}/{}.html#s{}", next(900), next(90_000), next(40));
        objects.push(
            format!(
                "{id} 0 obj\n<< /Type /Annot /Subtype /Link /Border [0 0 0] /C [0 1 1] /F 4 \
                 /Rect [{x} {y} {} {}] /A << /Type /Action /S /URI /URI ({target}) >> >>\nendobj\n",
                x + 20 + next(80),
                y + 8 + next(6)
            )
            .into_bytes(),
        );
    }
    pdf_from_objects(&objects)
}

/// No Dart battery sees it: Dart reads a saved file's objects, not how many object streams hold them.
#[test]
fn small_objects_fill_every_object_stream_to_its_member_count() {
    let count = 1000;
    let source = many_small_objects(count);
    let mut editor = DocumentEditor::from_bytes(source).unwrap();
    let bytes = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();

    let objects = count as usize + 3;
    let streams = object_stream_count(&bytes);
    let members = crate::host::object_streams::MEMBERS;
    assert!(
        streams <= objects.div_ceil(members) + 1,
        "{objects} objects in {streams} object streams of at most {members}"
    );

    let saved = crate::document::PdfDocument::from_bytes(bytes).unwrap();
    for id in 4..4 + count {
        assert!(matches!(saved.load_object(ObjectRef::new(id, 0)).unwrap(), Object::Dictionary(_)), "object {id}");
    }
}

/// The offsets of `rows` objects whose sizes vary from 150 to 549 bytes with no period.
fn xref_offsets(rows: u32) -> Vec<u64> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut at = 9;
    (0..rows)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            at += 150 + (state >> 20) % 400;
            at
        })
        .collect()
}

/// The (type, field two, field three) of `rows` objects the way a save with object streams lays them out: a stream every 128th object, the rest packed in it by index.
fn xref_fields(rows: u32) -> Vec<(u8, u64, u16)> {
    let offsets = xref_offsets(rows);
    (0..rows)
        .map(|id| match id {
            0 => (0, 0, 65535),
            id if id % 128 == 0 => (1, offsets[id as usize], 0),
            id => (2, (id - id % 128) as u64, (id % 128) as u16),
        })
        .collect()
}

/// The cross-reference stream of those rows, as `compress` writes it.
fn xref_of(rows: u32) -> Object {
    use crate::host::object_streams::{field_width, xref_stream, Row};
    let fields = xref_fields(rows);
    let widest = fields.iter().map(|f| f.1).max().unwrap_or(0);
    let table = fields.into_iter().map(|(kind, second, third)| match kind {
        0 => Row::Free { next: second as u32, gen: third },
        1 => Row::Offset { offset: second, gen: third },
        _ => Row::Packed { stream: second as u32, index: third as u32 },
    });
    xref_stream(table, field_width(widest), Some(9), HashMap::new(), &crate::cancel::CancelCheck::none())
        .unwrap()
}

/// What the rows of `xref_of` hold, with the given field-two width.
fn xref_rows(rows: u32, width: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for (kind, second, third) in xref_fields(rows) {
        out.push(kind);
        out.extend((0..width).map(|i| (second >> (8 * (width - 1 - i))) as u8));
        out.extend_from_slice(&third.to_be_bytes());
    }
    out
}

/// No Dart battery sees it: Dart reads the objects of a saved file, and which way the cross-reference stream is encoded changes no object.
#[test]
fn a_small_cross_reference_stream_is_written_without_the_predictor() {
    let xref = xref_of(24);
    let Object::Stream { dict, .. } = &xref else { panic!("not a stream") };
    assert!(!dict.contains_key("DecodeParms"), "the predictor was kept: {dict:?}");
    assert_eq!(xref.decode_stream_data().unwrap(), xref_rows(24, 1));
}

/// No Dart battery sees it: Dart reads the objects of a saved file, and which way the cross-reference stream is encoded changes no object.
#[test]
fn a_large_cross_reference_stream_keeps_the_predictor_and_decodes_to_its_rows() {
    let xref = xref_of(20_000);
    let Object::Stream { dict, .. } = &xref else { panic!("not a stream") };
    assert!(dict.contains_key("DecodeParms"), "the predictor was dropped: {dict:?}");
    assert_eq!(xref.decode_stream_data().unwrap(), xref_rows(20_000, 3));
}
