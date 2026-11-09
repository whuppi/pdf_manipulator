//! The PDF/A identification is spliced into the XMP packet as it is read, and the
//! conversion stages its edits.

use super::filter_readers::thread_growth;
use super::support::{open_editor_from_bytes, pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::compliance::{validate_pdf_a, ConversionResult, PdfAConverter, PdfALevel};
use crate::compliance::types::ValidationResult;
use crate::document::PdfDocument;
use crate::host::dispatch::edit_convert_to_pdf_a;
use crate::object::Object;
use std::io::Write;

const HEAD: &str = "<?xpacket begin='x' id='W5M0MpCehiHzreSzNTczkc9d'?>\n<x:xmpmeta xmlns:x='adobe:ns:meta/'>\n\
                    <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
                    <rdf:Description xmlns:pdf='http://ns.adobe.com/pdf/1.3/'>\n";
const TAIL: &str = "</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end='w'?>";
const LINE: &str = "<pdf:Producer>probe</pdf:Producer>\n";
/// What the identification adds before the closing `</rdf:RDF>`.
const BLOCK: &str = "    <rdf:Description rdf:about=\"\" xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\">\n\
      <pdfaid:part>2</pdfaid:part>\n\
      <pdfaid:conformance>B</pdfaid:conformance>\n\
    </rdf:Description>\n";

fn packet(lines: usize) -> Vec<u8> {
    format!("{HEAD}{}{TAIL}", LINE.repeat(lines)).into_bytes()
}

/// The document with `body` as its metadata stream under `dict`.
fn document(dict: &str, body: &[u8]) -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Metadata 4 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        stream_object(4, &format!("/Type /Metadata /Subtype /XML {dict}"), body),
    ])
}

fn flate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// The packet the staged metadata stream holds, and what it grew the thread by.
fn identified(file: &[u8]) -> (Vec<u8>, usize) {
    let mut editor = open_editor_from_bytes(file, "pdfa_xmp");
    let converter = PdfAConverter::new(PdfALevel::A2b);
    let mut result = ConversionResult {
        success: false,
        level: PdfALevel::A2b,
        validation: ValidationResult::new(PdfALevel::A2b),
        actions: Vec::new(),
        errors: Vec::new(),
    };
    let (done, growth) = thread_growth(|| converter.add_pdfa_identification(&mut editor, &mut result));
    done.unwrap();
    // A rebuilt packet is a new object the staged catalog points at.
    let id = match editor.get_modified(1) {
        Some(Object::Dictionary(catalog)) => catalog.get("Metadata").and_then(Object::as_reference).map_or(4, |r| r.id),
        _ => 4,
    };
    let Some(Object::Stream { dict, body }) = editor.get_modified(id) else { panic!("a staged stream") };
    assert!(dict.get("Filter").is_none(), "PDF/A wants a plain packet");
    (body.inline().expect("the staged packet is held in memory").to_vec(), growth)
}

/// The packet gains the identification block before its last `</rdf:RDF>`
/// and nothing else, Flate-coded or plain, and a packet with no RDF root is
/// rebuilt instead. No Dart battery can observe this: the validator reads the
/// identification the same however the packet was rewritten, and the bytes
/// around the block are not read back by anything but this proof.
#[test]
fn the_identification_is_spliced_before_the_last_rdf_close() {
    let plain = packet(3);
    let expected = format!(
        "{HEAD}{}</rdf:Description>\n{BLOCK}</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end='w'?>",
        LINE.repeat(3)
    );
    for (what, file) in [
        ("plain", document("", &plain)),
        ("flate", document("/Filter /FlateDecode", &flate(&plain))),
    ] {
        let (got, _) = identified(&file);
        assert_eq!(String::from_utf8(got).unwrap(), expected, "{what}");
    }
    let (rebuilt, _) = identified(&document("", b"<html>not xmp</html>"));
    assert!(String::from_utf8(rebuilt).unwrap().contains("<pdfaid:part>2</pdfaid:part>"));
}

/// A packet of 64 MiB is identified with the thread growing by the patched
/// packet (the stream the editor stages), not by the decoded text, a lossy
/// copy of it and the patched text beside it. No Dart battery can observe
/// this: the conversion's own commit holds the saved file, which dominates
/// its peak, so only the staging step measured alone shows these copies.
#[test]
fn a_64_mib_packet_is_identified_without_holding_it_as_text() {
    let lines = 64 * 1024 * 1024 / LINE.len();
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(HEAD.as_bytes()).unwrap();
    let block = LINE.repeat(64 * 1024 / LINE.len());
    for _ in 0..lines / (64 * 1024 / LINE.len()) {
        encoder.write_all(block.as_bytes()).unwrap();
    }
    encoder.write_all(TAIL.as_bytes()).unwrap();
    let file = document("/Filter /FlateDecode", &encoder.finish().unwrap());
    let (got, growth) = identified(&file);
    let size = got.len();
    assert!(size > 60 * 1024 * 1024, "the whole packet is kept: {size}");
    assert!(got.ends_with(b"</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end='w'?>"));
    // The patched packet itself is held once; a copy of it is not.
    assert!(
        growth < size + 16 * 1024 * 1024,
        "identifying a {size} byte packet grew the thread by {growth} bytes"
    );
}

/// One page with a stream of `weight` incompressible bytes that the page holds
/// under a key of its own, so it is written with the page and read by nothing.
fn heavy_document(weight: usize) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let noise: Vec<u8> = (0..weight)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 56) as u8
        })
        .collect();
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Weight 4 0 R >>\nendobj\n".to_vec(),
        stream_object(4, "", &noise),
    ])
}

/// The conversion leaves what it changed staged in the editor, and holds
/// nothing the size of the file: the editor was neither saved nor reopened.
/// No Dart battery can observe this: the saved file is the same bytes whether
/// the conversion saved the document once on the way or only the save did, and
/// the memory rows measure the whole operation, not what the conversion alone
/// holds.
#[test]
fn the_conversion_stages_its_edits_without_saving_the_document() {
    let file = heavy_document(24 * 1024 * 1024);
    let mut editor = open_editor_from_bytes(&file, "pdfa_staged");
    let (done, growth) = thread_growth(|| edit_convert_to_pdf_a(&mut editor, 2, &CancelCheck::none()));
    done.unwrap();
    let Some(Object::Dictionary(catalog)) = editor.get_modified(1) else {
        panic!("the catalog is staged, not committed")
    };
    assert!(catalog.contains_key("Metadata"), "the identification is staged");
    assert!(catalog.contains_key("OutputIntents"), "the output intent is staged");
    assert!(editor.is_modified());
    assert!(
        growth < 8 * 1024 * 1024,
        "converting a {} byte file grew the thread by {growth} bytes",
        file.len()
    );
}

/// A converted document, saved and opened again, is PDF/A. The conversion
/// itself checks nothing at its end: this is where the output is checked.
/// No Dart battery can observe this: the Dart side has no validator, and the
/// result of a conversion is its file.
#[test]
fn a_converted_file_saved_and_opened_again_validates() {
    let mut editor = open_editor_from_bytes(&heavy_document(64 * 1024), "pdfa_staged_valid");
    edit_convert_to_pdf_a(&mut editor, 2, &CancelCheck::none()).unwrap();
    let saved = editor.save_to_bytes().unwrap();
    let mut reopened = PdfDocument::from_bytes(saved).unwrap();
    let validation = validate_pdf_a(&mut reopened, PdfALevel::A2b).unwrap();
    assert!(validation.is_compliant, "{:?}", validation.errors);
}
