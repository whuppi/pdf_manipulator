//! A file to embed and an image to stamp stay in their source until the save:
//! `editorMutate` `embedFile` and `addImageStamp` keep the reader they are
//! handed, stage the data as a stream over it, and the save copies a file or a
//! JPEG from the source in pieces and deflates a PNG a row at a time.
//!
//! Each proof drives the ops through the request door (`handle_request`) with
//! the source a lane hands it, and compares the saved file with the one the
//! whole-bytes path (`FileData::Whole`, `ImageBytes::Whole`) saves.
//!
//! No Dart battery can observe this: a battery sees the saved bytes, and the
//! whole-bytes path saves the same pixels and the same file; only the drop of
//! the source, the polls of the cancel check and the refusals' effect on the
//! editor tell the two apart.

use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::cancel::CancelCheck;
use crate::host::binary_codec::Request;
use crate::host::bridge_api::{handle_request, BoxedReader, BoxedWriter};
use crate::host::dispatch::{self, FileData};
use crate::host::engine_state::EngineState;
use crate::object::{Object, ObjectRef};
use crate::PdfDocument;

use super::support::{open_editor_from_bytes, pdf_from_objects};

enum Field<'a> {
    Int(i32),
    Real(f64),
    Bool(bool),
    Text(&'a str),
}

