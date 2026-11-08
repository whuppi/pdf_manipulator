//! Bridge API — the door: parse the request and dispatch through the registry.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! ## Executor model
//!
//! Engine state lives per executor (`EngineState`) — one thread on native,
//! one Web Worker on web. This file never boots, routes, or terminates
//! executors; it is the decision-free request door that
//! every executor body calls:
//!
//!   Native executor body: `host/native/executor.rs` (mailbox thread)
//!   Web executor body:    `web_assets/executor_worker.js` (Worker + wasm_entry)
//!
//! ## Request routing
//!
//! `handle_request` parses binary request bytes, looks the op up in the
//! registry (`host/ops/registry.rs`), runs that op unit's handler against
//! the executor's own state, and returns the binary response bytes. Each op
//! unit under `host/ops/` owns its body. Same source code for both
//! targets. 100% platform-agnostic. No locks — an executor's state has
//! exactly one owner thread, so handlers take `&mut EngineState`.

use crate::host::binary_codec::{Request, ResponseWriter};
use crate::host::engine_state::EngineState;

// ═══════════════════════════════════════════════════════════════════
// BoxedReader / BoxedWriter — concrete wrappers for trait objects
// ═══════════════════════════════════════════════════════════════════

/// Concrete wrapper around a boxed Read+Seek trait object.
pub(crate) struct BoxedReader(pub(crate) Box<dyn crate::document::ReadSeek>);

impl std::io::Read for BoxedReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.0.read(buf) }
}

impl std::io::Seek for BoxedReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> { self.0.seek(pos) }
}

thread_local! {
    /// The message of the first read the host's source failed in the op
    /// running on this thread. An executor runs one op at a time, on one
    /// thread, and every read of a source happens on it.
    static SOURCE_FAILURE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Records that the host's source failed a read, and returns the error the
/// reader answers with. The first failure of an op is the one that is kept.
pub(crate) fn fail_source(message: &str) -> std::io::Error {
    SOURCE_FAILURE.with(|failure| {
        failure.borrow_mut().get_or_insert_with(|| message.to_string());
    });
    std::io::Error::other(message)
}

/// The error of the op's first failed source read, if one failed: a reader
/// answers every read after it with this, and never asks the host again.
pub(crate) fn source_failure() -> Option<std::io::Error> {
    SOURCE_FAILURE.with(|failure| failure.borrow().as_deref().map(std::io::Error::other))
}

fn take_source_failure() -> Option<String> {
    SOURCE_FAILURE.with(|failure| failure.borrow_mut().take())
}

/// A sink writer that buffers: it holds bytes back until `flush`, and
/// `discard` drops what it holds. Neither writes on drop.
pub(crate) trait BufferedSink: std::io::Write + Send {
    fn discard(&mut self);
}

enum SinkWriter {
    /// A writer that holds nothing back: the proofs' and the probe's sinks.
    #[cfg(any(test, feature = "test-support"))]
    Plain(Box<dyn WriteSend>),
    Buffered(Box<dyn BufferedSink>),
}

struct SinkCore {
    writer: SinkWriter,
    written: u64,
}

/// Concrete wrapper around a boxed Write+Send trait object.
///
/// The handle an op takes and the handle the request door keeps share one
/// writer: the op's end drops its handle, the door's flush or discard is the
/// last word on what the writer holds.
pub(crate) struct BoxedWriter {
    core: std::sync::Arc<std::sync::Mutex<SinkCore>>,
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) trait WriteSend: std::io::Write + Send {}
#[cfg(any(test, feature = "test-support"))]
impl<T: std::io::Write + Send> WriteSend for T {}

impl BoxedWriter {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn new(w: impl std::io::Write + Send + 'static) -> Self {
        Self::of(SinkWriter::Plain(Box::new(w)))
    }

    pub(crate) fn buffered(w: impl BufferedSink + 'static) -> Self {
        Self::of(SinkWriter::Buffered(Box::new(w)))
    }

    fn of(writer: SinkWriter) -> Self {
        let core = SinkCore { writer, written: 0 };
        Self { core: std::sync::Arc::new(std::sync::Mutex::new(core)) }
    }

    /// Another handle to this writer.
    fn share(&self) -> Self {
        Self { core: self.core.clone() }
    }

    /// Drops the bytes the writer still holds.
    fn discard(&mut self) {
        match &mut self.core().writer {
            SinkWriter::Buffered(w) => w.discard(),
            #[cfg(any(test, feature = "test-support"))]
            SinkWriter::Plain(_) => {}
        }
    }

    fn core(&self) -> std::sync::MutexGuard<'_, SinkCore> {
        self.core.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl std::io::Write for BoxedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut core = self.core();
        let n = match &mut core.writer {
            #[cfg(any(test, feature = "test-support"))]
            SinkWriter::Plain(w) => w.write(buf)?,
            SinkWriter::Buffered(w) => w.write(buf)?,
        };
        core.written += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.core().writer {
            #[cfg(any(test, feature = "test-support"))]
            SinkWriter::Plain(w) => w.flush(),
            SinkWriter::Buffered(w) => w.flush(),
        }
    }
}

