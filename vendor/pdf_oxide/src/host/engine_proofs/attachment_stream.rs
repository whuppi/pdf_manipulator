//! An attachment leaves the document in pieces: `extractAttachment` runs the
//! stream's filters as a reader and hands the sink at most 64 KiB at a time,
//! and `convertToPdf` has no answer without a sink.
//!
//! No Dart battery can observe this: a battery reads the sink's bytes, which
//! were the same whether they arrived in one write or many, and Dart always
//! hands `convertToPdf` a sink.

use std::io::Write;
use std::sync::{Arc, Mutex};

use crate::cancel::CancelCheck;
use crate::host::bridge_api::{handle_request, BoxedReader};
use crate::host::dispatch;
use crate::host::engine_state::EngineState;
use crate::PdfDocument;

use super::support::{pdf_from_objects, stream_object};

const KIB: usize = 1024;

/// A sink that keeps what it is given and the size of its largest write.
#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<(Vec<u8>, usize)>>);

impl Write for Recorder {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut inner = self.0.lock().unwrap();
        inner.0.extend_from_slice(buf);
        inner.1 = inner.1.max(buf.len());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

fn deflated(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    z.write_all(data).unwrap();
    z.finish().unwrap()
}

/// A document whose one attachment, `name`, is `stored` under `filter`.
fn with_attachment(name: &str, filter: &str, stored: &[u8]) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Names << /EmbeddedFiles 4 0 R >> >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec(),
        format!("4 0 obj\n<< /Names [({name}) 5 0 R] >>\nendobj\n").into_bytes(),
        format!("5 0 obj\n<< /Type /Filespec /F ({name}) /UF ({name}) /EF << /F 6 0 R >> >>\nendobj\n").into_bytes(),
        stream_object(6, &format!("/Type /EmbeddedFile {filter}"), stored),
    ])
}

fn extract(doc_bytes: Vec<u8>, name: &str) -> (crate::error::Result<()>, Vec<u8>, usize) {
    let mut doc = PdfDocument::from_bytes(doc_bytes).unwrap();
    let sink = Recorder::default();
    let result = dispatch::doc_extract_attachment(&mut doc, name, &mut sink.clone());
    let inner = sink.0.lock().unwrap();
    (result, inner.0.clone(), inner.1)
}

/// A stored attachment and a Flate one arrive whole, in writes of at most
/// 64 KiB.
#[test]
fn an_attachment_reaches_the_sink_in_pieces_of_at_most_64_kib() {
    let data = noise(700 * KIB);
    let cases = [
        ("plain", with_attachment("a.bin", "", &data)),
        ("flate", with_attachment("a.bin", "/Filter /FlateDecode", &deflated(&data))),
    ];
    for (label, pdf) in cases {
        let (result, got, widest) = extract(pdf, "a.bin");
        result.unwrap_or_else(|e| panic!("{label}: {e}"));
        assert!(got == data, "{label}: the attachment's bytes");
        assert!(widest <= 64 * KIB, "{label}: a write of {widest} bytes");
    }
}

/// A name no file spec carries is refused before the sink is touched.
#[test]
fn an_unknown_attachment_writes_nothing() {
    let (result, got, _) = extract(with_attachment("a.bin", "", b"abc"), "missing.bin");
    assert!(result.unwrap_err().to_string().contains("no attachment named missing.bin"));
    assert!(got.is_empty());
}

/// A cancel stops the copy between pieces.
#[test]
fn an_attachment_copy_polls_for_a_cancel() {
    let data = noise(700 * KIB);
    let mut doc = PdfDocument::from_bytes(with_attachment("a.bin", "", &data)).unwrap();
    let sink = Recorder::default();
    let _job = crate::cancel::enter(&CancelCheck::tripping_at(3));
    let result = dispatch::doc_extract_attachment(&mut doc, "a.bin", &mut sink.clone());
    assert!(result.unwrap_err().to_string().contains("operation cancelled"));
    assert!(sink.0.lock().unwrap().0.len() < data.len());
}

/// `convertToPdf` without a sink answers that it needs one; it does not read
/// the source whole to answer in the response.
#[test]
fn convert_to_pdf_without_a_sink_asks_for_one() {
    let mut frame = vec!["convertToPdf".len() as u8];
    frame.extend_from_slice(b"convertToPdf");
    frame.extend_from_slice(&0u16.to_le_bytes());
    let source = BoxedReader(Box::new(std::io::Cursor::new(b"PK\x03\x04".to_vec())));
    let mut state = EngineState::new();
    let answer = handle_request(&mut state, &frame, None, vec![source], Vec::new(), &CancelCheck::none());
    assert_eq!(answer[0], 0, "an error answer");
    let text = String::from_utf8_lossy(&answer);
    assert!(text.contains("convertToPdf requires a sink"), "{text}");
}
