//! Bridge API — the shared request brain for both native and WASM lanes.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! ## Lane model
//!
//! Engine state lives per LANE (`LaneState`) — one isolated execution
//! unit per native thread / per Web Worker. This file never spawns,
//! routes, or kills lanes; it is the decision-free request brain that
//! every lane body calls:
//!
//!   Native lane body: `host/native/lane.rs` (mailbox thread)
//!   Web lane body:    `web_assets/lane_worker.js` (Worker + wasm_entry)
//!
//! ## Request routing
//!
//! `handle_request` parses binary request bytes, calls the matching
//! dispatch function against the lane's own state, and encodes the
//! result as binary response bytes. Same source code for both
//! targets. 100% platform-agnostic. No locks — a lane's state has
//! exactly one owner thread, so handlers take `&mut LaneState`.

use crate::host::binary_codec::{Request, ResponseWriter};
use crate::host::dispatch;
use crate::host::lane_state::LaneState;

use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::writer::PageSize;

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
    state: &mut LaneState,
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
    // Helper: take the op's DATA source (image bytes, embedded file,
    // merge document). Pinned ops re-create the document reader at
    // sources[0], so their data rides at sources[1]; non-pinned ops
    // carry data at sources[0].
    fn take_data_reader(sources: &mut Vec<BoxedReader>) -> Option<BoxedReader> {
        if sources.len() > 1 {
            Some(sources.remove(1))
        } else if !sources.is_empty() {
            Some(sources.remove(0))
        } else {
            None
        }
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
            state.documents.remove(&req_handle(&req));
            ok_flag("disposed")
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

        // ── Editor lifecycle ──
        "editorOpen" => handle_editor_open(state, &req, source_bytes, take_source(&mut sources, 0)),
        "editorDispose" => {
            state.editors.remove(&req_handle(&req));
            ok_flag("disposed")
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
        "editorMutate" => {
            let data = take_data_reader(&mut sources);
            handle_editor_mutate(state, &req, data)
        }
        "editorMergeFrom" => {
            let data = take_data_reader(&mut sources);
            handle_editor_merge_from(state, &req, data)
        }

        // ── Builder lifecycle ──
        "builderCreate" => handle_builder_create(state),
        "builderDispose" => {
            let hid = req_handle(&req);
            state.builders.remove(&hid);
            state.page_ops.remove(&hid);
            ok_flag("disposed")
        }
        "builderSetMetadata" => handle_builder_set_metadata(state, &req),
        "builderAddPage" => handle_builder_add_page(state, &req),
        "builderPageOp" => handle_builder_page_op(state, &req, take_source(&mut sources, 0)),
        "builderPageDone" => {
            state.page_ops
                .entry(req_handle(&req))
                .or_default()
                .push(dispatch::PageOp::Done);
            ok_flag("done")
        }

        _ => ResponseWriter::error(&format!("unknown op: {}", req.op())),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Op handlers — each takes &mut LaneState; no handler touches globals
// ═══════════════════════════════════════════════════════════════════

/// The request's handle id (0 when absent — never a valid handle).
fn req_handle(req: &Request<'_>) -> u32 {
    req.get_i32("handleId").unwrap_or(0) as u32
}

/// A success response carrying a single `true` flag.
fn ok_flag(key: &str) -> Vec<u8> {
    let mut w = ResponseWriter::ok();
    w.put_bool(key, true);
    w.finish()
}

/// Open a document from the op's source: the streaming reader when the
/// transport provided one, else inline bytes. `Err` carries the
/// encoded error response, ready to return.
fn open_source(
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
    op: &str,
) -> Result<PdfDocument, Vec<u8>> {
    if let Some(reader) = source_reader {
        PdfDocument::from_external_reader(reader.0)
            .map_err(|e| ResponseWriter::error(&e.to_string()))
    } else if let Some(bytes) = source_bytes {
        PdfDocument::from_bytes(bytes.to_vec())
            .map_err(|e| ResponseWriter::error(&e.to_string()))
    } else {
        Err(ResponseWriter::error(&format!("no source for {op}")))
    }
}

fn handle_builder_create(state: &mut LaneState) -> Vec<u8> {
    let builder = dispatch::builder_new();
    let hid = state.next_handle_id();
    state.builders.insert(hid, builder);
    let mut w = ResponseWriter::ok();
    w.put_i32("handleId", hid as i32);
    w.finish()
}

fn handle_builder_set_metadata(state: &mut LaneState, req: &Request<'_>) -> Vec<u8> {
    let hid = req_handle(req);
    let Some(mut b) = state.builders.remove(&hid) else {
        return ResponseWriter::error("builder not found");
    };
    if let Some(v) = req.get_str("title") {
        b = dispatch::builder_set_title(b, v);
    }
    if let Some(v) = req.get_str("author") {
        b = dispatch::builder_set_author(b, v);
    }
    if let Some(v) = req.get_str("subject") {
        b = dispatch::builder_set_subject(b, v);
    }
    if let Some(v) = req.get_str("keywords") {
        b = dispatch::builder_set_keywords(b, v);
    }
    state.builders.insert(hid, b);
    ok_flag("set")
}

fn handle_builder_add_page(state: &mut LaneState, req: &Request<'_>) -> Vec<u8> {
    let size = match req.get_str("pageType") {
        Some("a4") => PageSize::A4,
        Some("letter") => PageSize::Letter,
        _ => PageSize::Custom(
            req.get_f64("width").unwrap_or(595.0) as f32,
            req.get_f64("height").unwrap_or(842.0) as f32,
        ),
    };
    let (pw, ph) = size.dimensions();
    state
        .page_ops
        .entry(req_handle(req))
        .or_default()
        .push(dispatch::PageOp::NewPage { width: pw, height: ph });
    ok_flag("added")
}

fn handle_editor_merge_from(
    state: &mut LaneState,
    req: &Request<'_>,
    merge_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let Some(editor) = state.editors.get_mut(&req_handle(req)) else {
        return ResponseWriter::error("editor not found");
    };
    let result = if let Some(reader) = merge_reader {
        editor.merge_from_reader(reader.0)
    } else if let Some(other) = req.get_bytes("otherBytes") {
        editor.merge_from_bytes(other)
    } else {
        return ResponseWriter::error("editorMergeFrom: no source");
    };
    match result {
        Ok(_) => ok_flag("merged"),
        Err(e) => ResponseWriter::error(&e.to_string()),
    }
}

/// Authenticates `doc` against the request's optional password, or
/// returns the error response that refuses the operation.
///
/// Always runs — with the empty password when none was supplied —
/// because the PDF spec's empty-user-password convention is what lets
/// owner-only documents open for everyone, while a real user password
/// makes both the no-password and wrong-password attempts fail.
/// Discarding the result here is the bug class this helper replaces:
/// an unauthenticated encrypted document must refuse, never open.
fn authenticate_or_refuse(doc: &PdfDocument, password: Option<&str>) -> Option<Vec<u8>> {
    match doc.authenticate(password.unwrap_or("").as_bytes()) {
        Ok(true) => None,
        Ok(false) => Some(ResponseWriter::error(if password.is_some() {
            "wrong password"
        } else {
            "password required: document is encrypted with a user password"
        })),
        Err(e) => Some(ResponseWriter::error(&e.to_string())),
    }
}

fn handle_open(
    state: &mut LaneState,
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let password = req.get_str("password");
    let mut doc = match open_source(source_bytes, source_reader, "open") {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    if let Some(resp) = authenticate_or_refuse(&doc, password) {
        return resp;
    }
    match dispatch::open_document(&mut doc) {
        Ok(result) => {
            let hid = state.next_handle_id();
            state.documents.insert(hid, doc);
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
    state: &mut LaneState,
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let password = req.get_str("password");
    let doc = match open_source(source_bytes, source_reader, "editorOpen") {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    if let Some(resp) = authenticate_or_refuse(&doc, password) {
        return resp;
    }
    let editor = match DocumentEditor::from_document(doc) {
        Ok(e) => e,
        Err(e) => return ResponseWriter::error(&e.to_string()),
    };
    let hid = state.next_handle_id();
    state.editors.insert(hid, editor);
    let mut w = ResponseWriter::ok();
    w.put_i32("handleId", hid as i32);
    w.finish()
}

fn handle_with_doc<F>(state: &mut LaneState, req: &Request<'_>, f: F) -> Vec<u8>
where F: FnOnce(&mut PdfDocument, &Request<'_>) -> crate::error::Result<Vec<u8>>
{
    let hid = req_handle(req);
    let docs = &mut state.documents;
    match docs.get_mut(&hid) {
        Some(doc) => match f(doc, req) {
            Ok(bytes) => bytes,
            Err(e) => ResponseWriter::error(&e.to_string()),
        },
        None => ResponseWriter::error("document not found"),
    }
}

fn handle_with_editor<F>(state: &mut LaneState, req: &Request<'_>, f: F) -> Vec<u8>
where F: FnOnce(&mut DocumentEditor, &Request<'_>) -> crate::error::Result<Vec<u8>>
{
    let hid = req_handle(req);
    let editors = &mut state.editors;
    match editors.get_mut(&hid) {
        Some(editor) => match f(editor, req) {
            Ok(bytes) => bytes,
            Err(e) => ResponseWriter::error(&e.to_string()),
        },
        None => ResponseWriter::error("editor not found"),
    }
}

fn handle_editor_mutate(
    state: &mut LaneState,
    req: &Request<'_>,
    data_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let edit_op = match req.get_str("editOp") {
        Some(op) => op,
        None => return ResponseWriter::error("missing editOp"),
    };
    let hid = req_handle(req);
    let editors = &mut state.editors;
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
    state: &mut LaneState,
    req: &Request<'_>,
    mut data_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let hid = req_handle(req);
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

    state.page_ops
        .entry(hid)
        .or_default()
        .push(op);

    ok_flag("buffered")
}

fn handle_convert_to(
    state: &mut LaneState,
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let format = req.get_str("format").unwrap_or("docx");

    // Try handle-based path first (if caller opened the doc already).
    let hid = req_handle(req);
    if hid > 0 {
        let docs = &mut state.documents;
        if let Some(doc) = docs.get_mut(&hid) {
            return convert_to_with_doc(doc, format, sink_writer);
        }
    }

    // One-shot path: open from source, convert, no handle needed.
    // O(1)-memory via BoxedReader when available.
    let mut doc = match open_source(source_bytes, source_reader, "convertTo") {
        Ok(d) => d,
        Err(resp) => return resp,
    };

    if let Some(resp) = authenticate_or_refuse(&doc, req.get_str("password")) {
        return resp;
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
                ok_flag("streamed")
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
                    ok_flag("streamed")
                }
                Err(e) => ResponseWriter::error(&e.to_string()),
            }
        } else if let Some(bytes) = source_bytes {
            match dispatch::convert_from_format_writer(std::io::Cursor::new(bytes.to_vec()), format, &mut writer) {
                Ok(()) => {
                    ok_flag("streamed")
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

fn handle_builder_save(state: &mut LaneState, req: &Request<'_>, sink_writer: Option<BoxedWriter>) -> Vec<u8> {
    let hid = req_handle(req);
    let builders_map = &mut state.builders;
    let builder = match builders_map.remove(&hid) {
        Some(b) => b,
        None => return ResponseWriter::error("builder not found"),
    };

    let ops = state.page_ops.remove(&hid).unwrap_or_default();
    let mut builder = builder;
    if !ops.is_empty() {
        dispatch::replay_page_ops(&mut builder, PageSize::A4, ops);
    }

    if let Some(mut writer) = sink_writer {
        match dispatch::builder_save_to_writer(builder, &mut writer) {
            Ok(()) => {
                ok_flag("streamed")
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

fn handle_render_streamed(state: &mut LaneState, req: &Request<'_>, sink_writer: Option<BoxedWriter>) -> Vec<u8> {
    let hid = req_handle(req);
    let docs = &mut state.documents;
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

fn handle_extract_images_streamed(state: &mut LaneState, req: &Request<'_>, sink_writer: Option<BoxedWriter>) -> Vec<u8> {
    let hid = req_handle(req);
    let docs = &mut state.documents;
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
                ok_flag("signed")
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
    state: &mut LaneState,
    req: &Request<'_>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let hid = req_handle(req);
    let editors = &mut state.editors;
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
                ok_flag("streamed")
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
    state: &mut LaneState,
    req: &Request<'_>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let hid = req_handle(req);
    let editors = &mut state.editors;
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
            ok_flag("streamed")
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
// WASM lane body — one Worker = one lane = one LaneState
// ═══════════════════════════════════════════════════════════════════
//
// The web twin of `host/native/lane.rs`. A Web Worker IS a lane: the
// browser provides the isolated thread; this module provides the
// lane's owned state and the call into the shared request brain.
// Decision-free by design — routing, pinning, queuing, and cancel
// bookkeeping all live in the shared Dart Router.
//
// Hard kill on web is `worker.terminate()` — the browser frees the
// whole WASM heap with the worker, so `lane_destroy` exists only for
// the graceful path (lane reuse without worker restart).

#[cfg(target_arch = "wasm32")]
mod wasm_entry {
    use super::*;
    use crate::host::wasm::js_reader::JsCallbackReader;
    use crate::host::wasm::js_writer::JsCallbackWriter;
    use wasm_bindgen::prelude::*;

    /// Create this worker's lane state. Returns a WASM pointer.
    /// Called exactly once per worker, from lane_worker.js bootstrap.
    #[wasm_bindgen]
    pub fn lane_init() -> u32 {
        let lane = Box::new(LaneState::new());
        Box::into_raw(lane) as u32
    }

    /// Execute one job against this lane's state.
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
    pub fn lane_execute(
        lane_ptr: u32,
        request_bytes: &[u8],
        source_lengths: &[u8],
        sink_count: u32,
    ) -> Vec<u8> {
        let state = unsafe { &mut *(lane_ptr as *mut LaneState) };

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

    /// Drop this lane's state, freeing every handle it owns.
    /// Graceful-path only — `worker.terminate()` makes this moot.
    #[wasm_bindgen]
    pub fn lane_destroy(lane_ptr: u32) {
        if lane_ptr == 0 { return; }
        unsafe {
            let _ = Box::from_raw(lane_ptr as *mut LaneState);
        }
    }
}
