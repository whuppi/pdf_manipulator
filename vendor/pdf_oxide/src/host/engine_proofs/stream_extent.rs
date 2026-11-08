//! A stream of a document is an extent that reads itself.

use super::support::{corpus_pdfs, pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::error::Error;
use crate::object::{Object, ObjectRef, StreamBody};
use crate::writer::ObjectSerializer;
use std::io::Read;
use std::sync::Arc;

/// The bytes the extent names in the source, as stored.
fn stored(body: &StreamBody) -> Vec<u8> {
    let StreamBody::Extent(extent) = body else { panic!("not an extent: {body:?}") };
    let mut out = Vec::new();
    extent.stored_reader().read_to_end(&mut out).unwrap();
    out
}

/// What the committed loader handed out for a stream: the stored bytes, run
/// through `decrypt_stream` when the document is encrypted and the stream is
/// not an object stream or a cross-reference stream.
fn committed_bytes(doc: &PdfDocument, id: u32, dict: &std::collections::HashMap<String, Object>, body: &StreamBody) -> Result<Vec<u8>, Error> {
    let StreamBody::Extent(extent) = body else { panic!("not an extent: {body:?}") };
    let bytes = stored(body);
    let exempt = matches!(
        dict.get("Type").and_then(|t| t.as_name()),
        Some("ObjStm") | Some("XRef")
    );
    match extent.source().handler() {
        Some(handler) if !exempt => {
            handler.decrypt_stream(&bytes, id, doc.object_generation(id).unwrap_or(0) as u32)
        },
        _ => Ok(bytes),
    }
}

/// No Dart battery sees it: the loader's old and new reads of a stream are compared byte for byte on every stream of every fixture, which Dart sees only as a rendered page.
#[test]
fn every_fixture_stream_reads_the_bytes_the_committed_loader_held() {
    let (mut streams, mut decrypted) = (0, 0);
    for path in corpus_pdfs() {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(doc) = PdfDocument::from_bytes(bytes) else { continue };
        for id in doc.all_object_ids() {
            // An object inside an object stream is never a stream, and a free
            // entry names nothing: loading one sweeps every object stream.
            if doc.container_stream_of(id).is_some() || id == 0 || doc.is_free_entry(id) {
                continue;
            }
            let Ok(object @ Object::Stream { .. }) = doc.load_object(ObjectRef::new(id, 0)) else {
                continue;
            };
            let Object::Stream { dict, body } = &object else { continue };
            if !matches!(body, StreamBody::Extent(_)) {
                continue;
            }
            streams += 1;
            let expected = committed_bytes(&doc, id, dict, body);
            let got = body.read_whole_raw();
            match (expected, got) {
                (Ok(expected), Ok(got)) => {
                    assert_eq!(got, expected, "{} object {id}", path.display());
                    decrypted += usize::from(doc.is_encrypted());
                },
                (Err(_), Err(_)) => {},
                (expected, got) => {
                    panic!("{} object {id}: committed {expected:?}, extent {got:?}", path.display())
                },
            }
        }
    }
    assert!(streams > 100, "only {streams} streams were compared");
    assert!(decrypted > 0, "no encrypted stream was compared");
}

/// No Dart battery sees it: the equality is on the engine's object type, which Dart never holds.
#[test]
fn extents_are_equal_by_source_and_window_and_never_to_inline() {
    let data = b"some stream data".to_vec();
    let bytes = pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
        stream_object(4, "", &data),
    ]);
    let one = PdfDocument::from_bytes(bytes.clone()).unwrap();
    let other = PdfDocument::from_bytes(bytes).unwrap();
    let r = ObjectRef::new(4, 0);
    let (a, b, c) = (one.load_object(r).unwrap(), one.load_object(r).unwrap(), other.load_object(r).unwrap());
    assert_eq!(a, b, "the same window of the same source");
    assert_ne!(a, c, "the same bytes in another source");
    let Object::Stream { dict, .. } = &a else { panic!("not a stream") };
    assert_ne!(a, Object::stream(dict.clone(), data), "an extent never equals an inline body");
}

