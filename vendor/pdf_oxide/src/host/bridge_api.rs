//! Bridge API — the ONE entry point for both native FFI and WASM.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! ## Instance model
//!
//! Each `Pdf()` in Dart/JS creates one `InstanceState` via `bridge_init`.
//! The instance owns all handles (documents, editors, builders) and the
//! thread pool (native). Destroying the instance via `bridge_shutdown`
//! drops everything — all handles freed, all threads stopped.
//!
//! Multiple instances are fully isolated. Different pools, different
//! handle maps, different memory. Killing one doesn't touch the others.
//!
//! ## Entry points (cfg-gated, same logic)
//!
//!   Native: `bridge_init` → `*mut InstanceState`
//!           `bridge_execute(instance, request, ...)` → posts result via allo-isolate
//!           `bridge_shutdown(instance)` → drops everything
//!
//!   WASM:   `bridge_init` → `u32` (WASM pointer)
//!           `bridge_execute(instance, request, ...)` → returns `Vec<u8>`
//!           `bridge_shutdown(instance)` → drops everything
//!
//! ## Request routing
//!
//! `handle_request` parses binary request bytes, calls the matching
//! dispatch function, encodes the result as binary response bytes.
//! Same source code for both targets. 100% platform-agnostic.

use crate::host::binary_codec::{Request, ResponseWriter};
use crate::host::dispatch;

use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::writer::{DocumentBuilder, PageSize};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

// ═══════════════════════════════════════════════════════════════════
// InstanceState — all per-instance state in one struct.
// Created by bridge_init. Destroyed by bridge_shutdown.
// Passed as opaque pointer to every bridge_execute call.
// ═══════════════════════════════════════════════════════════════════

/// All state for one Pdf engine instance.
///
/// Each `Pdf()` on the Dart/JS side creates exactly one of these.
/// The instance is fully isolated — different instances can't see
/// each other's documents, editors, or builders.
///
/// `dispose()` on the Dart side calls `bridge_shutdown`, which drops
/// this struct. All handles, all memory, all threads — gone.
pub struct InstanceState {
    documents: Mutex<HashMap<u32, PdfDocument>>,
    editors: Mutex<HashMap<u32, DocumentEditor>>,
    builders: Mutex<HashMap<u32, DocumentBuilder>>,
    page_ops: Mutex<HashMap<u32, Vec<dispatch::PageOp>>>,
    next_handle: AtomicU32,
    /// Instance-wide cancellation flag. Set by bridge_shutdown.
    /// Readers/writers check this to bail early during teardown.
    pub cancel: AtomicBool,
}

impl InstanceState {
    fn new() -> Self {
        Self {
            documents: Mutex::new(HashMap::new()),
            editors: Mutex::new(HashMap::new()),
            builders: Mutex::new(HashMap::new()),
            page_ops: Mutex::new(HashMap::new()),
            next_handle: AtomicU32::new(1),
            cancel: AtomicBool::new(false),
        }
    }

    fn next_handle_id(&self) -> u32 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }
}

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

/// Concrete wrapper around a boxed Write+Seek+Send trait object.
pub(crate) struct BoxedWriter(pub(crate) Box<dyn WriteSeekTrait>);

pub(crate) trait WriteSeekTrait: std::io::Write + std::io::Seek + Send {}
impl<T: std::io::Write + std::io::Seek + Send> WriteSeekTrait for T {}

impl std::io::Write for BoxedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.0.write(buf) }
    fn flush(&mut self) -> std::io::Result<()> { self.0.flush() }
}

impl std::io::Seek for BoxedWriter {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> { self.0.seek(pos) }
}

impl crate::host::positioned_write::PositionedWrite for BoxedWriter {
    fn position(&mut self) -> u64 {
        use std::io::Seek;
        self.stream_position().unwrap_or(0)
    }
}

/// Read all bytes from a reader in 64KB chunks.
/// Used when the engine needs the full data (images, embedded files)
/// but the transport must stream it — never as one blob in the message.
fn read_all_from_reader(reader: &mut BoxedReader) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 { break; }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(buf)
}

// ═══════════════════════════════════════════════════════════════════
// Core request handler — platform-agnostic
// ═══════════════════════════════════════════════════════════════════

