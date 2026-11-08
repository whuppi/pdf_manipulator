//! XFA packets are read as XML events over their decoded streams, never as a
//! whole buffer or a whole string.

use super::filter_readers::thread_growth;
use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::xfa::{analyze_xfa_document, XfaExtractor};
use std::io::Write;

const PACKET: usize = 64 * 1024 * 1024;

const TEMPLATE: &str = r#"<template xmlns="http://www.xfa.org/schema/xfa-template/3.0/">
<subform name="form1">
<field name="firstName"><ui><textEdit/></ui></field>
<field name="lastName"><ui><textEdit/></ui></field>
</subform>
</template>"#;

const DATASETS_HEAD: &str =
    "<xfa:datasets xmlns:xfa=\"http://www.xfa.org/schema/xfa-data/1.0/\"><xfa:data><form1>\n";
const DATASETS_TAIL: &str = "</form1></xfa:data></xfa:datasets>";

/// A datasets packet of `bytes` bytes once decoded, Flate-coded block by block
/// as it is generated: every value names one key, so the values it yields stay
/// one.
fn datasets(bytes: usize) -> Vec<u8> {
    let line = "<firstName>John</firstName>\n";
    let block = line.repeat(64 * 1024 / line.len());
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(DATASETS_HEAD.as_bytes()).unwrap();
    for _ in 0..bytes / block.len() {
        encoder.write_all(block.as_bytes()).unwrap();
    }
    encoder.write_all(DATASETS_TAIL.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

fn document(acroform: &str, streams: Vec<Vec<u8>>) -> PdfDocument {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /AcroForm 4 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        format!("4 0 obj\n<< /Fields [] /XFA {acroform} >>\nendobj\n").into_bytes(),
    ];
    objects.extend(streams);
    PdfDocument::from_bytes(pdf_from_objects(&objects)).unwrap()
}

/// A datasets packet of 64 MiB once decoded is analysed and parsed with the
/// thread holding under 1 MiB over where it began, and the form still carries
/// the value the packet gives its field. The whole-buffer lint
/// (`tool/check_whole_buffers.sh`) fails when an XFA caller goes back to
/// decoding a packet whole. No Dart battery can observe this: Dart reads the same field count
/// either way, and what the parse holds is visible only to the allocator
/// counter.
#[test]
fn a_64_mib_datasets_packet_is_analysed_as_events() {
    let mut doc = document(
        "[(template) 5 0 R (datasets) 6 0 R]",
        vec![
            stream_object(5, "", TEMPLATE.as_bytes()),
            stream_object(6, "/Filter /FlateDecode", &datasets(PACKET)),
        ],
    );
    let (analysis, analysed) = thread_growth(|| analyze_xfa_document(&mut doc).unwrap());
    println!("64 MiB XFA datasets packet analysed: peak growth {analysed} bytes");
    assert!(analysis.has_xfa);
    assert_eq!(analysis.field_count, Some(2));
    assert_eq!(analysis.page_count, Some(1));
    assert!(analysed < 1024 * 1024, "analysing a 64 MiB packet grew the thread by {analysed} bytes");

    let (form, parsed) = thread_growth(|| XfaExtractor::parse_form(&mut doc).unwrap());
    assert!(parsed < 1024 * 1024, "parsing a 64 MiB packet grew the thread by {parsed} bytes");
    assert_eq!(form.get_field("firstName").unwrap().value.as_deref(), Some("John"));
    assert_eq!(form.get_field("lastName").unwrap().value, None);
    assert_eq!(form.dataset_values.len(), 1);
}

/// A complete XDP in one stream and the same form as an array of packets give
/// the same fields and values, the first from the stream read as events with
/// its sections found by element, the second packet by packet. No Dart battery
/// can observe this: it compares the engine's parsed form field by field.
#[test]
fn one_xdp_stream_and_an_array_of_packets_give_the_same_form() {
    let datasets = format!("{DATASETS_HEAD}<firstName>Ada</firstName><lastName>Lovelace</lastName>{DATASETS_TAIL}");
    let xdp = format!(
        "<?xml version=\"1.0\"?><xdp:xdp xmlns:xdp=\"http://ns.adobe.com/xdp/\">{TEMPLATE}{datasets}</xdp:xdp>"
    );
    let mut single = document("5 0 R", vec![stream_object(5, "", xdp.as_bytes())]);
    let mut array = document(
        "[(template) 5 0 R (datasets) 6 0 R]",
        vec![stream_object(5, "", TEMPLATE.as_bytes()), stream_object(6, "", datasets.as_bytes())],
    );
    let from_stream = XfaExtractor::parse_form(&mut single).unwrap();
    let from_array = XfaExtractor::parse_form(&mut array).unwrap();
    for form in [&from_stream, &from_array] {
        assert_eq!(form.fields.len(), 2);
        assert_eq!(form.get_field("firstName").unwrap().value.as_deref(), Some("Ada"));
        assert_eq!(form.get_field("lastName").unwrap().value.as_deref(), Some("Lovelace"));
        assert_eq!(form.pages.len(), 1);
    }
    assert_eq!(from_stream.dataset_values, from_array.dataset_values);
}
