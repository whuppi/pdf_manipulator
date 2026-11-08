//! The document's XMP packet is read as XML events over its decoded stream.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::extractors::xmp::XmpExtractor;

fn packet(extra_properties: usize) -> Vec<u8> {
    let mut xml = String::from(
        "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n<x:xmpmeta xmlns:x='adobe:ns:meta/'>\n\
         <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
         <rdf:Description xmlns:dc='http://purl.org/dc/elements/1.1/' xmlns:pdf='http://ns.adobe.com/pdf/1.3/'>\n\
         <dc:title><rdf:Alt><rdf:li xml:lang='x-default'>Large packet</rdf:li></rdf:Alt></dc:title>\n",
    );
    for i in 0..extra_properties {
        xml.push_str(&format!("<pdf:Custom{i}>value {i}</pdf:Custom{i}>\n"));
    }
    xml.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end='w'?>");
    xml.into_bytes()
}

fn document(packet: &[u8]) -> PdfDocument {
    PdfDocument::from_bytes(pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Metadata 4 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        stream_object(4, "/Type /Metadata /Subtype /XML", packet),
    ]))
    .unwrap()
}

/// A packet of 20000 properties gives the same properties the string parser
/// gives, from the stream alone, and keeps no copy of the packet. The
/// whole-buffer lint (`tool/check_whole_buffers.sh`) is what fails when
/// `extract` goes back to decoding the stream whole. No Dart battery can
/// observe this: Dart reads the same title either way, and a whole copy of
/// the packet is held only for the length of the call.
#[test]
fn a_large_xmp_packet_is_read_as_events_and_not_kept() {
    let bytes = packet(20_000);
    let streamed = XmpExtractor::extract(&document(&bytes)).unwrap().expect("a packet");
    let from_text = XmpExtractor::parse_xmp(std::str::from_utf8(&bytes).unwrap()).unwrap().unwrap();
    assert_eq!(streamed.dc_title.as_deref(), Some("Large packet"));
    assert_eq!(streamed.dc_title, from_text.dc_title);
    assert_eq!(streamed.custom, from_text.custom);
    assert_eq!(streamed.custom.len(), 20_000);
    assert!(streamed.raw_xml.is_none(), "the packet must not be held whole");
    assert!(from_text.raw_xml.is_some());
}

/// A stream with no XMP root is `None`, as the string parser's missing
/// markers were.
#[test]
fn a_metadata_stream_without_a_packet_is_none() {
    assert!(XmpExtractor::extract(&document(b"<html>not xmp</html>")).unwrap().is_none());
}
