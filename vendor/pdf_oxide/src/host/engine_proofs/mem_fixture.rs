//! One document, built in code, that every memory site of t-441 reads: an
//! XMP packet of 64 MiB once decoded, an article thread of 6,000 beads
//! (more than the old cap of 4,096), a ToUnicode CMap whose ranges are
//! hostile, and a file the public incremental save appends to.

use super::filter_readers::{HELD, PEAK};
use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EditableDocument, SaveOptions};
use crate::extractors::xmp::XmpExtractor;
use crate::fonts::cmap::parse_tounicode_cmap;
use crate::structure::article_threads::parse_article_threads;
use std::io::Write;
use std::sync::atomic::Ordering;

const PACKET: usize = 64 * 1024 * 1024;
const BEADS: usize = 6000;

/// The packet, Flate-coded block by block as it is generated: 64 MiB of
/// properties that all name one key, so the properties it yields stay small.
fn xmp_stream() -> Vec<u8> {
    let head = "<?xpacket begin='x' id='W5M0MpCehiHzreSzNTczkc9d'?>\n<x:xmpmeta xmlns:x='adobe:ns:meta/'>\n\
                <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
                <rdf:Description xmlns:pdf='http://ns.adobe.com/pdf/1.3/'>\n";
    let tail = "</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end='w'?>";
    let line = "<pdf:Producer>probe</pdf:Producer>\n";
    let block = line.repeat(64 * 1024 / line.len());
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(head.as_bytes()).unwrap();
    for _ in 0..PACKET / block.len() {
        encoder.write_all(block.as_bytes()).unwrap();
    }
    encoder.write_all(tail.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

fn fixture() -> Vec<u8> {
    let first = 8usize;
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Metadata 4 0 R /Threads [5 0 R] >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        stream_object(4, "/Type /Metadata /Subtype /XML /Filter /FlateDecode", &xmp_stream()),
        format!("5 0 obj\n<< /Type /Thread /F {first} 0 R >>\nendobj\n").into_bytes(),
        stream_object(
            6,
            "",
            b"begincmap\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
              2 beginbfrange\n<00000000> <FFFFFFFF> [<0041>]\n<0010> <0001> [<0042>]\nendbfrange\nendcmap\n",
        ),
        b"7 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 6 0 R >>\nendobj\n".to_vec(),
    ];
    for i in 0..BEADS {
        let id = first + i;
        let next = first + (i + 1) % BEADS;
        let prev = first + (i + BEADS - 1) % BEADS;
        objects.push(
            format!(
                "{id} 0 obj\n<< /Type /Bead /T 5 0 R /N {next} 0 R /V {prev} 0 R /P 3 0 R /R [0 0 10 10] >>\nendobj\n"
            )
            .into_bytes(),
        );
    }
    pdf_from_objects(&objects)
}

/// The sites that read a document through its own readers do so on one
/// 64 MiB fixture: the packet is read as events with the process growing by
/// a few MiB, not by the packet; the thread is read to its 6,000th bead; the
/// hostile CMap ranges are refused without an overflow; the public
/// incremental save appends to the file by streaming it. No Dart battery
/// can observe this: the allocator counter, the cap and the overflow are
/// the engine's own, and a Dart read sees the same properties either way.
#[test]
fn one_64_mib_fixture_passes_every_site() {
    let bytes = fixture();
    let doc = PdfDocument::from_bytes(bytes.clone()).unwrap();

    let base = HELD.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let xmp = XmpExtractor::extract(&doc).unwrap().expect("a packet");
    let growth = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    assert_eq!(xmp.pdf_producer.as_deref(), Some("probe"));
    assert!(xmp.raw_xml.is_none());
    println!("64 MiB XMP packet: peak growth {growth} bytes");
    // The counter is the process's, shared with every test running beside this one; a
    // whole read holds 64 MiB or more, so half of that separates the two.
    assert!(growth < 32 * 1024 * 1024, "reading a 64 MiB packet grew the process by {growth} bytes");

    let threads = parse_article_threads(&doc);
    assert_eq!(threads[0].beads.len(), BEADS);

    let cmap = doc.load_object(crate::object::ObjectRef::new(6, 0)).unwrap();
    let data = cmap.decode_stream_data().unwrap();
    let parsed = parse_tounicode_cmap(&data).expect("hostile ranges are not an error");
    assert_eq!(parsed.get(&0).as_deref(), Some("A"));

    let dir = std::env::temp_dir();
    let src = dir.join(format!("pdf_oxide_mem_fixture_src_{}.pdf", std::process::id()));
    let out = dir.join(format!("pdf_oxide_mem_fixture_out_{}.pdf", std::process::id()));
    std::fs::write(&src, &bytes).unwrap();
    let mut editor = DocumentEditor::open(&src).unwrap();
    editor.set_title("Fixture");
    let saved = editor.save_with_options(&out, SaveOptions::incremental());
    let written = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&out);
    saved.unwrap();
    assert_eq!(&written[..bytes.len()], &bytes[..]);
    assert!(!PdfDocument::from_bytes(written).unwrap().xref_repaired());
}