impl crate::host::positioned_write::PositionedWrite for BoxedWriter {
    fn position(&mut self) -> u64 {
        self.core().written
    }
}

/// The response for a sink that failed to take an op's last bytes: the one
/// the op itself answers for a failed write.
fn write_failure(error: std::io::Error) -> Vec<u8> {
    if error.to_string() == "operation cancelled" {
        ResponseWriter::cancelled()
    } else {
        ResponseWriter::error_of(&crate::Error::Io(error))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Core request handler — platform-agnostic
// ═══════════════════════════════════════════════════════════════════

/// Handle a binary request within an instance's context.
///
/// O(1)-memory I/O: when reader/writer are Some, PDF bytes flow
/// through them on demand — never fully buffered.
pub(crate) fn handle_request(
    state: &mut EngineState,
    bytes: &[u8],
    source_bytes: Option<&[u8]>,
    mut sources: Vec<BoxedReader>,
    mut sinks: Vec<BoxedWriter>,
    cancel: &crate::cancel::CancelCheck,
) -> Vec<u8> {
    debug_assert_eq!(
        state.caches().owner(),
        std::thread::current().id(),
        "a lane's state is used on the thread that made it"
    );
    let _job = crate::cancel::enter(cancel);
    take_source_failure();
    let req = match Request::parse(bytes) {
        Ok(r) => r,
        Err(e) => return ResponseWriter::error(&format!("parse error: {e}")),
    };

    // The ops take their sinks and drop them; these handles keep every
    // writer alive to the end, where its last bytes are written or dropped.
    let mut doors: Vec<BoxedWriter> = sinks.iter().map(BoxedWriter::share).collect();

    let documents_before = state.documents.keys().max().copied();
    let editors_before = state.editors.keys().max().copied();
    let builders_before = state.builders.keys().max().copied();

    let mut ctx = crate::host::ops::OpCtx {
        state,
        req: &req,
        source_bytes,
        sources: &mut sources,
        sinks: &mut sinks,
        cancel,
    };
    let mut response = match crate::host::ops::registry::find(req.op()) {
        Some(entry) => (entry.handle)(&mut ctx),
        None => ResponseWriter::error(&format!("unknown op: {}", req.op())),
    };
    // A source that failed a read ends the op, whatever the engine made of
    // the error on its way up: a parser that took it for damage, a loop that
    // skipped the object it was reading, a rebuild that read on. A cancel
    // stays a cancel, and a handle the op made is dropped with the op.
    if let Some(message) = take_source_failure() {
        if response != ResponseWriter::cancelled() {
            response = ResponseWriter::error_of(&crate::Error::Io(std::io::Error::other(message)));
            ctx.state.documents.retain(|id, _| Some(*id) <= documents_before);
            ctx.state.editors.retain(|id, _| Some(*id) <= editors_before);
            ctx.state.builders.retain(|id, _| Some(*id) <= builders_before);
        }
    }
    // The op's last bytes are written here, by an explicit flush, never by a
    // writer's drop: a drop cannot report that the write failed. An op that
    // ended cancelled or failed holds no partial output, so its sinks write
    // nothing more.
    if ResponseWriter::is_ok(&response) {
        for door in &mut doors {
            if let Err(error) = std::io::Write::flush(door) {
                response = write_failure(error);
                break;
            }
        }
    }
    if !ResponseWriter::is_ok(&response) {
        for door in &mut doors {
            door.discard();
        }
    }
    // An op ends here: no open document keeps the object stream it decoded.
    for document in ctx.state.documents.values() {
        document.release_objstm_memo();
    }
    for editor in ctx.state.editors.values() {
        editor.release_objstm_memo();
    }
    response
}

// ═══════════════════════════════════════════════════════════════════
// WASM executor body — one Worker = one executor = one EngineState
// ═══════════════════════════════════════════════════════════════════
//
// The web twin of `host/native/executor.rs`. A Web Worker IS an executor: the
// browser provides the isolated thread; this module provides the
// executor's owned state and the call into the request door.
// Decision-free by design — routing, pinning, queuing, and cancel
// bookkeeping all live in the Dart Router.
//
// Terminating an executor on web is `worker.terminate()` — the browser frees the
// whole WASM heap with the worker, so `engine_state_drop` exists only for
// the graceful path (executor reuse without worker restart).

#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use super::*;
    use crate::host::wasm::js_reader::JsCallbackReader;
    use crate::host::wasm::js_writer::JsCallbackWriter;
    use wasm_bindgen::prelude::*;

    /// Create this worker's engine state. Returns a WASM pointer.
    /// Called exactly once per worker, from executor_worker.js bootstrap.
    #[wasm_bindgen]
    pub fn engine_state_new() -> u32 {
        let state = Box::new(EngineState::new());
        crate::host::heap::reset_peak();
        Box::into_raw(state) as u32
    }

    /// The most bytes this worker's heap has held since its last
    /// `engine_state_new`, which a reset calls again.
    #[wasm_bindgen]
    pub fn heap_peak() -> u32 {
        crate::host::heap::peak() as u32
    }

    /// Allocate `size` bytes (align 1) from the module's global allocator.
    ///
    /// Stable twin of the glue's internal malloc, for executor_worker.js's JSPI
    /// driver: wasm-bindgen's `__wbindgen_export_N` aliases are renumbered
    /// whenever the module's export set changes, so the worker must never
    /// call those by name. Same allocator and layout as the glue's malloc,
    /// so buffers are interchangeable with glue-allocated ones.
    #[wasm_bindgen]
    pub fn executor_alloc(size: u32) -> u32 {
        if size == 0 {
            return 1; // non-null dangling, mirrors the glue's zero-size malloc
        }
        unsafe {
            let layout =
                std::alloc::Layout::from_size_align_unchecked(size as usize, 1);
            std::alloc::alloc(layout) as u32
        }
    }

    /// Free a buffer from `executor_alloc` or a response buffer handed out by
    /// `execute` (both come from the same global allocator, align 1).
    #[wasm_bindgen]
    pub fn executor_dealloc(ptr: u32, size: u32) {
        if size == 0 {
            return;
        }
        unsafe {
            let layout =
                std::alloc::Layout::from_size_align_unchecked(size as usize, 1);
            std::alloc::dealloc(ptr as *mut u8, layout);
        }
    }

    /// Execute one job against this executor's state.
    ///
    /// source_lengths: packed f64 array — each 8 bytes is one source
    /// length (f64 because JS numbers cross the boundary as doubles).
    /// Sources get indexed readers (0, 1, 2, ...).
    /// sink_count: how many output sinks exist.
    /// Sinks get indexed writers (0, 1, 2, ...).
    ///
    /// The worker is single-threaded, so the &mut reconstruction from
    /// the raw pointer is sound: no other code can hold a reference
    /// while this runs.
    #[wasm_bindgen]
    pub fn execute(
        state_ptr: u32,
        request_bytes: &[u8],
        source_lengths: &[u8],
        sink_count: u32,
    ) -> Vec<u8> {
        let state = unsafe { &mut *(state_ptr as *mut EngineState) };

        let mut sources = Vec::new();
        let num_sources = source_lengths.len() / 8;
        for i in 0..num_sources {
            let offset = i * 8;
            let len = f64::from_le_bytes([
                source_lengths[offset], source_lengths[offset+1],
                source_lengths[offset+2], source_lengths[offset+3],
                source_lengths[offset+4], source_lengths[offset+5],
                source_lengths[offset+6], source_lengths[offset+7],
            ]);
            // An empty source stays in the list, as on native: dropping it
            // would shift every later source's index.
            sources.push(BoxedReader(Box::new(JsCallbackReader::new(i as u32, len as u64))));
        }

        let mut sinks = Vec::new();
        for i in 0..sink_count {
            sinks.push(BoxedWriter::buffered(JsCallbackWriter::new(i)));
        }

        handle_request(state, request_bytes, None, sources, sinks, &crate::cancel::CancelCheck::from_host())
    }

    /// Drop this executor's engine state, freeing every handle it owns.
    /// Graceful-path only — `worker.terminate()` makes this moot.
    #[wasm_bindgen]
    pub fn engine_state_drop(state_ptr: u32) {
        if state_ptr == 0 { return; }
        unsafe {
            let _ = Box::from_raw(state_ptr as *mut EngineState);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// A request frame with integer fields, as the Dart codec lays it out.
    fn frame(op: &str, fields: &[(&str, i32)]) -> Vec<u8> {
        let mut out = vec![op.len() as u8];
        out.extend_from_slice(op.as_bytes());
        out.extend_from_slice(&(fields.len() as u16).to_le_bytes());
        for (key, value) in fields {
            out.push(key.len() as u8);
            out.extend_from_slice(key.as_bytes());
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// A sink that buffers as the host writers do: `write` always succeeds,
    /// the bytes are delivered only by `flush`, which fails with `refusal`
    /// when one is set.
    struct Buffered {
        pending: Vec<u8>,
        delivered: Arc<Mutex<Vec<u8>>>,
        flushes: Arc<AtomicUsize>,
        discards: Arc<AtomicUsize>,
        refusal: Option<&'static str>,
    }

    impl BufferedSink for Buffered {
        fn discard(&mut self) {
            self.discards.fetch_add(1, Ordering::SeqCst);
            self.pending.clear();
        }
    }

    impl Write for Buffered {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.pending.extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes.fetch_add(1, Ordering::SeqCst);
            if let Some(message) = self.refusal {
                return Err(std::io::Error::other(message));
            }
            self.delivered.lock().unwrap().append(&mut self.pending);
            Ok(())
        }
    }

    struct Probe {
        delivered: Arc<Mutex<Vec<u8>>>,
        flushes: Arc<AtomicUsize>,
        discards: Arc<AtomicUsize>,
    }

    fn buffered(refusal: Option<&'static str>) -> (BoxedWriter, Probe) {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        let flushes = Arc::new(AtomicUsize::new(0));
        let discards = Arc::new(AtomicUsize::new(0));
        let sink = Buffered {
            pending: Vec::new(),
            delivered: delivered.clone(),
            flushes: flushes.clone(),
            discards: discards.clone(),
            refusal,
        };
        (BoxedWriter::buffered(sink), Probe { delivered, flushes, discards })
    }

    /// `builderSave` of a fresh builder into `sink`.
    fn save_into(sink: BoxedWriter, handle: Option<i32>) -> Vec<u8> {
        let mut state = EngineState::new();
        let created = handle_request(
            &mut state,
            &frame("builderCreate", &[]),
            None,
            Vec::new(),
            Vec::new(),
            &crate::cancel::CancelCheck::none(),
        );
        assert_eq!(created[0], 1, "the builder is created");
        let handle = handle.unwrap_or_else(|| {
            let mut as_request = vec![0u8];
            as_request.extend_from_slice(&created[1..]);
            Request::parse(&as_request).unwrap().get_i32("handleId").unwrap()
        });
        handle_request(
            &mut state,
            &frame("builderSave", &[("handleId", handle)]),
            None,
            Vec::new(),
            vec![sink],
            &crate::cancel::CancelCheck::none(),
        )
    }

    #[test]
    fn an_ok_op_has_its_sinks_flushed() {
        let (sink, probe) = buffered(None);
        let answer = save_into(sink, None);
        assert_eq!(answer[0], 1, "the save is ok");
        assert_eq!(probe.flushes.load(Ordering::SeqCst), 1, "flushed once, by the door");
        assert!(probe.delivered.lock().unwrap().starts_with(b"%PDF-"), "the output arrived");
        assert_eq!(probe.discards.load(Ordering::SeqCst), 0, "nothing is dropped after an ok op");
    }

    #[test]
    fn a_failed_last_flush_fails_the_op() {
        let (sink, probe) = buffered(Some("host write failed"));
        let answer = save_into(sink, None);
        assert_eq!(answer[0], 0, "the response is an error, not ok");
        assert_eq!(answer[1], crate::host::binary_codec::ErrorKind::Io as u8);
        assert!(probe.delivered.lock().unwrap().is_empty());
        assert_eq!(probe.discards.load(Ordering::SeqCst), 1, "the refused bytes are dropped");
    }

    #[test]
    fn a_cancel_at_the_last_flush_cancels_the_op() {
        let (sink, _probe) = buffered(Some("operation cancelled"));
        let answer = save_into(sink, None);
        assert_eq!(answer, ResponseWriter::cancelled());
    }

    #[test]
    fn a_failed_op_does_not_flush_its_sinks() {
        let (sink, probe) = buffered(None);
        let answer = save_into(sink, Some(9999));
        assert_eq!(answer[0], 0, "no such builder");
        assert_eq!(probe.flushes.load(Ordering::SeqCst), 0);
        assert!(probe.delivered.lock().unwrap().is_empty());
        assert_eq!(probe.discards.load(Ordering::SeqCst), 1, "a failed op's sink is discarded");
    }
}