fn frame(op: &str, fields: &[(&str, Field)]) -> Vec<u8> {
    let mut out = vec![op.len() as u8];
    out.extend_from_slice(op.as_bytes());
    out.extend_from_slice(&(fields.len() as u16).to_le_bytes());
    for (key, value) in fields {
        out.push(key.len() as u8);
        out.extend_from_slice(key.as_bytes());
        match value {
            Field::Bool(b) => {
                out.push(4);
                out.push(*b as u8);
            },
            Field::Int(n) => {
                out.push(1);
                out.extend_from_slice(&n.to_le_bytes());
            },
            Field::Real(x) => {
                out.push(3);
                out.extend_from_slice(&x.to_le_bytes());
            },
            Field::Text(s) => {
                out.push(5);
                out.extend_from_slice(&(s.len() as u32).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            },
        }
    }
    out
}

fn message_of(answer: &[u8]) -> String {
    let mut as_request = vec![0u8];
    as_request.extend_from_slice(&answer[2..]);
    Request::parse(&as_request).ok().and_then(|r| r.get_str("message").map(str::to_owned)).unwrap_or_default()
}

fn run(state: &mut EngineState, request: Vec<u8>, sources: Vec<BoxedReader>, sinks: Vec<BoxedWriter>, cancel: &CancelCheck) -> Vec<u8> {
    handle_request(state, &request, None, sources, sinks, cancel)
}

#[derive(Clone, Default)]
struct Collect(Arc<Mutex<Vec<u8>>>);

impl Write for Collect {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A source that says when it is dropped.
struct Watched {
    inner: std::io::Cursor<Vec<u8>>,
    dropped: Arc<AtomicBool>,
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

impl Read for Watched {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for Watched {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

fn watched(bytes: &[u8]) -> (BoxedReader, Arc<AtomicBool>) {
    let dropped = Arc::new(AtomicBool::new(false));
    (BoxedReader(Box::new(Watched { inner: std::io::Cursor::new(bytes.to_vec()), dropped: dropped.clone() })), dropped)
}

fn one_page() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
    ])
}

fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed ^ 0x2545_F491_4F6C_DD1D;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

/// An editor over `one_page` in the state, and its handle.
fn open(state: &mut EngineState) -> i32 {
    let bytes = one_page();
    let source = BoxedReader(Box::new(std::io::Cursor::new(bytes.clone())));
    let request = frame("editorOpen", &[("sourceLength", Field::Int(bytes.len() as i32))]);
    let answer = run(state, request, vec![source], Vec::new(), &CancelCheck::none());
    assert_eq!(answer[0], 1, "the editor opens: {}", message_of(&answer));
    let mut as_request = vec![0u8];
    as_request.extend_from_slice(&answer[1..]);
    Request::parse(&as_request).unwrap().get_i32("handleId").unwrap()
}

fn save(state: &mut EngineState, handle: i32, cancel: &CancelCheck) -> (Vec<u8>, Vec<u8>) {
    let out = Collect::default();
    let request = frame(
        "editorSave",
        &[("handleId", Field::Int(handle)), ("saveMode", Field::Int(0)), ("deflate", Field::Bool(false)), ("garbageCollect", Field::Bool(false))],
    );
    let answer = run(state, request, Vec::new(), vec![BoxedWriter::new(out.clone())], cancel);
    let bytes = out.0.lock().unwrap().clone();
    (answer, bytes)
}

fn dispose(state: &mut EngineState, handle: i32) {
    let answer = run(state, frame("editorDispose", &[("handleId", Field::Int(handle))]), Vec::new(), Vec::new(), &CancelCheck::none());
    assert_eq!(answer[0], 1);
}

fn embed(state: &mut EngineState, handle: i32, data: &[u8], source: BoxedReader) -> Vec<u8> {
    let request = frame(
        "editorMutate",
        &[
            ("handleId", Field::Int(handle)),
            ("editOp", Field::Text("embedFile")),
            ("sourceLength", Field::Int(data.len() as i32)),
            ("name", Field::Text("data.bin")),
            ("description", Field::Text("noise")),
            ("mimeType", Field::Text("application/octet-stream")),
        ],
    );
    run(state, request, vec![source], Vec::new(), &CancelCheck::none())
}

fn stamp(state: &mut EngineState, handle: i32, image: &[u8], source: BoxedReader, cancel: &CancelCheck) -> Vec<u8> {
    let request = frame(
        "editorMutate",
        &[
            ("handleId", Field::Int(handle)),
            ("editOp", Field::Text("addImageStamp")),
            ("sourceLength", Field::Int(image.len() as i32)),
            ("pagesAll", Field::Bool(true)),
            ("x", Field::Real(10.0)),
            ("y", Field::Real(10.0)),
            ("width", Field::Real(80.0)),
            ("height", Field::Real(60.0)),
            ("opacity", Field::Real(1.0)),
        ],
    );
    run(state, request, vec![source], Vec::new(), cancel)
}

fn attachment_of(pdf: &[u8]) -> Vec<u8> {
    let mut doc = PdfDocument::from_bytes(pdf.to_vec()).unwrap();
    let out = Collect::default();
    dispatch::doc_extract_attachment(&mut doc, "data.bin", &mut out.clone()).unwrap();
    let bytes = out.0.lock().unwrap().clone();
    bytes
}

/// The `/Params` of the attachment's stream, as text.
fn params_of(pdf: &[u8]) -> String {
    let doc = PdfDocument::from_bytes(pdf.to_vec()).unwrap();
    for id in doc.all_object_ids() {
        if let Ok(Object::Stream { dict, .. }) = doc.load_object(ObjectRef::new(id, 0)) {
            if dict.get("Type").and_then(Object::as_name) == Some("EmbeddedFile") {
                let params = dict.get("Params").and_then(Object::as_dict).expect("the stream has /Params");
                return format!("{:?} {:?} {:?}", params.get("Size"), params.get("CheckSum"), dict.get("Subtype"));
            }
        }
    }
    panic!("no embedded file stream");
}

/// A file embedded from its source saves the file spec, the stream and the
/// checksum the whole-bytes path saves, and the source is held from the op to
/// the dispose.
#[test]
fn an_embedded_file_is_copied_from_its_source_at_the_save() {
    for len in [0usize, 1, 64 * 1024, 300_000] {
        let data = noise(len, 3);
        let mut state = EngineState::new();
        let handle = open(&mut state);
        let (source, dropped) = watched(&data);
        let answer = embed(&mut state, handle, &data, source);
        assert_eq!(answer[0], 1, "{len}: the file is embedded: {}", message_of(&answer));
        assert!(!dropped.load(Ordering::SeqCst), "{len}: the source is held after the op");
        let (answer, streamed) = save(&mut state, handle, &CancelCheck::none());
        assert_eq!(answer[0], 1, "{len}: the editor saves: {}", message_of(&answer));
        dispose(&mut state, handle);
        assert!(dropped.load(Ordering::SeqCst), "{len}: the dispose lets go of the source");

        let mut editor = open_editor_from_bytes(&one_page(), "embed_whole");
        dispatch::edit_embed_file(&mut editor, "data.bin", FileData::Whole(data.clone()), "noise", "application/octet-stream", "").unwrap();
        let whole = editor.save_to_bytes().unwrap();

        assert!(attachment_of(&streamed) == data, "{len}: the attachment's bytes");
        assert_eq!(params_of(&streamed), params_of(&whole), "{len}: size, checksum and type");
    }
}

/// A save with neither a sink nor `measure` would build the whole file in
/// memory; the package always sends one of them, so the engine refuses it.
#[test]
fn a_save_with_no_sink_is_refused() {
    let mut state = EngineState::new();
    let handle = open(&mut state);
    let request = frame("editorSave", &[("handleId", Field::Int(handle)), ("saveMode", Field::Int(0))]);
    let answer = run(&mut state, request, Vec::new(), Vec::new(), &CancelCheck::none());
    assert_eq!(answer[0], 0, "refused, not saved into memory");
    assert!(message_of(&answer).contains("a save writes into a sink or measures"), "{}", message_of(&answer));
    dispose(&mut state, handle);
}

/// A cancel during the checksum pass at the embed leaves the editor as it was.
#[test]
fn a_cancelled_embed_changes_nothing() {
    let data = noise(300_000, 4);
    let mut state = EngineState::new();
    let handle = open(&mut state);
    let (before, _) = (save(&mut state, handle, &CancelCheck::none()).1, ());
    let (source, _) = watched(&data);
    let request = frame(
        "editorMutate",
        &[("handleId", Field::Int(handle)), ("editOp", Field::Text("embedFile")), ("sourceLength", Field::Int(data.len() as i32)), ("name", Field::Text("data.bin"))],
    );
    let check = CancelCheck::tripping_at(2);
    let answer = run(&mut state, request, vec![source], Vec::new(), &check);
    assert_eq!(answer[0], 0, "the embed is refused");
    assert!(check.polls() >= 2, "the checksum polls for a cancel");
    assert!(save(&mut state, handle, &CancelCheck::none()).1 == before, "the editor is as it was");
}

// ── Images ───────────────────────────────────────────────────────────────

struct PngSpec {
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    palette: Option<Vec<u8>>,
    trns: Option<Vec<u8>>,
}

fn png_bytes(spec: &PngSpec) -> Vec<u8> {
    let samples = match spec.color {
        png::ColorType::Grayscale | png::ColorType::Indexed => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
    };
    let row_bytes = (spec.width as usize * samples * spec.depth as usize).div_ceil(8);
    let out = Collect::default();
    let mut encoder = png::Encoder::new(out.clone(), spec.width, spec.height);
    encoder.set_color(spec.color);
    encoder.set_depth(spec.depth);
    if let Some(palette) = &spec.palette {
        encoder.set_palette(palette.clone());
    }
    if let Some(trns) = &spec.trns {
        encoder.set_trns(trns.clone());
    }
    let mut writer = encoder.write_header().unwrap().into_stream_writer().unwrap();
    for y in 0..spec.height {
        let mut row = noise(row_bytes, 7 + u64::from(y));
        // Indexed rows must name palette entries that exist.
        if spec.color == png::ColorType::Indexed {
            let entries = spec.palette.as_ref().unwrap().len() / 3;
            if spec.depth == png::BitDepth::Eight {
                row.iter_mut().for_each(|b| *b %= entries as u8);
            } else {
                row.iter_mut().for_each(|b| *b = 0);
            }
        }
        writer.write_all(&row).unwrap();
    }
    writer.finish().unwrap();
    let bytes = out.0.lock().unwrap().clone();
    bytes
}

fn jpeg_bytes(color: jpeg_encoder::ColorType, progressive: bool) -> Vec<u8> {
    let per = match color {
        jpeg_encoder::ColorType::Luma => 1,
        jpeg_encoder::ColorType::Rgb => 3,
        _ => 4,
    };
    let (w, h) = (45u16, 33u16);
    let pixels = noise(w as usize * h as usize * per, 5);
    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, 90);
    encoder.set_progressive(progressive);
    encoder.encode(&pixels, w, h, color).unwrap();
    out
}

/// Every image XObject of `pdf` as (size, colour space, depth, decoded
/// samples), sorted.
fn images_of(pdf: &[u8]) -> Vec<(String, Vec<u8>)> {
    let doc = PdfDocument::from_bytes(pdf.to_vec()).unwrap();
    let mut found = Vec::new();
    for id in doc.all_object_ids() {
        let Ok(object) = doc.load_object(ObjectRef::new(id, 0)) else { continue };
        let Object::Stream { dict, .. } = &object else { continue };
        if dict.get("Subtype").and_then(Object::as_name) != Some("Image") {
            continue;
        }
        let key = |name: &str| format!("{:?}", dict.get(name));
        let description = format!("{} {} {} {} {}", key("Width"), key("Height"), key("ColorSpace"), key("BitsPerComponent"), key("Filter"));
        found.push((description, object.decode_stream_data().unwrap()));
    }
    found.sort();
    found
}

fn stamped_whole(image: &[u8]) -> Vec<u8> {
    let mut editor = open_editor_from_bytes(&one_page(), "stamp_whole");
    dispatch::edit_add_image_stamp(&mut editor, &[0], image.to_vec(), 10.0, 10.0, 80.0, 60.0, 1.0).unwrap();
    editor.save_to_bytes().unwrap()
}

fn stamped_through_ops(image: &[u8]) -> (Vec<u8>, bool, bool) {
    let mut state = EngineState::new();
    let handle = open(&mut state);
    let (source, dropped) = watched(image);
    let answer = stamp(&mut state, handle, image, source, &CancelCheck::none());
    assert_eq!(answer[0], 1, "the stamp is added: {}", message_of(&answer));
    let held_after_op = !dropped.load(Ordering::SeqCst);
    let (answer, bytes) = save(&mut state, handle, &CancelCheck::none());
    assert_eq!(answer[0], 1, "the editor saves: {}", message_of(&answer));
    dispose(&mut state, handle);
    (bytes, held_after_op, dropped.load(Ordering::SeqCst))
}

fn png_cases() -> Vec<(&'static str, PngSpec)> {
    use png::{BitDepth::*, ColorType::*};
    let palette: Vec<u8> = (0..16 * 3).map(|i| (i * 37 % 251) as u8).collect();
    let spec = |color, depth| PngSpec { width: 37, height: 29, color, depth, palette: None, trns: None };
    let mut cases = vec![
        ("gray8", spec(Grayscale, Eight)),
        ("gray16", spec(Grayscale, Sixteen)),
        ("gray1", spec(Grayscale, One)),
        ("graya8", spec(GrayscaleAlpha, Eight)),
        ("graya16", spec(GrayscaleAlpha, Sixteen)),
        ("rgb8", spec(Rgb, Eight)),
        ("rgb16", spec(Rgb, Sixteen)),
        ("rgba8", spec(Rgba, Eight)),
        ("rgba16", spec(Rgba, Sixteen)),
    ];
    let mut indexed = spec(Indexed, Eight);
    indexed.palette = Some(palette.clone());
    cases.push(("indexed8", indexed));
    let mut keyed = spec(Indexed, Eight);
    keyed.palette = Some(palette);
    keyed.trns = Some(vec![0, 90, 200]);
    cases.push(("indexed8+trns", keyed));
    cases
}

/// A PNG stamped from its source saves the pixels and the alpha the
/// whole-bytes path saves, in every colour type and depth, and the source is
/// held from the op to the dispose.
#[test]
fn a_png_stamp_saves_the_pixels_of_the_whole_bytes_path() {
    for (label, spec) in png_cases() {
        let image = png_bytes(&spec);
        let (streamed, held, released) = stamped_through_ops(&image);
        assert!(held, "{label}: the source is held after the op");
        assert!(released, "{label}: the dispose lets go of the source");
        let whole = stamped_whole(&image);
        let (got, want) = (images_of(&streamed), images_of(&whole));
        assert_eq!(got.len(), want.len(), "{label}: the images");
        for ((got_key, got_data), (want_key, want_data)) in got.iter().zip(&want) {
            assert_eq!(got_key.split(' ').take(4).collect::<Vec<_>>(), want_key.split(' ').take(4).collect::<Vec<_>>(), "{label}: the description");
            assert!(got_data == want_data, "{label}: the samples of {got_key}");
        }
    }
}

/// A JPEG stamped from its source is saved as its own bytes.
#[test]
fn a_jpeg_stamp_saves_its_own_bytes() {
    for (label, image) in [
        ("rgb", jpeg_bytes(jpeg_encoder::ColorType::Rgb, false)),
        ("gray", jpeg_bytes(jpeg_encoder::ColorType::Luma, false)),
        ("cmyk", jpeg_bytes(jpeg_encoder::ColorType::Cmyk, false)),
        ("progressive", jpeg_bytes(jpeg_encoder::ColorType::Rgb, true)),
    ] {
        let (streamed, held, released) = stamped_through_ops(&image);
        assert!(held && released, "{label}: the source is held from the op to the dispose");
        let got = images_of(&streamed);
        assert_eq!(got.len(), 1, "{label}");
        assert!(got[0].1 == image, "{label}: the stream is the file");
        assert!(images_of(&stamped_whole(&image))[0].1 == image, "{label}: today's path agrees");
    }
}

/// An Adam7 RGBA PNG of noise, written chunk by chunk here because the
/// encoder in use writes no interlaced images.
fn interlaced_rgba(width: u32, height: u32) -> Vec<u8> {
    const PASSES: [(u32, u32, u32, u32); 7] =
        [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];
    let mut scanlines = Vec::new();
    for (x0, y0, dx, dy) in PASSES {
        if x0 >= width || y0 >= height {
            continue;
        }
        let (pass_w, pass_h) = ((width + dx - 1 - x0) / dx, (height + dy - 1 - y0) / dy);
        for row in 0..pass_h {
            scanlines.push(0u8);
            scanlines.extend_from_slice(&noise(pass_w as usize * 4, 3 + u64::from(y0 + row * dy)));
        }
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&scanlines).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc = crc32fast::Hasher::new();
        crc.update(kind);
        crc.update(data);
        out.extend_from_slice(&crc.finalize().to_be_bytes());
    };
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 1]);
    chunk(b"IHDR", &header);
    chunk(b"IDAT", &compressed);
    chunk(b"IEND", &[]);
    out
}

