//! Cutting font programs down to the glyphs shown reads the pages once.
//!
//! A full rewrite that prunes resources walks the content of every kept
//! page to find the names it uses, and then the objects of the document once
//! to find what else points at the dictionaries it cut. The glyph sets the
//! font cut needs come from those two passes: no content and no object is
//! opened for them a second time. Every
//! source read that the reader cannot answer from its last 64 KiB chunk is a
//! host round trip (a thread wake on native, a suspension on the web), so
//! a second walk over the pages is a cost, not a detail.
//!
//! No Dart battery can observe this exactly: a battery counts all the reads
//! of a whole compress and has a limit loose enough for the file's own
//! structure, and cannot tell a font cut from the rest of the save.

use super::font_subset::{truetype, GLYPHS, PLAIN};
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::host::constants::READ_BUF_CAPACITY;
use crate::host::positioned_write::CountingWriter;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const PAGES: usize = 400;

/// A source that answers a read from the last chunk it fetched and counts
/// a fetch otherwise, as the native bridge's reader does.
struct Chunked {
    bytes: Vec<u8>,
    position: u64,
    window: (u64, usize),
    trips: Arc<AtomicUsize>,
}

impl Read for Chunked {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.bytes.len() as u64 || out.is_empty() {
            return Ok(0);
        }
        let (start, len) = self.window;
        if self.position < start || self.position >= start + len as u64 {
            let at = self.position as usize;
            let got = (self.bytes.len() - at).min(READ_BUF_CAPACITY);
            self.window = (self.position, got);
            self.trips.fetch_add(1, Ordering::Relaxed);
        }
        let (start, len) = self.window;
        let at = (self.position - start) as usize;
        let n = out.len().min(len - at);
        let from = self.position as usize;
        out[..n].copy_from_slice(&self.bytes[from..from + n]);
        self.position += n as u64;
        Ok(n)
    }
}

impl Seek for Chunked {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let end = self.bytes.len() as i64;
        let target = match to {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(d) => end + d,
            SeekFrom::Current(d) => self.position as i64 + d,
        };
        if target < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"));
        }
        self.position = target as u64;
        Ok(self.position)
    }
}

/// `PAGES` text pages, each page dictionary followed by its content. Every
/// page shows glyphs of one embedded TrueType font and holds its resources
/// directly. When `embedded` is false the font has no program, so the same
/// save has nothing to cut.
fn long_pdf(embedded: bool) -> Vec<u8> {
    let kids: String = (0..PAGES).map(|i| format!("{} 0 R ", 6 + 2 * i)).collect();
    let program = truetype(PLAIN);
    let file = if embedded { "/FontFile2 5 0 R" } else { "" };
    let mut objs = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {PAGES} >>\nendobj\n").into_bytes(),
        format!(
            "3 0 obj\n<< /Type /Font /Subtype /TrueType /BaseFont /TestFont /FirstChar 65 \
             /LastChar {} /Widths [{}] /Encoding /WinAnsiEncoding /FontDescriptor 4 0 R >>\nendobj\n",
            65 + GLYPHS - 2,
            vec!["600"; GLYPHS - 1].join(" ")
        )
        .into_bytes(),
        format!(
            "4 0 obj\n<< /Type /FontDescriptor /FontName /TestFont /Flags 32 \
             /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 800 /Descent -200 \
             /CapHeight 700 /StemV 80 {file} >>\nendobj\n"
        )
        .into_bytes(),
        stream_object(5, &format!("/Length1 {}", program.len()), &program),
    ];
    for i in 0..PAGES {
        let page = 6 + 2 * i as u32;
        objs.push(
            format!(
                "{page} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] \
                 /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>\nendobj\n",
                page + 1
            )
            .into_bytes(),
        );
        let mut content = String::new();
        for line in 0..24 {
            let glyphs: String = (0..8).map(|k| (b'A' + ((i + line + k) % 12) as u8) as char).collect();
            content.push_str(&format!("BT /F1 10 Tf 10 {} Td ({glyphs}) Tj ET\n", 10 + line * 11));
        }
        objs.push(stream_object(page + 1, "", content.as_bytes()));
    }
    pdf_from_objects(&objs)
}

/// The host round trips of a pruning full rewrite of `source`.
fn trips_of_a_pruning_save(source: Vec<u8>) -> usize {
    let trips = Arc::new(AtomicUsize::new(0));
    let reader = Chunked { bytes: source, position: 0, window: (0, 0), trips: trips.clone() };
    let doc = PdfDocument::from_external_reader(Box::new(reader)).unwrap();
    let mut editor = DocumentEditor::from_document(doc).unwrap();
    let mut out = CountingWriter::new(Vec::new());
    crate::host::dispatch::edit_save_with_options(
        &mut editor,
        &mut out,
        &SaveOptions { prune_resources: true, ..SaveOptions::full_rewrite() },
        &CancelCheck::none(),
    )
    .unwrap();
    drop(editor);
    trips.load(Ordering::Relaxed)
}

/// Proves that cutting an embedded font down to the glyphs shown opens no
/// page content and no object a second time: a file whose font is embedded
/// (and is cut, as its output shows) makes at most two round trips more than
/// the same file without the program (the one that fetches the program
/// itself, and one of slack). No Dart-level test can observe it: a battery
/// counts the trips of a whole compress against a limit that the file's own
/// structure sets, so a second walk over the pages passes or fails it by
/// the size of the file, not by what it proves.
#[test]
fn font_cut_reads_no_page_twice() {
    let embedded = long_pdf(true);
    let bare = long_pdf(false);
    let cut = DocumentEditor::from_bytes(embedded.clone())
        .unwrap()
        .save_to_bytes_with_options(SaveOptions { prune_resources: true, ..SaveOptions::full_rewrite() })
        .unwrap();
    let whole = DocumentEditor::from_bytes(embedded.clone())
        .unwrap()
        .save_to_bytes_with_options(SaveOptions::full_rewrite())
        .unwrap();
    assert!(cut.len() < whole.len(), "the font was not cut: {} bytes is not under {}", cut.len(), whole.len());

    let chunks = embedded.len().div_ceil(READ_BUF_CAPACITY);
    let with = trips_of_a_pruning_save(embedded);
    let without = trips_of_a_pruning_save(bare);
    assert!(
        with <= without + 2,
        "{with} round trips with the font cut, {without} without it, over {chunks} chunks"
    );
}