/// No Dart battery sees it: it reads one document's extent into an output that belongs to no document, which no Dart call composes.
#[test]
fn an_extent_written_elsewhere_reads_from_its_own_document() {
    let data: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    let bytes = pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
        stream_object(4, "", &data),
    ]);
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    let object = doc.load_object(ObjectRef::new(4, 0)).unwrap();
    let mut out = Vec::new();
    ObjectSerializer::compact().write_indirect(&mut out, 9, 0, &object).unwrap();
    assert!(out.starts_with(b"9 0 obj\n"));
    let at = out.windows(8).position(|w| w == b"\nstream\n").unwrap() + 8;
    assert_eq!(&out[at..at + data.len()], data.as_slice());
    assert!(out.ends_with(b"\nendstream\nendobj\n"));
    let length = format!("/Length {}", data.len());
    assert!(out.windows(length.len()).any(|w| w == length.as_bytes()));
}

/// No Dart battery sees it: the unencrypted bytes a rewrite writes are compared with what the committed serializer wrote for the same stream, and Dart sees only a file that opens.
#[test]
fn write_indirect_is_byte_identical_to_the_committed_serializer_on_unencrypted_fixtures() {
    let serializer = ObjectSerializer::compact();
    let mut compared = 0;
    for path in corpus_pdfs() {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(doc) = PdfDocument::from_bytes(bytes) else { continue };
        if doc.is_encrypted() {
            continue;
        }
        for id in doc.all_object_ids() {
            // An object inside an object stream is never a stream, and a free
            // entry names nothing: loading one sweeps every object stream.
            if doc.container_stream_of(id).is_some() || id == 0 || doc.is_free_entry(id) {
                continue;
            }
            let Ok(object) = doc.load_object(ObjectRef::new(id, 0)) else { continue };
            let Object::Stream { dict, body } = &object else { continue };
            let Ok(data) = body.read_whole_raw() else { continue };
            let mut committed_dict = dict.clone();
            committed_dict.insert("Length".to_string(), Object::Integer(data.len() as i64));
            let expected = serializer.serialize_indirect(id, 0, &Object::stream(committed_dict, data));
            let mut got = Vec::new();
            serializer.write_indirect(&mut got, id, 0, &object).unwrap();
            assert_eq!(got, expected, "{} object {id}", path.display());
            compared += 1;
        }
    }
    assert!(compared > 100, "only {compared} streams were compared");
}

/// A source that pauses after every seek, so a seek and the read that follows it
/// are far enough apart for another thread to land between them if the
/// engine ever let it.
struct SlowSeek(std::io::Cursor<Vec<u8>>);

impl Read for SlowSeek {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Seek for SlowSeek {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let at = self.0.seek(pos)?;
        std::thread::sleep(std::time::Duration::from_micros(2));
        Ok(at)
    }
}

/// No Dart battery sees it: two threads cold-load and read the same streams of one shared document, a Rust-level race no Dart call can stage.
#[test]
fn two_threads_cold_loading_and_reading_streams_never_see_each_others_bytes() {
    const ITERATIONS: usize = 10_000;
    const STREAMS: u32 = 12;
    let datas: Vec<Vec<u8>> = (0..STREAMS)
        .map(|i| (0..2000 + 700 * i).map(|b| (b as u8).wrapping_mul(i as u8 + 3)).collect())
        .collect();
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
    ];
    for (i, data) in datas.iter().enumerate() {
        objects.push(stream_object(4 + i as u32, "", data));
    }
    let doc = Arc::new(
        PdfDocument::from_external_reader(Box::new(SlowSeek(std::io::Cursor::new(
            pdf_from_objects(&objects),
        ))))
        .unwrap(),
    );
    let datas = Arc::new(datas);
    let threads: Vec<_> = (0..2)
        .map(|t| {
            let (doc, datas) = (Arc::clone(&doc), Arc::clone(&datas));
            std::thread::spawn(move || {
                for n in 0..ITERATIONS {
                    let i = (n * (5 + 2 * t) + t) % STREAMS as usize;
                    if n % 3 != 0 {
                        doc.clear_object_cache();
                    }
                    let object = doc.load_object(ObjectRef::new(4 + i as u32, 0)).unwrap();
                    let Object::Stream { body, .. } = &object else { panic!("not a stream") };
                    assert_eq!(body.read_whole_raw().unwrap(), datas[i], "thread {t} stream {i}");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}
