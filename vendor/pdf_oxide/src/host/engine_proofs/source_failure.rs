//! A source that fails is an I/O error, whatever the engine was doing.
//!
//! No Dart battery sees these: they need a read that fails at an exact point
//! of the open, and a count of the reads the engine makes after it, which the
//! host's source never reports back.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::error::Error;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A document larger than one read buffer, so its xref sits in a read of its own.
fn padded_pdf() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec(),
        stream_object(4, "", &vec![b'x'; 200_000]),
    ])
}

/// Reads a document's bytes and fails once, on the first read that starts at
/// or past `from`; `after_failure` counts the reads the engine makes after it.
struct FailsOnce {
    inner: Cursor<Vec<u8>>,
    from: u64,
    failed: bool,
    after_failure: Arc<AtomicUsize>,
}

impl FailsOnce {
    fn new(bytes: Vec<u8>, from: u64) -> (Self, Arc<AtomicUsize>) {
        let after_failure = Arc::new(AtomicUsize::new(0));
        let reader = Self { inner: Cursor::new(bytes), from, failed: false, after_failure: after_failure.clone() };
        (reader, after_failure)
    }
}

impl Read for FailsOnce {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            self.after_failure.fetch_add(1, Ordering::SeqCst);
        } else if self.inner.position() >= self.from {
            self.failed = true;
            return Err(io::Error::other("host read failed"));
        }
        self.inner.read(buf)
    }
}

impl Seek for FailsOnce {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// No Dart battery sees it: it fails the one read at byte 0 and the host's source answers every later read.
#[test]
fn a_failed_read_of_the_header_is_an_io_error_not_a_bad_header() {
    let (reader, _) = FailsOnce::new(padded_pdf(), 0);
    let error = PdfDocument::from_external_reader(Box::new(reader)).err().expect("the open fails");
    assert!(matches!(error, Error::Io(_)), "a source error is an I/O error, got {error:?}");
}

/// No Dart battery sees it: it counts the reads the engine makes after the failed one, which the host's source never reports.
#[test]
fn a_failed_read_of_the_xref_ends_the_open_without_a_rebuild() {
    let bytes = padded_pdf();
    let xref_at = bytes.windows(4).rposition(|w| w == b"xref").unwrap() as u64;
    let (reader, after_failure) = FailsOnce::new(bytes, xref_at);
    let error = PdfDocument::from_external_reader(Box::new(reader)).err().expect("the open fails");
    assert!(matches!(error, Error::Io(_)), "a source error is an I/O error, got {error:?}");
    assert_eq!(
        after_failure.load(Ordering::SeqCst),
        0,
        "the engine asked the source again after it failed"
    );
}

#[cfg(feature = "native-bridge")]
mod bridge {
    use super::*;
    use crate::cancel::CancelCheck;
    use crate::host::binary_codec::ErrorKind;
    use crate::host::bridge_api::{handle_request, BoxedReader};
    use crate::host::engine_state::EngineState;
    use crate::host::native::callback_reader::CallbackReader;
    use crate::host::native::cancel::CancelFlags;
    use crate::host::native::shared_buffer::{self as sb, read_channel as rc};
    use std::sync::atomic::AtomicPtr;
    use std::sync::OnceLock;

    static TRIPS: AtomicUsize = AtomicUsize::new(0);
    static FAIL_TRIP: AtomicUsize = AtomicUsize::new(usize::MAX);
    static CHANNEL: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
    static SOURCE: OnceLock<Vec<u8>> = OnceLock::new();

    /// The host side of a read: the trip numbered `FAIL_TRIP` answers with an
    /// error, every other one with the bytes.
    unsafe extern "C" fn answer_read() {
        let trip = TRIPS.fetch_add(1, Ordering::SeqCst) + 1;
        let channel = CHANNEL.load(Ordering::SeqCst);
        if trip == FAIL_TRIP.load(Ordering::SeqCst) {
            sb::host_respond(channel, rc::OFFSET_REQUEST_SEQ, rc::OFFSET_RESPONSE, sb::FLAG_ERROR);
            return;
        }
        let source = SOURCE.get().unwrap();
        let offset = sb::read_i64(channel, rc::OFFSET_REQUEST_OFFSET) as usize;
        let count = sb::read_i64(channel, rc::OFFSET_REQUEST_COUNT) as usize;
        let n = count.min(source.len() - offset);
        std::ptr::copy_nonoverlapping(source[offset..].as_ptr(), sb::data_ptr(channel, rc::OFFSET_DATA), n);
        sb::write_i64(channel, rc::OFFSET_RESPONSE_LENGTH, n as i64);
        sb::host_respond(channel, rc::OFFSET_REQUEST_SEQ, rc::OFFSET_RESPONSE, sb::FLAG_READY);
    }

    /// `open` of the source, its reads answered by [`answer_read`]; the
    /// response and the number of documents the state holds after it.
    fn open(state: &mut EngineState, fail_trip: usize) -> (Vec<u8>, usize) {
        let source = SOURCE.get_or_init(padded_pdf);
        let mut channel = vec![0u8; rc::TOTAL_SIZE];
        unsafe { sb::init_sync(channel.as_mut_ptr(), rc::OFFSET_SYNC_PTR) };
        CHANNEL.store(channel.as_mut_ptr(), Ordering::SeqCst);
        TRIPS.store(0, Ordering::SeqCst);
        FAIL_TRIP.store(fail_trip, Ordering::SeqCst);
        let reader = unsafe {
            CallbackReader::new(channel.as_mut_ptr(), answer_read, source.len() as u64, CancelFlags::unconnected())
        };
        let mut request = vec![4u8];
        request.extend_from_slice(b"open");
        request.extend_from_slice(&0u16.to_le_bytes());
        let response = handle_request(
            state,
            &request,
            None,
            vec![BoxedReader(Box::new(reader))],
            Vec::new(),
            &CancelCheck::none(),
        );
        unsafe { sb::destroy_sync(channel.as_mut_ptr(), rc::OFFSET_SYNC_PTR) };
        (response, state.documents.len())
    }

    /// No Dart battery sees it: it needs the one trip to the host that fails
    /// to be every trip of the open in turn, and a count of the documents the lane keeps.
    #[test]
    fn a_trip_that_fails_once_ends_the_open_with_an_io_error_and_keeps_no_document() {
        let mut state = EngineState::new();
        let (clean, held) = open(&mut state, usize::MAX);
        assert_eq!(clean[0], 1, "the open succeeds on a sound source");
        assert_eq!(held, 1);
        let trips = TRIPS.load(Ordering::SeqCst);
        assert!(trips >= 2, "the file must take more than one trip, took {trips}");

        for trip in 1..=trips {
            let mut state = EngineState::new();
            let (response, held) = open(&mut state, trip);
            assert_eq!(response[0], 0, "trip {trip} of {trips} failed and the open succeeded");
            assert_eq!(response[1], ErrorKind::Io as u8, "trip {trip} of {trips}: a failed source is not {}", response[1]);
            assert_eq!(held, 0, "trip {trip} of {trips}: a failed open keeps no document");
        }

        let (again, _) = open(&mut state, usize::MAX);
        assert_eq!(again[0], 1, "the next op on the lane starts clean");
    }
}