/// Handle a binary request within an instance's context.
///
/// O(1)-memory I/O: when reader/writer are Some, PDF bytes flow
/// through them on demand — never fully buffered.
pub(crate) fn handle_request(
    state: &InstanceState,
    bytes: &[u8],
    source_bytes: Option<&[u8]>,
    mut sources: Vec<BoxedReader>,
    mut sinks: Vec<BoxedWriter>,
) -> Vec<u8> {
    let req = match Request::parse(bytes) {
        Ok(r) => r,
        Err(e) => return ResponseWriter::error(&format!("parse error: {e}")),
    };

    // Helper: take a source/sink by index (removes from vec, takes ownership).
    fn take_source(sources: &mut Vec<BoxedReader>, idx: usize) -> Option<BoxedReader> {
        if idx < sources.len() { Some(sources.remove(idx)) } else { None }
    }
    fn take_sink(sinks: &mut Vec<BoxedWriter>, idx: usize) -> Option<BoxedWriter> {
        if idx < sinks.len() { Some(sinks.remove(idx)) } else { None }
    }

    // Ops that produce output via sinks[0].
    match req.op() {
        "editorSave" => return handle_editor_save(state, &req, take_sink(&mut sinks, 0)),
        "editorExtractPages" => return handle_editor_extract_pages(state, &req, take_sink(&mut sinks, 0)),
        "convertTo" => return handle_convert_to(state, &req, source_bytes, take_source(&mut sources, 0), take_sink(&mut sinks, 0)),
        "convertToPdf" => return handle_convert_to_pdf(&req, source_bytes, take_source(&mut sources, 0), take_sink(&mut sinks, 0)),
        "builderSave" => return handle_builder_save(state, &req, take_sink(&mut sinks, 0)),
        "render" => return handle_render_streamed(state, &req, take_sink(&mut sinks, 0)),
        "extractImages" => return handle_extract_images_streamed(state, &req, take_sink(&mut sinks, 0)),
        "sign" => return handle_sign(&req, take_source(&mut sources, 0), source_bytes, take_sink(&mut sinks, 0)),
        _ => {}
    }

    match req.op() {
        // ── Document handle lifecycle ──
        "open" => handle_open(state, &req, source_bytes, take_source(&mut sources, 0)),
        "docDispose" => {
            let hid = req.get_i32("handleId").unwrap_or(0) as u32;
            state.documents.lock().unwrap().remove(&hid);
            let mut w = ResponseWriter::ok();
            w.put_bool("disposed", true);
            w.finish()
        }

        // ── Document read ops (reuse already-parsed doc via handleId) ──
        "extract" => handle_with_doc(state, &req, |doc, req| {
            let page = req.get_i32("page").map(|p| p as usize);
            let format = req.get_str("format").unwrap_or("plainText");
            let result = dispatch::extract_text(doc, page, format)?;
            let mut w = ResponseWriter::ok();
            w.put_str("text", &result.text);
            Ok(w.finish())
        }),
        "search" => handle_with_doc(state, &req, |doc, req| {
            let query = req.get_str("query").unwrap_or("");
            let page = req.get_i32("page").map(|p| p as usize);
            let result = dispatch::search_text(doc, query, page)?;
            let mut w = ResponseWriter::ok();
            w.put_map_list("hits", result.hits.len(), |i, item| {
                let h = &result.hits[i];
                item.put_i32("page", h.page as i32);
                item.put_str("text", &h.text);
                item.put_f64("x", h.x as f64);
                item.put_f64("y", h.y as f64);
                item.put_f64("width", h.width as f64);
                item.put_f64("height", h.height as f64);
            });
            Ok(w.finish())
        }),
        "getSignatures" => handle_with_doc(state, &req, |doc, _| {
            let result = dispatch::get_signatures(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_map_list("signatures", result.signatures.len(), |i, item| {
                let s = &result.signatures[i];
                item.put_str("signerName", &s.signer_name);
                item.put_str("reason", &s.reason);
                item.put_str("location", &s.location);
            });
            Ok(w.finish())
        }),
        "verifySignatures" => handle_with_doc(state, &req, |doc, _| {
            let result = dispatch::verify_signatures(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_bool("valid", result);
            Ok(w.finish())
        }),
        "validatePdfA" => handle_with_doc(state, &req, |doc, req| {
            let level = req.get_i32("level").unwrap_or(2);
            let result = dispatch::validate_pdf_a(doc, level)?;
            let mut w = ResponseWriter::ok();
            w.put_bool("compliant", result.compliant);
            w.put_i32("errors", result.errors);
            w.put_i32("warnings", result.warnings);
            Ok(w.finish())
        }),
        "validatePdfUa" => handle_with_doc(state, &req, |doc, req| {
            let level = req.get_i32("level").unwrap_or(1);
            let result = dispatch::validate_pdf_ua(doc, level)?;
            let mut w = ResponseWriter::ok();
            w.put_bool("valid", result);
            Ok(w.finish())
        }),
        "planSplitByBookmarks" => handle_with_doc(state, &req, |doc, _| {
            let result = dispatch::plan_split_by_bookmarks(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_map_list("splits", result.len(), |i, item| {
                let s = &result[i];
                item.put_str("title", &s.title);
                item.put_i32("startPage", s.start_page as i32);
                item.put_i32("endPage", s.end_page as i32);
            });
            Ok(w.finish())
        }),
        "classifyPage" => handle_with_doc(state, &req, |doc, req| {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            let result = dispatch::classify_page(doc, page)?;
            let mut w = ResponseWriter::ok();
            w.put_str("type", &result.type_name);
            Ok(w.finish())
        }),
        "classifyDocument" => handle_with_doc(state, &req, |doc, _| {
            let result = dispatch::classify_document(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_str("type", &result.type_name);
            Ok(w.finish())
        }),

        // ── One-shot write ops ──
        "sign" => ResponseWriter::error("sign: use early-return path"),

        // ── Editor lifecycle ──
        "editorOpen" => handle_editor_open(state, &req, source_bytes, take_source(&mut sources, 0)),
        "editorDispose" => {
            let hid = req.get_i32("handleId").unwrap_or(0) as u32;
            state.editors.lock().unwrap().remove(&hid);
            let mut w = ResponseWriter::ok();
            w.put_bool("disposed", true);
            w.finish()
        }
        "editorGetMetadata" => handle_with_editor(state, &req, |editor, _| {
            let m = dispatch::edit_get_metadata(editor);
            let mut w = ResponseWriter::ok();
            w.put_i32("pageCount", m.page_count as i32);
            w.put_str("version", &format!("{}.{}", m.version_major, m.version_minor));
            w.put_str("title", &m.title);
            w.put_str("author", &m.author);
            w.put_str("subject", &m.subject);
            w.put_str("keywords", &m.keywords);
            Ok(w.finish())
        }),
        "editorIsModified" => handle_with_editor(state, &req, |editor, _| {
            let modified = dispatch::edit_is_modified(editor);
            let mut w = ResponseWriter::ok();
            w.put_bool("modified", modified);
            Ok(w.finish())
        }),
        "editorPageMediaBox" => handle_with_editor(state, &req, |editor, req| {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            let (x, y, w2, h) = dispatch::edit_page_media_box(editor, page)?;
            let mut w = ResponseWriter::ok();
            w.put_f64("x", x as f64);
            w.put_f64("y", y as f64);
            w.put_f64("width", w2 as f64);
            w.put_f64("height", h as f64);
            Ok(w.finish())
        }),
        "editorRedactionCount" => handle_with_editor(state, &req, |editor, req| {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            let count = dispatch::edit_redaction_count(editor, page)?;
            let mut w = ResponseWriter::ok();
            w.put_i32("count", count as i32);
            Ok(w.finish())
        }),
        // For pinned ops, sources[0] is the re-created PDF reader (unused by
        // the editor — it has its own internal reader). sources[1] is the
        // actual data (image, file). For non-pinned ops, sources is empty.
        "editorMutate" => {
            let data = if sources.len() > 1 {
                Some(sources.remove(1))
            } else if !sources.is_empty() {
                Some(sources.remove(0))
            } else {
                None
            };
            handle_editor_mutate(state, &req, data)
        }
        "editorMergeFrom" => {
            let hid = req.get_i32("handleId").unwrap_or(0) as u32;
            let mut editors = state.editors.lock().unwrap();
            let editor = match editors.get_mut(&hid) {
                Some(e) => e,
                None => return ResponseWriter::error("editor not found"),
            };
            // sources[1] = merge data (pinned ops), sources[0] = merge data (non-pinned)
            let merge_reader = if sources.len() > 1 {
                Some(sources.remove(1))
            } else if !sources.is_empty() {
                Some(sources.remove(0))
            } else {
                None
            };
            let result = if let Some(reader) = merge_reader {
                editor.merge_from_reader(reader.0)
            } else if let Some(other) = req.get_bytes("otherBytes") {
                editor.merge_from_bytes(other)
            } else {
                return ResponseWriter::error("editorMergeFrom: no source");
            };
            match result {
                Ok(_) => {
                    let mut w = ResponseWriter::ok();
                    w.put_bool("merged", true);
                    w.finish()
                }
                Err(e) => ResponseWriter::error(&e.to_string()),
            }
        }

        // ── Builder lifecycle ──
        "builderCreate" => {
            let builder = dispatch::builder_new();
            let hid = state.next_handle_id();
            state.builders.lock().unwrap().insert(hid, builder);
            let mut w = ResponseWriter::ok();
            w.put_i32("handleId", hid as i32);
            w.finish()
        }
        "builderDispose" => {
            let hid = req.get_i32("handleId").unwrap_or(0) as u32;
            state.builders.lock().unwrap().remove(&hid);
            state.page_ops.lock().unwrap().remove(&hid);
            let mut w = ResponseWriter::ok();
            w.put_bool("disposed", true);
            w.finish()
        }
        "builderSetMetadata" => {
            let hid = req.get_i32("handleId").unwrap_or(0) as u32;
            let mut builders = state.builders.lock().unwrap();
            if let Some(b) = builders.remove(&hid) {
                let mut b = b;
                if let Some(v) = req.get_str("title") { b = dispatch::builder_set_title(b, v); }
                if let Some(v) = req.get_str("author") { b = dispatch::builder_set_author(b, v); }
                if let Some(v) = req.get_str("subject") { b = dispatch::builder_set_subject(b, v); }
                if let Some(v) = req.get_str("keywords") { b = dispatch::builder_set_keywords(b, v); }
                builders.insert(hid, b);
                let mut w = ResponseWriter::ok();
                w.put_bool("set", true);
                w.finish()
            } else {
                ResponseWriter::error("builder not found")
            }
        }
        "builderAddPage" => {
            let hid = req.get_i32("handleId").unwrap_or(0) as u32;
            let page_type = req.get_str("pageType");
            let width = req.get_f64("width");
            let height = req.get_f64("height");
            let size = match page_type {
                Some("a4") => PageSize::A4,
                Some("letter") => PageSize::Letter,
                _ => PageSize::Custom(
                    width.unwrap_or(595.0) as f32,
                    height.unwrap_or(842.0) as f32,
                ),
            };
            let (pw, ph) = size.dimensions();
            state.page_ops.lock().unwrap()
                .entry(hid)
                .or_default()
                .push(dispatch::PageOp::NewPage { width: pw, height: ph });
            let mut w = ResponseWriter::ok();
            w.put_bool("added", true);
            w.finish()
        }
        "builderPageOp" => handle_builder_page_op(state, &req, take_source(&mut sources, 0)),
        "builderPageDone" => {
            let hid = req.get_i32("handleId").unwrap_or(0) as u32;
            state.page_ops.lock().unwrap()
                .entry(hid)
                .or_default()
                .push(dispatch::PageOp::Done);
            let mut w = ResponseWriter::ok();
            w.put_bool("done", true);
            w.finish()
        }

        _ => ResponseWriter::error(&format!("unknown op: {}", req.op())),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Op handlers — each takes &InstanceState instead of accessing globals
// ═══════════════════════════════════════════════════════════════════

fn handle_open(
    state: &InstanceState,
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let password = req.get_str("password");
    let mut doc = if let Some(reader) = source_reader {
        match PdfDocument::from_external_reader(reader.0) {
            Ok(d) => d,
            Err(e) => return ResponseWriter::error(&e.to_string()),
        }
    } else if let Some(bytes) = source_bytes {
        match PdfDocument::from_bytes(bytes.to_vec()) {
            Ok(d) => d,
            Err(e) => return ResponseWriter::error(&e.to_string()),
        }
    } else {
        return ResponseWriter::error("no source for open");
    };
    if let Some(pw) = password {
        let _ = doc.authenticate(pw.as_bytes());
    }
    match dispatch::open_document(&mut doc) {
        Ok(result) => {
            let hid = state.next_handle_id();
            state.documents.lock().unwrap().insert(hid, doc);
            let mut w = ResponseWriter::ok();
            w.put_i32("handleId", hid as i32);
            w.put_i32("pageCount", result.page_count as i32);
            w.put_str("version", &format!("{}.{}", result.version_major, result.version_minor));
            w.put_bool("isEncrypted", result.is_encrypted);
            w.put_bool("requiresPassword", result.requires_password);
            w.put_bool("isTagged", result.is_tagged);
            w.put_i32("encryptionAlgorithm", result.encryption_algorithm as i32);
            w.put_i32("permissionBits", result.permission_bits as i32);
            w.put_str("title", &result.title);
            w.put_str("author", &result.author);
            w.put_str("subject", &result.subject);
            w.put_str("keywords", &result.keywords);
            w.put_map_list("pages", result.pages.len(), |i, item| {
                let p = &result.pages[i];
                item.put_i32("index", i as i32);
                item.put_f64("width", p.width);
                item.put_f64("height", p.height);
                item.put_i32("rotation", p.rotation);
            });
            w.finish()
        }
        Err(e) => ResponseWriter::error(&e.to_string()),
    }
}

fn handle_editor_open(
    state: &InstanceState,
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let password = req.get_str("password");
    let doc = if let Some(reader) = source_reader {
        match PdfDocument::from_external_reader(reader.0) {
            Ok(d) => d,
            Err(e) => return ResponseWriter::error(&e.to_string()),
        }
    } else if let Some(bytes) = source_bytes {
        match PdfDocument::from_bytes(bytes.to_vec()) {
            Ok(d) => d,
            Err(e) => return ResponseWriter::error(&e.to_string()),
        }
    } else {
        return ResponseWriter::error("no source for editorOpen");
    };
    if let Some(pw) = password {
        let _ = doc.authenticate(pw.as_bytes());
    }
    let editor = match DocumentEditor::from_document(doc) {
        Ok(e) => e,
        Err(e) => return ResponseWriter::error(&e.to_string()),
    };
    let hid = state.next_handle_id();
    state.editors.lock().unwrap().insert(hid, editor);
    let mut w = ResponseWriter::ok();
    w.put_i32("handleId", hid as i32);
    w.finish()
}

fn handle_with_doc<F>(state: &InstanceState, req: &Request<'_>, f: F) -> Vec<u8>
where F: FnOnce(&mut PdfDocument, &Request<'_>) -> crate::error::Result<Vec<u8>>
{
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut docs = state.documents.lock().unwrap();
    match docs.get_mut(&hid) {
        Some(doc) => match f(doc, req) {
            Ok(bytes) => bytes,
            Err(e) => ResponseWriter::error(&e.to_string()),
        },
        None => ResponseWriter::error("document not found"),
    }
}

fn handle_with_editor<F>(state: &InstanceState, req: &Request<'_>, f: F) -> Vec<u8>
where F: FnOnce(&mut DocumentEditor, &Request<'_>) -> crate::error::Result<Vec<u8>>
{
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut editors = state.editors.lock().unwrap();
    match editors.get_mut(&hid) {
        Some(editor) => match f(editor, req) {
            Ok(bytes) => bytes,
            Err(e) => ResponseWriter::error(&e.to_string()),
        },
        None => ResponseWriter::error("editor not found"),
    }
}

fn handle_editor_mutate(
    state: &InstanceState,
    req: &Request<'_>,
    data_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let edit_op = match req.get_str("editOp") {
        Some(op) => op,
        None => return ResponseWriter::error("missing editOp"),
    };
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut editors = state.editors.lock().unwrap();
    let editor = match editors.get_mut(&hid) {
        Some(e) => e,
        None => return ResponseWriter::error("editor not found"),
    };

    match do_editor_mutate(editor, edit_op, req, data_reader) {
        Ok(bytes) => bytes,
        Err(e) => ResponseWriter::error(&e.to_string()),
    }
}

fn do_editor_mutate(
    editor: &mut DocumentEditor,
    edit_op: &str,
    req: &Request<'_>,
    mut data_reader: Option<BoxedReader>,
) -> crate::error::Result<Vec<u8>> {
    use crate::error::Error;

    match edit_op {
        "selectPages" => {
            let pages: Vec<usize> = req.get_int_list("pages").unwrap_or(&[])
                .iter().map(|&p| p as usize).collect();
            dispatch::edit_select_pages(editor, &pages)?;
            ok_response()
        }
        "deletePage" => {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            dispatch::edit_delete_pages(editor, &[page])?;
            ok_response()
        }
        "rotatePage" => {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            let degrees = req.get_i32("degrees").unwrap_or(0);
            dispatch::edit_rotate_pages(editor, &[(page, degrees)])?;
            ok_response()
        }
        "rotateAll" => {
            dispatch::edit_rotate_all(editor, req.get_i32("degrees").unwrap_or(0))?;
            ok_response()
        }
        "movePage" => {
            let from = req.get_i32("from").unwrap_or(0) as usize;
            let to = req.get_i32("to").unwrap_or(0) as usize;
            dispatch::edit_move_page(editor, from, to)?;
            ok_response()
        }
        "flattenForms" => {
            dispatch::edit_flatten_forms(editor)?;
            ok_response()
        }
        "flattenAllAnnotations" => {
            dispatch::edit_flatten_all_annotations(editor)?;
            ok_response()
        }
        "applyRedactionsDestructive" => {
            dispatch::edit_apply_redactions_destructive(editor)?;
            ok_response()
        }
        "compress" => {
            dispatch::edit_compress(editor, req.get_i32("quality").unwrap_or(75) as u8)?;
            ok_response()
        }
        "optimizeImages" => {
            let count = dispatch::edit_optimize_images(
                editor,
                req.get_i32("quality").unwrap_or(75) as u8,
                req.get_i32("minSize").unwrap_or(128) as u32,
            )?;
            let mut w = ResponseWriter::ok();
            w.put_i32("count", count as i32);
            Ok(w.finish())
        }
        "embedFile" => {
            let name = req.get_str("name").unwrap_or("file");
            let data = if let Some(ref mut reader) = data_reader {
                read_all_from_reader(reader)
                    .map_err(|e| Error::InvalidPdf(e.to_string()))?
            } else {
                req.get_bytes("data").unwrap_or(&[]).to_vec()
            };
            dispatch::edit_embed_file(editor, name, data)?;
            ok_response()
        }
        "eraseRegions" => {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            let coords = req.get_f64_list("regions").unwrap_or(&[]);
            let rects: Vec<[f32; 4]> = coords.chunks(4)
                .filter(|c| c.len() == 4)
                .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32])
                .collect();
            dispatch::edit_erase_regions(editor, page, &rects)?;
            ok_response()
        }
        "watermark" => {
            let pos_fields: Vec<f32> = req.get_f64_list("posFields")
                .unwrap_or(&[]).iter().map(|&f| f as f32).collect();
            dispatch::edit_watermark(
                editor,
                req.get_i32("page").unwrap_or(-1),
                req.get_str("text").unwrap_or("WATERMARK"),
                req.get_f64("fontSize").unwrap_or(48.0) as f32,
                req.get_f64("rotation").unwrap_or(45.0) as f32,
                req.get_f64("opacity").unwrap_or(0.3) as f32,
                req.get_f64("r").unwrap_or(0.5) as f32,
                req.get_f64("g").unwrap_or(0.5) as f32,
                req.get_f64("b").unwrap_or(0.5) as f32,
                req.get_i32("layer").unwrap_or(0),
                req.get_i32("posType").unwrap_or(0),
                &pos_fields,
            )?;
            ok_response()
        }
        "addStamp" => {
            dispatch::edit_add_stamp(
                editor,
                req.get_i32("page").unwrap_or(0) as usize,
                req.get_i32("stampType").unwrap_or(12),
                req.get_f64("x").unwrap_or(0.0) as f32,
                req.get_f64("y").unwrap_or(0.0) as f32,
                req.get_f64("width").unwrap_or(100.0) as f32,
                req.get_f64("height").unwrap_or(50.0) as f32,
                req.get_f64("opacity").unwrap_or(1.0) as f32,
            )?;
            ok_response()
        }
        "addImageStamp" => {
            let image_bytes = if let Some(ref mut reader) = data_reader {
                read_all_from_reader(reader)
                    .map_err(|e| Error::InvalidPdf(e.to_string()))?
            } else {
                req.get_bytes("imageData").unwrap_or(&[]).to_vec()
            };
            dispatch::edit_add_image_stamp(
                editor,
                req.get_i32("page").unwrap_or(0) as usize,
                image_bytes,
                req.get_f64("x").unwrap_or(0.0) as f32,
                req.get_f64("y").unwrap_or(0.0) as f32,
                req.get_f64("width").unwrap_or(100.0) as f32,
                req.get_f64("height").unwrap_or(50.0) as f32,
                req.get_f64("opacity").unwrap_or(1.0) as f32,
            )?;
            ok_response()
        }
        "setTitle" => {
            dispatch::edit_set_title(editor, req.get_str("title").unwrap_or(""));
            ok_response()
        }
        "setAuthor" => {
            dispatch::edit_set_author(editor, req.get_str("author").unwrap_or(""));
            ok_response()
        }
        "setSubject" => {
            dispatch::edit_set_subject(editor, req.get_str("subject").unwrap_or(""));
            ok_response()
        }
        "setKeywords" => {
            dispatch::edit_set_keywords(editor, req.get_str("keywords").unwrap_or(""));
            ok_response()
        }
        "unembedStandardFonts" => {
            let count = dispatch::edit_unembed_standard_fonts(editor)?;
            let mut w = ResponseWriter::ok();
            w.put_i32("count", count as i32);
            Ok(w.finish())
        }
        "setFormFieldValue" => {
            dispatch::edit_set_form_field_value(
                editor,
                req.get_str("fieldName").unwrap_or(""),
                req.get_str("value").unwrap_or(""),
            )?;
            ok_response()
        }
        "cropMargins" => {
            dispatch::edit_crop_margins(
                editor,
                req.get_f64("left").unwrap_or(0.0) as f32,
                req.get_f64("right").unwrap_or(0.0) as f32,
                req.get_f64("top").unwrap_or(0.0) as f32,
                req.get_f64("bottom").unwrap_or(0.0) as f32,
            )?;
            ok_response()
        }
        "convertToPdfA" => {
            dispatch::edit_convert_to_pdf_a(editor, req.get_i32("level").unwrap_or(2))?;
            ok_response()
        }
        "resizeImage" => {
            dispatch::edit_resize_image(
                editor,
                req.get_i32("page").unwrap_or(0) as usize,
                req.get_str("imageName").unwrap_or(""),
                req.get_f64("width").unwrap_or(100.0) as f32,
                req.get_f64("height").unwrap_or(100.0) as f32,
            )?;
            ok_response()
        }
        "addRedaction" => {
            dispatch::edit_add_redaction(
                editor,
                req.get_i32("page").unwrap_or(0) as usize,
                [
                    req.get_f64("x").unwrap_or(0.0) as f32,
                    req.get_f64("y").unwrap_or(0.0) as f32,
                    req.get_f64("width").unwrap_or(100.0) as f32,
                    req.get_f64("height").unwrap_or(50.0) as f32,
                ],
            )?;
            ok_response()
        }
        "scrubMetadata" => {
            dispatch::edit_scrub_metadata(editor)?;
            ok_response()
        }
        _ => Err(Error::InvalidPdf(format!("unknown editOp: {edit_op}"))),
    }
}

fn handle_builder_page_op(
    state: &InstanceState,
    req: &Request<'_>,
    mut data_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let page_op = match req.get_str("pageOp") {
        Some(op) => op,
        None => return ResponseWriter::error("missing pageOp"),
    };

    let op = match page_op {
        "font" => dispatch::PageOp::Font(
            req.get_str("name").unwrap_or("Helvetica").to_string(),
            req.get_f64("size").unwrap_or(12.0) as f32,
        ),
        "at" => dispatch::PageOp::At(
            req.get_f64("x").unwrap_or(0.0) as f32,
            req.get_f64("y").unwrap_or(0.0) as f32,
        ),
        "text" => dispatch::PageOp::Text(req.get_str("text").unwrap_or("").to_string()),
        "heading" => dispatch::PageOp::Heading(
            req.get_i32("level").unwrap_or(1) as u8,
            req.get_str("text").unwrap_or("").to_string(),
        ),
        "paragraph" => dispatch::PageOp::Paragraph(req.get_str("text").unwrap_or("").to_string()),
        "space" => dispatch::PageOp::Space(req.get_f64("points").unwrap_or(12.0) as f32),
        "horizontalRule" => dispatch::PageOp::HorizontalRule,
        "watermark" => dispatch::PageOp::Watermark(req.get_str("text").unwrap_or("").to_string()),
        "image" => {
            let image_data = if let Some(ref mut reader) = data_reader {
                read_all_from_reader(reader).unwrap_or_default()
            } else {
                req.get_bytes("data").unwrap_or(&[]).to_vec()
            };
            dispatch::PageOp::Image {
                data: image_data,
                x: req.get_f64("x").unwrap_or(0.0) as f32,
                y: req.get_f64("y").unwrap_or(0.0) as f32,
                w: req.get_f64("width").unwrap_or(0.0) as f32,
                h: req.get_f64("height").unwrap_or(0.0) as f32,
                alt: req.get_str("altText").unwrap_or("").to_string(),
            }
        }
        "textField" => dispatch::PageOp::TextField {
            name: req.get_str("name").unwrap_or("").to_string(),
            x: req.get_f64("x").unwrap_or(0.0) as f32,
            y: req.get_f64("y").unwrap_or(0.0) as f32,
            w: req.get_f64("width").unwrap_or(0.0) as f32,
            h: req.get_f64("height").unwrap_or(0.0) as f32,
            default_value: req.get_str("defaultValue").map(|s| s.to_string()),
        },
        "checkbox" => dispatch::PageOp::Checkbox {
            name: req.get_str("name").unwrap_or("").to_string(),
            x: req.get_f64("x").unwrap_or(0.0) as f32,
            y: req.get_f64("y").unwrap_or(0.0) as f32,
            w: req.get_f64("width").unwrap_or(0.0) as f32,
            h: req.get_f64("height").unwrap_or(0.0) as f32,
            checked: req.get_bool("checked").unwrap_or(false),
        },
        "comboBox" => dispatch::PageOp::ComboBox {
            name: req.get_str("name").unwrap_or("").to_string(),
            x: req.get_f64("x").unwrap_or(0.0) as f32,
            y: req.get_f64("y").unwrap_or(0.0) as f32,
            w: req.get_f64("width").unwrap_or(0.0) as f32,
            h: req.get_f64("height").unwrap_or(0.0) as f32,
            options: req.get_string_list("options").unwrap_or_default().into_iter().map(|s| s.to_string()).collect(),
            selected: req.get_str("selected").map(|s| s.to_string()),
        },
        "pushButton" => dispatch::PageOp::PushButton {
            name: req.get_str("name").unwrap_or("").to_string(),
            x: req.get_f64("x").unwrap_or(0.0) as f32,
            y: req.get_f64("y").unwrap_or(0.0) as f32,
            w: req.get_f64("width").unwrap_or(0.0) as f32,
            h: req.get_f64("height").unwrap_or(0.0) as f32,
            caption: req.get_str("caption").unwrap_or("").to_string(),
        },
        "signatureField" => dispatch::PageOp::SignatureField {
            name: req.get_str("name").unwrap_or("").to_string(),
            x: req.get_f64("x").unwrap_or(0.0) as f32,
            y: req.get_f64("y").unwrap_or(0.0) as f32,
            w: req.get_f64("width").unwrap_or(0.0) as f32,
            h: req.get_f64("height").unwrap_or(0.0) as f32,
        },
        "fieldKeystroke" => dispatch::PageOp::FieldKeystroke(req.get_str("script").unwrap_or("").to_string()),
        "fieldFormat" => dispatch::PageOp::FieldFormat(req.get_str("script").unwrap_or("").to_string()),
        "fieldValidate" => dispatch::PageOp::FieldValidate(req.get_str("script").unwrap_or("").to_string()),
        "fieldCalculate" => dispatch::PageOp::FieldCalculate(req.get_str("script").unwrap_or("").to_string()),
        "linkUrl" => dispatch::PageOp::LinkUrl(req.get_str("url").unwrap_or("").to_string()),
        "linkPage" => dispatch::PageOp::LinkPage(req.get_i32("targetPage").unwrap_or(0) as usize),
        "footnote" => dispatch::PageOp::Footnote {
            ref_mark: req.get_str("refMark").unwrap_or("").to_string(),
            note_text: req.get_str("noteText").unwrap_or("").to_string(),
        },
        "columns" => dispatch::PageOp::Columns {
            column_count: req.get_i32("columnCount").unwrap_or(2) as u32,
            gap_pt: req.get_f64("gapPt").unwrap_or(12.0) as f32,
            text: req.get_str("text").unwrap_or("").to_string(),
        },
        "newline" => dispatch::PageOp::Newline,
        "newPageSameSize" => dispatch::PageOp::NewPageSameSize,
        _ => {
            return ResponseWriter::error(&format!("unknown pageOp: {page_op}"));
        }
    };

    state.page_ops.lock().unwrap()
        .entry(hid)
        .or_default()
        .push(op);

    let mut w = ResponseWriter::ok();
    w.put_bool("buffered", true);
    w.finish()
}

fn handle_convert_to(
    state: &InstanceState,
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let format = req.get_str("format").unwrap_or("docx");

    // Try handle-based path first (if caller opened the doc already).
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    if hid > 0 {
        let mut docs = state.documents.lock().unwrap();
        if let Some(doc) = docs.get_mut(&hid) {
            return convert_to_with_doc(doc, format, sink_writer);
        }
    }

    // One-shot path: open from source, convert, no handle needed.
    // O(1)-memory via BoxedReader when available.
    let mut doc = if let Some(reader) = source_reader {
        match PdfDocument::from_external_reader(reader.0) {
            Ok(d) => d,
            Err(e) => return ResponseWriter::error(&e.to_string()),
        }
    } else if let Some(bytes) = source_bytes {
        match PdfDocument::from_bytes(bytes.to_vec()) {
            Ok(d) => d,
            Err(e) => return ResponseWriter::error(&e.to_string()),
        }
    } else {
        return ResponseWriter::error("no source for convertTo");
    };

    if let Some(pw) = req.get_str("password") {
        let _ = doc.authenticate(pw.as_bytes());
    }

    convert_to_with_doc(&mut doc, format, sink_writer)
}

fn convert_to_with_doc(
    doc: &mut PdfDocument,
    format: &str,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    if let Some(mut writer) = sink_writer {
        match dispatch::convert_to_format_writer(doc, format, &mut writer) {
            Ok(()) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("streamed", true);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    } else {
        match dispatch::convert_to_format(doc, format) {
            Ok(bytes) => {
                let mut w = ResponseWriter::ok();
                w.put_bytes("data", &bytes);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
}

fn handle_convert_to_pdf(
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let format = req.get_str("format").unwrap_or("docx");

    if let Some(mut writer) = sink_writer {
        // Streaming path: reader → converter → writer. O(1) memory.
        if let Some(reader) = source_reader {
            match dispatch::convert_from_format_writer(reader, format, &mut writer) {
                Ok(()) => {
                    let mut w = ResponseWriter::ok();
                    w.put_bool("streamed", true);
                    w.finish()
                }
                Err(e) => ResponseWriter::error(&e.to_string()),
            }
        } else if let Some(bytes) = source_bytes {
            match dispatch::convert_from_format_writer(std::io::Cursor::new(bytes.to_vec()), format, &mut writer) {
                Ok(()) => {
                    let mut w = ResponseWriter::ok();
                    w.put_bool("streamed", true);
                    w.finish()
                }
                Err(e) => ResponseWriter::error(&e.to_string()),
            }
        } else {
            ResponseWriter::error("no source for convertToPdf")
        }
    } else {
        // Fallback: no sink, return bytes in response.
        let data = if let Some(mut reader) = source_reader {
            match read_all_from_reader(&mut reader) {
                Ok(d) => d,
                Err(e) => return ResponseWriter::error(&e.to_string()),
            }
        } else if let Some(b) = source_bytes {
            b.to_vec()
        } else {
            return ResponseWriter::error("no source for convertToPdf");
        };
        match dispatch::convert_from_format_to_bytes(&data, format) {
            Ok(bytes) => {
                let mut w = ResponseWriter::ok();
                w.put_bytes("data", &bytes);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
}

fn handle_builder_save(state: &InstanceState, req: &Request<'_>, sink_writer: Option<BoxedWriter>) -> Vec<u8> {
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut builders_map = state.builders.lock().unwrap();
    let builder = match builders_map.remove(&hid) {
        Some(b) => b,
        None => return ResponseWriter::error("builder not found"),
    };

    let ops = state.page_ops.lock().unwrap().remove(&hid).unwrap_or_default();
    let mut builder = builder;
    if !ops.is_empty() {
        dispatch::replay_page_ops(&mut builder, PageSize::A4, ops);
    }

    if let Some(mut writer) = sink_writer {
        match dispatch::builder_save_to_writer(builder, &mut writer) {
            Ok(()) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("streamed", true);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    } else {
        match dispatch::builder_save(builder) {
            Ok(bytes) => {
                let mut w = ResponseWriter::ok();
                w.put_bytes("data", &bytes);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
}

fn handle_render_streamed(state: &InstanceState, req: &Request<'_>, sink_writer: Option<BoxedWriter>) -> Vec<u8> {
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut docs = state.documents.lock().unwrap();
    let doc = match docs.get_mut(&hid) {
        Some(d) => d,
        None => return ResponseWriter::error("document not found"),
    };

    let page_indices = req.get_int_list("pageIndices").unwrap_or(&[]);
    // Empty list = all pages (Dart convention: PdfPages.all() sends []).
    let pages: Vec<usize> = if page_indices.is_empty() {
        (0..doc.page_count().unwrap_or(0)).collect()
    } else {
        page_indices.iter().map(|&p| p as usize).collect()
    };
    let max_w = req.get_i32("maxWidth").unwrap_or(0) as u32;
    let max_h = req.get_i32("maxHeight").unwrap_or(0) as u32;

    if let Some(mut writer) = sink_writer {
        match dispatch::render_pages_streamed(doc, &pages, max_w, max_h, &mut writer) {
            Ok(count) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("streamed", true);
                w.put_i32("itemCount", count as i32);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    } else {
        let page = req.get_i32("page").unwrap_or(0) as usize;
        match dispatch::render_page(doc, page, max_w, max_h) {
            Ok(result) => {
                let mut w = ResponseWriter::ok();
                w.put_i32("width", result.width as i32);
                w.put_i32("height", result.height as i32);
                w.put_bytes("data", &result.data);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
}

fn handle_extract_images_streamed(state: &InstanceState, req: &Request<'_>, sink_writer: Option<BoxedWriter>) -> Vec<u8> {
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut docs = state.documents.lock().unwrap();
    let doc = match docs.get_mut(&hid) {
        Some(d) => d,
        None => return ResponseWriter::error("document not found"),
    };

    let page = req.get_i32("page").unwrap_or(0) as usize;

    if let Some(mut writer) = sink_writer {
        match dispatch::extract_images_streamed(doc, page, &mut writer) {
            Ok(count) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("streamed", true);
                w.put_i32("itemCount", count as i32);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    } else {
        match dispatch::extract_images(doc, page) {
            Ok(result) => {
                let mut w = ResponseWriter::ok();
                w.put_map_list("images", result.len(), |i, item| {
                    let img = &result[i];
                    item.put_i32("width", img.width as i32);
                    item.put_i32("height", img.height as i32);
                    item.put_str("format", &img.format);
                    item.put_str("colorSpace", &img.color_space);
                    item.put_i32("bitsPerComponent", img.bits_per_component as i32);
                    item.put_bytes("data", &img.data);
                });
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
}

#[cfg(feature = "signatures")]
fn handle_sign(
    req: &Request<'_>,
    source_reader: Option<BoxedReader>,
    source_bytes: Option<&[u8]>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    use crate::signatures::{SignOptions, SigningCredentials};

    let credentials = if let (Some(cert), Some(pw)) = (req.get_bytes("certificate"), req.get_str("certificatePassword")) {
        SigningCredentials::from_pkcs12(cert, pw)
    } else if let (Some(cert_pem), Some(key_pem)) = (req.get_str("certPem"), req.get_str("keyPem")) {
        SigningCredentials::from_pem(cert_pem, key_pem)
    } else {
        return ResponseWriter::error("sign: missing credentials (certificate+password or certPem+keyPem)");
    };

    let credentials = match credentials {
        Ok(c) => c,
        Err(e) => return ResponseWriter::error(&format!("sign: invalid credentials: {e}")),
    };

    let opts = SignOptions {
        reason: req.get_str("reason").map(|s| s.to_string()),
        location: req.get_str("location").map(|s| s.to_string()),
        ..Default::default()
    };

    if let (Some(mut reader), Some(mut writer)) = (source_reader, sink_writer) {
        let length = req.get_i64("sourceLength").unwrap_or(0) as u64;
        match crate::host::sign::sign_pdf(
            &mut reader, length, &mut writer, &credentials, opts,
        ) {
            Ok(()) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("signed", true);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    } else if let Some(bytes) = source_bytes {
        match crate::signatures::sign_pdf_bytes(bytes, &credentials, opts) {
            Ok(signed) => {
                let mut w = ResponseWriter::ok();
                w.put_bytes("data", &signed);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    } else {
        ResponseWriter::error("sign: no source provided")
    }
}

#[cfg(not(feature = "signatures"))]
fn handle_sign(
    _req: &Request<'_>,
    _source_reader: Option<BoxedReader>,
    _source_bytes: Option<&[u8]>,
    _sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    ResponseWriter::error("signatures feature not enabled")
}

fn handle_editor_save(
    state: &InstanceState,
    req: &Request<'_>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut editors = state.editors.lock().unwrap();
    let editor = match editors.get_mut(&hid) {
        Some(e) => e,
        None => return ResponseWriter::error("editor not found"),
    };

    let compress = req.get_bool("compress").unwrap_or(true);
    let gc = req.get_bool("garbageCollect").unwrap_or(true);
    let mode = req.get_i32("saveMode").unwrap_or(0);
    let encrypt_mode = req.get_i32("encryptMode").unwrap_or(0);
    let encryption = match encrypt_mode {
        // 0 = keep (preserve source encryption — editor handles this internally)
        // 1 = remove (strip encryption)
        1 => None,
        // 2 = apply new encryption config
        2 => {
            let algo = match req.get_i32("encryptAlgo").unwrap_or(4) {
                1 => crate::editor::EncryptionAlgorithm::Rc4_40,
                2 => crate::editor::EncryptionAlgorithm::Rc4_128,
                3 => crate::editor::EncryptionAlgorithm::Aes128,
                _ => crate::editor::EncryptionAlgorithm::Aes256,
            };
            let perm_bits = req.get_i32("encryptPermissions").unwrap_or(-1);
            Some(crate::editor::EncryptionConfig {
                user_password: req.get_str("encryptUserPw").unwrap_or("").to_string(),
                owner_password: req.get_str("encryptOwnerPw").unwrap_or("").to_string(),
                algorithm: algo,
                permissions: permissions_from_bits(perm_bits),
            })
        }
        _ => None,
    };
    let options = crate::editor::SaveOptions {
        incremental: mode == 1,
        compress,
        garbage_collect: gc,
        encryption,
        ..Default::default()
    };

    if let Some(mut writer) = sink_writer {
        match dispatch::edit_save_with_options(editor, &mut writer, &options) {
            Ok(()) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("streamed", true);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    } else {
        match editor.save_to_bytes_with_options(options) {
            Ok(bytes) => {
                let mut w = ResponseWriter::ok();
                w.put_bytes("data", &bytes);
                w.finish()
            }
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
}

/// Editor extract pages — select → save(sink) → restore page_order.
/// Editor state is unchanged after the call. O(1) streaming via sink.
fn handle_editor_extract_pages(
    state: &InstanceState,
    req: &Request<'_>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let hid = req.get_i32("handleId").unwrap_or(0) as u32;
    let mut editors = state.editors.lock().unwrap();
    let editor = match editors.get_mut(&hid) {
        Some(e) => e,
        None => return ResponseWriter::error("editor not found"),
    };

    let pages = req.get_int_list("pages").unwrap_or(&[]);
    if pages.is_empty() {
        return ResponseWriter::error("pages list must not be empty");
    }

    let page_count = editor.current_page_count();
    for &p in pages {
        if (p as usize) >= page_count {
            return ResponseWriter::error(&format!(
                "page index {} out of range (document has {} pages)", p, page_count
            ));
        }
    }

    let visible: Vec<i32> = editor.page_order_visible();
    let new_order: Vec<i32> = pages.iter().map(|&i| visible[i as usize]).collect();

    // Save + restore state — editor unchanged after this call.
    let saved_order = std::mem::replace(editor.page_order_mut(), new_order);
    let saved_modified = editor.is_modified();
    editor.set_modified(true);

    // Stage trimmed /Pages for GC (same as extract_pages_to_bytes).
    let staged = editor.stage_trimmed_pages_for_gc();

    let result = if let Some(mut writer) = sink_writer {
        dispatch::edit_save_with_options(
            editor, &mut writer, &crate::editor::SaveOptions::full_rewrite(),
        )
    } else {
        Err(crate::error::Error::InvalidPdf(
            "editorExtractPages requires a sink".into(),
        ))
    };

    // Always restore — even on error.
    *editor.page_order_mut() = saved_order;
    editor.set_modified(saved_modified);
    if let Some((pages_id, prior)) = staged {
        match prior {
            Some(prev) => { editor.modified_objects_mut().insert(pages_id, prev); }
            None => { editor.modified_objects_mut().remove(&pages_id); }
        }
    }

    match result {
        Ok(()) => {
            let mut w = ResponseWriter::ok();
            w.put_bool("streamed", true);
            w.finish()
        }
        Err(e) => ResponseWriter::error(&e.to_string()),
    }
}

fn ok_response() -> crate::error::Result<Vec<u8>> {
    let mut w = ResponseWriter::ok();
    w.put_bool("ok", true);
    Ok(w.finish())
}

// Decode PDF permission bits (same layout as Dart's PdfPermissions.toBits).
fn permissions_from_bits(bits: i32) -> crate::editor::Permissions {
    crate::editor::Permissions {
        print: bits & (1 << 2) != 0,
        print_high_quality: bits & (1 << 11) != 0,
        modify: bits & (1 << 3) != 0,
        copy: bits & (1 << 4) != 0,
        annotate: bits & (1 << 5) != 0,
        fill_forms: bits & (1 << 8) != 0,
        accessibility: bits & (1 << 9) != 0,
        assemble: bits & (1 << 10) != 0,
    }
}

// ═══════════════════════════════════════════════════════════════════
// Native FFI entry point — per-instance pool + allo-isolate
// ═══════════════════════════════════════════════════════════════════

#[cfg(not(target_arch = "wasm32"))]
mod ffi_entry {
    use super::*;
    use crate::host::native::callback_reader::CallbackReader;
    use crate::host::native::callback_writer::CallbackWriter;
    use crate::host::native::thread_pool::{Task, ThreadPool};
    use std::sync::Arc;

    /// Combined instance: InstanceState + ThreadPool.
    /// The pool is separate because InstanceState is shared (&ref)
    /// while the pool owns the threads.
    struct NativeInstance {
        state: InstanceState,
        pool: ThreadPool,
    }

    /// Create a new engine instance. Returns an opaque pointer.
    ///
    /// The instance owns its own thread pool and handle maps.
    /// Multiple instances are fully isolated.
    /// Call `bridge_shutdown` to destroy.
    #[no_mangle]
    pub unsafe extern "C" fn bridge_init() -> *mut std::ffi::c_void {
        let instance = Box::new(NativeInstance {
            state: InstanceState::new(),
            pool: ThreadPool::new(),
        });
        Box::into_raw(instance) as *mut std::ffi::c_void
    }

    /// Execute an operation on an instance's thread pool.
    ///
    /// Submits work to the pool and returns immediately.
    /// The pool thread runs handle_request, then posts the
    /// result back to Dart via allo-isolate.
    /// source_count: how many sources. Each source has a (buf, notify, length) triple.
    /// source_bufs, source_notifys, source_lengths: parallel arrays of size source_count.
    /// sink_count, sink_bufs, sink_notifys: parallel arrays for output sinks.
    #[no_mangle]
    pub unsafe extern "C" fn bridge_execute(
        instance: *mut std::ffi::c_void,
        request_ptr: *const u8,
        request_len: i32,
        source_count: i32,
        source_bufs: *const *mut u8,
        source_notifys: *const Option<unsafe extern "C" fn()>,
        source_lengths: *const i64,
        sink_count: i32,
        sink_bufs: *const *mut u8,
        sink_notifys: *const Option<unsafe extern "C" fn()>,
        result_port: i64,
    ) {
        let inst = &*(instance as *const NativeInstance);
        let request_owned = std::slice::from_raw_parts(request_ptr, request_len as usize).to_vec();

        let sc = source_count as usize;
        let skc = sink_count as usize;

        // Copy array data to owned vecs (Send-safe usize + fn ptrs)
        let src_bufs: Vec<usize> = (0..sc).map(|i| *source_bufs.add(i) as usize).collect();
        let src_notifys: Vec<Option<unsafe extern "C" fn()>> = (0..sc).map(|i| *source_notifys.add(i)).collect();
        let src_lengths: Vec<i64> = (0..sc).map(|i| *source_lengths.add(i)).collect();
        let snk_bufs: Vec<usize> = (0..skc).map(|i| *sink_bufs.add(i) as usize).collect();
        let snk_notifys: Vec<Option<unsafe extern "C" fn()>> = (0..skc).map(|i| *sink_notifys.add(i)).collect();

        let cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = cancel.clone();
        let state_addr = &inst.state as *const InstanceState as usize;

        let task = Task::new(
            move |_arena| {
                let state = unsafe { &*(state_addr as *const InstanceState) };

                let mut sources = Vec::with_capacity(src_bufs.len());
                for i in 0..src_bufs.len() {
                    if src_bufs[i] != 0 && src_notifys[i].is_some() && src_lengths[i] > 0 {
                        let cb = unsafe {
                            CallbackReader::new(
                                src_bufs[i] as *mut u8,
                                src_notifys[i].unwrap(),
                                src_lengths[i] as u64,
                                Some(cancel.clone()),
                            )
                        };
                        sources.push(BoxedReader(Box::new(cb)));
                    }
                }

                let mut sink_vec = Vec::with_capacity(snk_bufs.len());
                for i in 0..snk_bufs.len() {
                    if snk_bufs[i] != 0 && snk_notifys[i].is_some() {
                        let cb = unsafe {
                            CallbackWriter::new(
                                snk_bufs[i] as *mut u8,
                                snk_notifys[i].unwrap(),
                                Some(cancel.clone()),
                            )
                        };
                        sink_vec.push(BoxedWriter(Box::new(cb)));
                    }
                }

                let result = handle_request(state, &request_owned, None, sources, sink_vec);

                let isolate = allo_isolate::Isolate::new(result_port);
                isolate.post(allo_isolate::ZeroCopyBuffer(result));
            },
            task_cancel,
        );

        let _ = inst.pool.submit(task);
    }

    /// Destroy an instance. Drops all handles, drains the pool.
    ///
    /// After this call, the instance pointer is invalid.
    /// All documents, editors, builders — gone.
    /// All pool threads — stopped and joined.
    #[no_mangle]
    pub unsafe extern "C" fn bridge_shutdown(instance: *mut std::ffi::c_void) {
        if instance.is_null() { return; }
        let inst = Box::from_raw(instance as *mut NativeInstance);
        // Set cancel flag so in-flight readers/writers bail early.
        inst.state.cancel.store(true, Ordering::Relaxed);
        // Drop inst — ThreadPool::drop drains threads, HashMap::drop frees handles.
        drop(inst);
    }

    // ── Sync buffer lifecycle (called by Dart coordinator) ──
    //
    // Cross-platform: std::sync::Mutex + Condvar stored as heap-allocated
    // SyncPair, pointer written into the buffer's sync_ptr slot.

    use crate::host::native::shared_buffer as sb;

    #[no_mangle]
    pub unsafe extern "C" fn bridge_init_read_buffer(buf: *mut u8) {
        sb::init_sync(buf, sb::read_channel::OFFSET_SYNC_PTR);
    }

    #[no_mangle]
    pub unsafe extern "C" fn bridge_destroy_read_buffer(buf: *mut u8) {
        sb::destroy_sync(buf, sb::read_channel::OFFSET_SYNC_PTR);
    }

    #[no_mangle]
    pub unsafe extern "C" fn bridge_signal_read(buf: *mut u8) {
        sb::notify(buf, sb::read_channel::OFFSET_SYNC_PTR);
    }

    #[no_mangle]
    pub unsafe extern "C" fn bridge_init_write_buffer(buf: *mut u8) {
        sb::init_sync(buf, sb::write_channel::OFFSET_SYNC_PTR);
    }

    #[no_mangle]
    pub unsafe extern "C" fn bridge_destroy_write_buffer(buf: *mut u8) {
        sb::destroy_sync(buf, sb::write_channel::OFFSET_SYNC_PTR);
    }

    #[no_mangle]
    pub unsafe extern "C" fn bridge_signal_write(buf: *mut u8) {
        sb::notify(buf, sb::write_channel::OFFSET_SYNC_PTR);
    }

    #[no_mangle]
    pub extern "C" fn bridge_read_buffer_size() -> i32 {
        sb::read_channel::TOTAL_SIZE as i32
    }

    #[no_mangle]
    pub extern "C" fn bridge_write_buffer_size() -> i32 {
        sb::write_channel::TOTAL_SIZE as i32
    }
}

// ═══════════════════════════════════════════════════════════════════
// WASM entry point — per-instance state in WASM linear memory
// ═══════════════════════════════════════════════════════════════════

#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use super::*;
    use crate::host::wasm::js_reader::JsCallbackReader;
    use crate::host::wasm::js_writer::JsCallbackWriter;
    use wasm_bindgen::prelude::*;

    /// Create a new engine instance. Returns a WASM pointer.
    #[wasm_bindgen]
    pub fn bridge_init() -> u32 {
        let instance = Box::new(InstanceState::new());
        Box::into_raw(instance) as u32
    }

    /// Execute an operation within an instance.
    ///
    /// source_lengths: packed f64 array — each 8 bytes is one source length.
    /// Sources get indexed readers (0, 1, 2, ...).
    /// sink_count: how many output sinks exist.
    /// Sinks get indexed writers (0, 1, 2, ...).
    #[wasm_bindgen]
    pub fn bridge_execute(
        instance_ptr: u32,
        request_bytes: &[u8],
        source_lengths: &[u8],
        sink_count: u32,
    ) -> Vec<u8> {
        let state = unsafe { &*(instance_ptr as *const InstanceState) };

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
            if len > 0.0 {
                sources.push(BoxedReader(Box::new(JsCallbackReader::new(i as u32, len as u64))));
            }
        }

        let mut sinks = Vec::new();
        for i in 0..sink_count {
            sinks.push(BoxedWriter(Box::new(JsCallbackWriter::new(i))));
        }

        handle_request(state, request_bytes, None, sources, sinks)
    }

    /// Destroy an instance. Drops all handles, frees all WASM heap memory.
    #[wasm_bindgen]
    pub fn bridge_shutdown(instance_ptr: u32) {
        if instance_ptr == 0 { return; }
        unsafe {
            let _ = Box::from_raw(instance_ptr as *mut InstanceState);
        }
    }
}