/// An interlaced PNG is read whole and stamped as the whole-bytes path does.
#[test]
fn an_interlaced_png_stamps_as_it_does_today() {
    let image = interlaced_rgba(20, 20);
    let (streamed, _, _) = stamped_through_ops(&image);
    assert!(images_of(&streamed) == images_of(&stamped_whole(&image)));
}

/// Bytes that are no image are refused and the editor is as it was.
#[test]
fn a_stamp_of_bytes_that_are_no_image_changes_nothing() {
    let mut state = EngineState::new();
    let handle = open(&mut state);
    let before = save(&mut state, handle, &CancelCheck::none()).1;
    let junk = b"not an image at all".to_vec();
    let (source, _) = watched(&junk);
    let answer = stamp(&mut state, handle, &junk, source, &CancelCheck::none());
    assert_eq!(answer[0], 0);
    assert!(message_of(&answer).contains("invalid image"), "{}", message_of(&answer));
    assert!(save(&mut state, handle, &CancelCheck::none()).1 == before);
}

/// The save of a PNG stamp polls for a cancel at every row, and a cancelled
/// save fails.
#[test]
fn the_save_of_a_png_stamp_polls_for_a_cancel() {
    let image = png_bytes(&PngSpec { width: 64, height: 200, color: png::ColorType::Rgba, depth: png::BitDepth::Eight, palette: None, trns: None });
    let mut state = EngineState::new();
    let handle = open(&mut state);
    let (source, _) = watched(&image);
    assert_eq!(stamp(&mut state, handle, &image, source, &CancelCheck::none())[0], 1);
    let counting = CancelCheck::tripping_at(usize::MAX);
    assert_eq!(save(&mut state, handle, &counting).0[0], 1);
    assert!(counting.polls() >= 200 * 2, "{} polls for 200 rows and a mask", counting.polls());
    let (answer, _) = save(&mut state, handle, &CancelCheck::tripping_at(100));
    assert_eq!(answer[0], 0, "a cancelled save fails");
    assert_eq!(save(&mut state, handle, &CancelCheck::none()).0[0], 1, "the editor still saves");
}
