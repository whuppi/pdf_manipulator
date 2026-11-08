//! A compress asks its source for bytes in few round trips.
//!
//! On the native bridge every read the reader cannot answer from the last
//! chunk is a round trip to the Dart isolate: the lane thread wakes it, waits
//! on a condition variable, and resumes when the bytes are in the shared
//! buffer. A source read in many small pieces spends its time in those
//! waits, not in the engine.

use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::host::images::execute::reduce_images;
use crate::host::images::policy::{Chroma, Policy};
use crate::host::native::cancel::CancelFlags;
use crate::host::native::callback_reader::CallbackReader;
use crate::host::native::shared_buffer::{self as sb, read_channel as rc};
use crate::host::positioned_write::CountingWriter;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::OnceLock;

const PAGES: usize = 70;
const IMAGE_BYTES: usize = 30_000;

static TRIPS: AtomicUsize = AtomicUsize::new(0);
static CHANNEL: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static SOURCE: OnceLock<Vec<u8>> = OnceLock::new();

/// The host side of a read, answered on the calling thread: counts the trip
/// and fills the shared buffer the way the Dart handler does.
unsafe extern "C" fn answer_read() {
    TRIPS.fetch_add(1, Ordering::Relaxed);
    let channel = CHANNEL.load(Ordering::Relaxed);
    let source = SOURCE.get().unwrap();
    let offset = sb::read_i64(channel, rc::OFFSET_REQUEST_OFFSET) as usize;
    let count = sb::read_i64(channel, rc::OFFSET_REQUEST_COUNT) as usize;
    let n = count.min(source.len() - offset);
    std::ptr::copy_nonoverlapping(
        source[offset..].as_ptr(),
        sb::data_ptr(channel, rc::OFFSET_DATA),
        n,
    );
    sb::write_i64(channel, rc::OFFSET_RESPONSE_LENGTH, n as i64);
    sb::host_respond(channel, rc::OFFSET_REQUEST_SEQ, rc::OFFSET_RESPONSE, sb::FLAG_READY);
}

/// A scan-like file: one 8-bit gray image of `IMAGE_BYTES` per page, stored
/// uncompressed, so a lossless compress has work on every page.
fn scan_like_pdf() -> Vec<u8> {
    let kids: String = (0..PAGES).map(|i| format!("{} 0 R ", 3 + 3 * i)).collect();
    let mut objs = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {PAGES} >>\nendobj\n").into_bytes(),
    ];
    for i in 0..PAGES {
        let page = 3 + 3 * i as u32;
        objs.push(
            format!(
                "{page} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 300] \
                 /Contents {} 0 R /Resources << /XObject << /Im0 {} 0 R >> >> >>\nendobj\n",
                page + 1,
                page + 2
            )
            .into_bytes(),
        );
        objs.push(stream_object(page + 1, "", b"q 100 0 0 300 0 0 cm /Im0 Do Q"));
        let pixels: Vec<u8> = (0..IMAGE_BYTES).map(|p| ((p / 100) as u8).wrapping_add(i as u8)).collect();
        objs.push(stream_object(
            page + 2,
            "/Type /XObject /Subtype /Image /Width 100 /Height 300 /ColorSpace /DeviceGray /BitsPerComponent 8",
            &pixels,
        ));
    }
    pdf_from_objects(&objs)
}

/// No Dart battery sees it: it counts the round trips the native read channel makes, which a battery sees only as time.
#[test]
fn a_lossless_compress_reads_its_source_in_few_trips() {
    let source = SOURCE.get_or_init(scan_like_pdf);
    let mut channel = vec![0u8; rc::TOTAL_SIZE];
    unsafe { sb::init_sync(channel.as_mut_ptr(), rc::OFFSET_SYNC_PTR) };
    CHANNEL.store(channel.as_mut_ptr(), Ordering::Relaxed);
    let reader = unsafe {
        CallbackReader::new(channel.as_mut_ptr(), answer_read, source.len() as u64, CancelFlags::unconnected())
    };
    let doc = PdfDocument::from_external_reader(Box::new(reader)).unwrap();
    let mut editor = DocumentEditor::from_document(doc).unwrap();
    let policy = Policy {
        color_ppi: None,
        gray_ppi: None,
        mono_ppi: None,
        downsample_threshold: 1.5,
        jpeg_quality: 75,
        allow_lossy: false,
        convert_cmyk_to_rgb: false,
        min_pixels: 32,
        min_savings: 0.0,
        chroma: Chroma::Auto,
        recompress_jpeg: false,
    };
    reduce_images(&mut editor, &policy, &CancelCheck::none()).unwrap();
    let mut out = CountingWriter::new(Vec::new());
    crate::host::dispatch::edit_save_with_options(
        &mut editor,
        &mut out,
        &SaveOptions::full_rewrite(),
        &CancelCheck::none(),
    )
    .unwrap();
    drop(editor);
    unsafe { sb::destroy_sync(channel.as_mut_ptr(), rc::OFFSET_SYNC_PTR) };
    let trips = TRIPS.load(Ordering::Relaxed);
    // Two passes over a 2.1 MB source in 64 KiB chunks is 66 trips.
    assert!(trips <= 100, "{trips} round trips for {} bytes", out.into_inner().len());
}
