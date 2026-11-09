//! A builder image streams from its source: `builderAddImagePage` and the
//! `image` page op hold the source, and `builderSave` copies a JPEG from it
//! in pieces and inflates a PNG row by row, so the engine holds a row and a
//! chunk of the image whatever its size.
//!
//! Each proof drives the ops through the request door (`handle_request`) with
//! the source a lane hands it, and measures with the counting allocator
//! (`host::heap`). The pixels are compared with today's whole-image embedding
//! (`PageOp::Image`, which the Rust builder API keeps), read back from the
//! saved bytes.
//!
//! No Dart battery can observe this: a battery sees the saved bytes, and the
//! same bytes came out of the whole-image path; only the allocator's peak, the
//! drop of the source and the polls of the cancel check tell the two apart.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::cancel::CancelCheck;
use crate::host::binary_codec::Request;
use crate::host::bridge_api::{handle_request, BoxedReader, BoxedWriter};
use crate::host::dispatch::{self, PageOp};
use crate::host::engine_state::EngineState;
use crate::host::heap;
use crate::host::image_embed::ImageBytes;
use crate::object::{Object, ObjectRef};
use crate::writer::PageSize;
use crate::PdfDocument;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

enum Field<'a> {
    Int(i32),
    Real(f64),
    Bool(bool),
    Text(&'a str),
}

/// A request frame as the Dart codec lays it out.
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

/// The text of an error answer.
fn message_of(answer: &[u8]) -> String {
    let mut as_request = vec![0u8];
    as_request.extend_from_slice(&answer[2..]);
    Request::parse(&as_request)
        .ok()
        .and_then(|r| r.get_str("message").map(str::to_owned))
        .unwrap_or_default()
}

fn run(
    state: &mut EngineState,
    request: Vec<u8>,
    sources: Vec<BoxedReader>,
    sinks: Vec<BoxedWriter>,
    cancel: &CancelCheck,
) -> Vec<u8> {
    handle_request(state, &request, None, sources, sinks, cancel)
}

fn create(state: &mut EngineState) -> i32 {
    let answer = run(state, frame("builderCreate", &[]), Vec::new(), Vec::new(), &CancelCheck::none());
    assert_eq!(answer[0], 1, "the builder is created");
    let mut as_request = vec![0u8];
    as_request.extend_from_slice(&answer[1..]);
    Request::parse(&as_request).unwrap().get_i32("handleId").unwrap()
}

/// `builderAddImagePage` over `source`.
fn add_image_page(
    state: &mut EngineState,
    handle: i32,
    source: impl Read + Seek + Send + 'static,
    length: usize,
) -> Vec<u8> {
    let request = frame(
        "builderAddImagePage",
        &[
            ("handleId", Field::Int(handle)),
            ("sourceLength", Field::Int(length as i32)),
            ("fit", Field::Text("contain")),
            ("margin", Field::Real(0.0)),
        ],
    );
    run(state, request, vec![BoxedReader(Box::new(source))], Vec::new(), &CancelCheck::none())
}

/// A page of 200 x 100 points and `builderPageOp` `image` over `source` on it.
fn place_image(
    state: &mut EngineState,
    handle: i32,
    source: impl Read + Seek + Send + 'static,
    length: usize,
) -> Vec<u8> {
    let page = frame(
        "builderAddPage",
        &[("handleId", Field::Int(handle)), ("width", Field::Real(200.0)), ("height", Field::Real(100.0))],
    );
    assert_eq!(run(state, page, Vec::new(), Vec::new(), &CancelCheck::none())[0], 1);
    let request = frame(
        "builderPageOp",
        &[
            ("handleId", Field::Int(handle)),
            ("pageOp", Field::Text("image")),
            ("sourceLength", Field::Int(length as i32)),
            ("x", Field::Real(10.0)),
            ("y", Field::Real(10.0)),
            ("width", Field::Real(180.0)),
            ("height", Field::Real(80.0)),
            ("altText", Field::Text("a picture")),
        ],
    );
    run(state, request, vec![BoxedReader(Box::new(source))], Vec::new(), &CancelCheck::none())
}

/// `builderSave` into `sink` under `cancel`.
fn save(state: &mut EngineState, handle: i32, sink: BoxedWriter, cancel: &CancelCheck) -> Vec<u8> {
    let request = frame("builderSave", &[("handleId", Field::Int(handle)), ("deflate", Field::Bool(true))]);
    run(state, request, Vec::new(), vec![sink], cancel)
}

/// A sink that collects what it is given.
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

/// A sink that counts and forgets.
#[derive(Clone, Default)]
struct Count(Arc<Mutex<usize>>);

impl Write for Count {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        *self.0.lock().unwrap() += buf.len();
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A source that says when it is dropped: the lane frees a source's channel
/// when the engine lets go of the reader.
struct Watched<R> {
    inner: R,
    dropped: Arc<AtomicBool>,
}

impl<R> Drop for Watched<R> {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

impl<R: Read> Read for Watched<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<R: Seek> Seek for Watched<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

fn watched(bytes: Vec<u8>) -> (Watched<std::io::Cursor<Vec<u8>>>, Arc<AtomicBool>) {
    let dropped = Arc::new(AtomicBool::new(false));
    (Watched { inner: std::io::Cursor::new(bytes), dropped: dropped.clone() }, dropped)
}

/// A scratch file removed when it goes out of scope.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        Scratch(std::env::temp_dir().join(format!("builder_image_stream-{}-{name}", std::process::id())))
    }
    fn open(&self) -> File {
        File::open(&self.0).unwrap()
    }
    fn len(&self) -> usize {
        std::fs::metadata(&self.0).unwrap().len() as usize
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The next word of an xorshift stream: the noise no codec shrinks.
fn noise(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn noise_row(seed: u64, y: u32, bytes: usize) -> Vec<u8> {
    let mut state = seed ^ (u64::from(y) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (0..bytes).map(|_| (noise(&mut state) >> 24) as u8).collect()
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

impl PngSpec {
    fn new(width: u32, height: u32, color: png::ColorType, depth: png::BitDepth) -> Self {
        PngSpec { width, height, color, depth, palette: None, trns: None }
    }

    fn row_bytes(&self) -> usize {
        let samples = match self.color {
            png::ColorType::Grayscale | png::ColorType::Indexed => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
        };
        (self.width as usize * samples * self.depth as usize).div_ceil(8)
    }
}

/// The PNG of `spec` filled with noise, written row by row to `out`.
fn write_png(spec: &PngSpec, out: impl Write + 'static) {
    let mut encoder = png::Encoder::new(out, spec.width, spec.height);
    encoder.set_color(spec.color);
    encoder.set_depth(spec.depth);
    encoder.set_compression(png::Compression::Fast);
    if let Some(palette) = &spec.palette {
        encoder.set_palette(palette.clone());
    }
    if let Some(trns) = &spec.trns {
        encoder.set_trns(trns.clone());
    }
    let mut writer = encoder.write_header().unwrap().into_stream_writer().unwrap();
    for y in 0..spec.height {
        writer.write_all(&noise_row(7, y, spec.row_bytes())).unwrap();
    }
    writer.finish().unwrap();
}

fn png_bytes(spec: &PngSpec) -> Vec<u8> {
    let out = Collect::default();
    write_png(spec, out.clone());
    let bytes = out.0.lock().unwrap().clone();
    bytes
}

/// `image` rows of a JPEG of noise, handed to the encoder one row at a time.
struct NoiseJpeg {
    width: u16,
    height: u16,
}

impl jpeg_encoder::ImageBuffer for NoiseJpeg {
    fn get_jpeg_color_type(&self) -> jpeg_encoder::JpegColorType {
        jpeg_encoder::JpegColorType::Ycbcr
    }
    fn width(&self) -> u16 {
        self.width
    }
    fn height(&self) -> u16 {
        self.height
    }
    fn fill_buffers(&self, y: u16, buffers: &mut [Vec<u8>; 4]) {
        let row = noise_row(11, u32::from(y), self.width as usize * 3);
        for pixel in row.chunks_exact(3) {
            buffers[0].push(pixel[0]);
            buffers[1].push(pixel[1]);
            buffers[2].push(pixel[2]);
        }
    }
}

fn write_noise_jpeg(width: u16, height: u16, out: impl Write) {
    let mut encoder = jpeg_encoder::Encoder::new(out, 100);
    encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_4_4);
    encoder.encode_image(NoiseJpeg { width, height }).unwrap();
}

fn jpeg_of(color: jpeg_encoder::ColorType, width: u16, height: u16, progressive: bool) -> Vec<u8> {
    let bytes_per_pixel = match color {
        jpeg_encoder::ColorType::Luma => 1,
        jpeg_encoder::ColorType::Rgb => 3,
        jpeg_encoder::ColorType::Cmyk => 4,
        other => panic!("no fixture for {other:?}"),
    };
    let mut pixels = Vec::new();
    for y in 0..u32::from(height) {
        pixels.extend_from_slice(&noise_row(5, y, width as usize * bytes_per_pixel));
    }
    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, 90);
    encoder.set_progressive(progressive);
    encoder.encode(&pixels, width, height, color).unwrap();
    out
}

// ── Reading a saved file back ────────────────────────────────────────────

struct Embedded {
    /// The keys that describe the pixels, as text, in key order.
    description: Vec<String>,
    /// The stream before its filters run.
    raw: Vec<u8>,
    /// The stream after its filters (and predictor) ran.
    decoded: Vec<u8>,
    /// The `/SMask` image's `description`, `raw` and `decoded`.
    mask: Option<Box<Embedded>>,
}

/// An object as text with the keys of every dictionary in order.
fn show(object: &Object) -> String {
    match object {
        Object::Dictionary(dict) => {
            let mut keys: Vec<&String> = dict.keys().collect();
            keys.sort();
            let entries: Vec<String> = keys.into_iter().map(|k| format!("{k}: {}", show(&dict[k]))).collect();
            format!("<<{}>>", entries.join(", "))
        },
        other => format!("{other:?}"),
    }
}

fn describe(dict: &HashMap<String, Object>) -> Vec<String> {
    let mut keys: Vec<&String> = dict
        .keys()
        .filter(|k| {
            ["Type", "Subtype", "Width", "Height", "ColorSpace", "BitsPerComponent", "Filter", "DecodeParms"]
                .contains(&k.as_str())
        })
        .collect();
    keys.sort();
    keys.into_iter().map(|k| format!("{k}={}", show(&dict[k]))).collect()
}

fn embedded(doc: &PdfDocument, object: &Object) -> Embedded {
    let Object::Stream { dict, body } = object else { panic!("an image is a stream: {object:?}") };
    let mask = dict.get("SMask").and_then(Object::as_reference).map(|r: ObjectRef| {
        let loaded = doc.load_object(r).unwrap();
        Box::new(embedded(doc, &loaded))
    });
    Embedded {
        description: describe(dict),
        raw: body.read_whole_raw().unwrap(),
        decoded: object.decode_stream_data().unwrap(),
        mask,
    }
}

/// Every image XObject of `pdf`, by object number.
fn images_of(pdf: &[u8]) -> Vec<Embedded> {
    let doc = PdfDocument::from_bytes(pdf.to_vec()).unwrap();
    let mut found = Vec::new();
    for id in doc.all_object_ids() {
        let Ok(object) = doc.load_object(ObjectRef::new(id, 0)) else { continue };
        let is_image = matches!(&object, Object::Stream { dict, .. }
            if dict.get("Subtype").and_then(Object::as_name) == Some("Image")
                && dict.get("Type").and_then(Object::as_name) == Some("XObject"));
        // The soft mask is an image too; it is read through its owner.
        let is_mask = doc.all_object_ids().iter().any(|other| {
            matches!(doc.load_object(ObjectRef::new(*other, 0)),
                Ok(Object::Stream { dict, .. }) if dict.get("SMask").and_then(Object::as_reference).map(|r| r.id) == Some(id))
        });
        if is_image && !is_mask {
            found.push(embedded(&doc, &object));
        }
    }
    found
}

/// What today's path saves for `image` on a page of its own size: the whole
/// bytes through `ImageData`.
fn today(image: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut builder = dispatch::builder_new();
    builder.begin_page(PageSize::Custom(width as f32, height as f32));
    dispatch::apply_page_op(
        &mut builder,
        PageOp::Image { data: ImageBytes::Whole(image.to_vec()), x: 0.0, y: 0.0, w: width as f32, h: height as f32, alt: String::new() },
    )
    .unwrap();
    builder.end_page();
    dispatch::builder_save(builder.compress_streams(true)).unwrap()
}

/// What the ops save for `image`: one image page. Also whether the source was
/// still held when the add returned, and whether it was let go of once the
/// save was over.
fn through_ops(image: &[u8]) -> (Vec<u8>, bool, bool) {
    let mut state = EngineState::new();
    let handle = create(&mut state);
    let (source, dropped) = watched(image.to_vec());
    let answer = add_image_page(&mut state, handle, source, image.len());
    assert_eq!(answer[0], 1, "the page is added: {}", message_of(&answer));
    let held_after_add = !dropped.load(Ordering::SeqCst);
    let collected = Collect::default();
    let answer = save(&mut state, handle, BoxedWriter::new(collected.clone()), &CancelCheck::none());
    assert_eq!(answer[0], 1, "the builder saves: {}", message_of(&answer));
    let bytes = collected.0.lock().unwrap().clone();
    (bytes, held_after_add, dropped.load(Ordering::SeqCst))
}

// ── Proofs ───────────────────────────────────────────────────────────────

/// A JPEG is embedded as its own bytes under `DCTDecode`, whatever its colour
/// model and scan order, and the engine holds its source from the add to the
/// save. No Dart battery sees it: a battery reads the saved file, which the
/// whole-image path wrote byte for byte too, and the drop of a source is not
/// on the wire.
#[test]
fn a_jpeg_is_embedded_byte_for_byte_from_its_source() {
    let fixtures = [
        ("rgb", jpeg_of(jpeg_encoder::ColorType::Rgb, 97, 61, false), 97, 61),
        ("gray", jpeg_of(jpeg_encoder::ColorType::Luma, 33, 70, false), 33, 70),
        ("cmyk", jpeg_of(jpeg_encoder::ColorType::Cmyk, 40, 24, false), 40, 24),
        ("progressive", jpeg_of(jpeg_encoder::ColorType::Rgb, 64, 48, true), 64, 48),
    ];
    for (name, jpeg, w, h) in fixtures {
        let reference = images_of(&today(&jpeg, w, h));
        let (saved, held_after_add, dropped) = through_ops(&jpeg);
        let streamed = images_of(&saved);
        assert_eq!(streamed.len(), 1, "{name}: one image");
        assert_eq!(streamed[0].description, reference[0].description, "{name}: the same description");
        assert_eq!(streamed[0].raw, jpeg, "{name}: the stream is the source's bytes");
        assert_eq!(streamed[0].raw, reference[0].raw, "{name}: today's bytes");
        assert!(streamed[0].mask.is_none(), "{name}: a JPEG has no mask");
        assert!(held_after_add, "{name}: the add let go of its source");
        assert!(dropped, "{name}: the source is released once the save is over");
    }
}

/// The source outlives the add: the lane holds its channel until the engine
/// lets go of the reader, so a builder that read the image at the add would
/// have dropped it by now. No Dart battery sees it: the channel's release is
/// the lane's, and only the engine's own drop of the reader says when it let go.
#[test]
fn the_source_is_held_from_the_add_to_the_save() {
    let jpeg = jpeg_of(jpeg_encoder::ColorType::Rgb, 32, 32, false);
    let png = png_bytes(&PngSpec::new(16, 16, png::ColorType::Rgba, png::BitDepth::Eight));
    for (name, image) in [("jpeg", jpeg), ("png", png)] {
        for page_op in [false, true] {
            let mut state = EngineState::new();
            let handle = create(&mut state);
            let (source, dropped) = watched(image.clone());
            let answer = if page_op {
                place_image(&mut state, handle, source, image.len())
            } else {
                add_image_page(&mut state, handle, source, image.len())
            };
            assert_eq!(answer[0], 1, "{name}: added: {}", message_of(&answer));
            assert!(!dropped.load(Ordering::SeqCst), "{name} (page op {page_op}): the add let go of its source");
            let sink = Collect::default();
            let answer = save(&mut state, handle, BoxedWriter::new(sink), &CancelCheck::none());
            assert_eq!(answer[0], 1, "{name}: saved: {}", message_of(&answer));
            assert!(dropped.load(Ordering::SeqCst), "{name} (page op {page_op}): the save held its source");
        }
    }
}

/// A builder disposed before its save lets go of every source it held. No Dart
/// battery sees it: the reader is the engine's, and a lane that never frees its
/// channel looks the same from Dart as one that does until the lane is full.
#[test]
fn a_disposed_builder_releases_its_sources() {
    let png = png_bytes(&PngSpec::new(16, 16, png::ColorType::Rgb, png::BitDepth::Eight));
    let mut state = EngineState::new();
    let handle = create(&mut state);
    let (source, dropped) = watched(png.clone());
    assert_eq!(add_image_page(&mut state, handle, source, png.len())[0], 1);
    assert!(!dropped.load(Ordering::SeqCst), "held after the add");
    let answer = run(&mut state, frame("builderDispose", &[("handleId", Field::Int(handle))]), Vec::new(), Vec::new(), &CancelCheck::none());
    assert_eq!(answer[0], 1);
    assert!(dropped.load(Ordering::SeqCst), "released with the builder");
}

fn spec_set() -> Vec<(&'static str, PngSpec)> {
    use png::{BitDepth::*, ColorType::*};
    let palette: Vec<u8> = (0..16 * 3).map(|i| (i * 37 % 251) as u8).collect();
    let mut cases = vec![
        ("gray8", PngSpec::new(37, 29, Grayscale, Eight)),
        ("gray16", PngSpec::new(37, 29, Grayscale, Sixteen)),
        ("gray1", PngSpec::new(37, 29, Grayscale, One)),
        ("gray2", PngSpec::new(37, 29, Grayscale, Two)),
        ("gray4", PngSpec::new(37, 29, Grayscale, Four)),
        ("graya8", PngSpec::new(37, 29, GrayscaleAlpha, Eight)),
        ("graya16", PngSpec::new(37, 29, GrayscaleAlpha, Sixteen)),
        ("rgb8", PngSpec::new(37, 29, Rgb, Eight)),
        ("rgb16", PngSpec::new(37, 29, Rgb, Sixteen)),
        ("rgba8", PngSpec::new(37, 29, Rgba, Eight)),
        ("rgba16", PngSpec::new(37, 29, Rgba, Sixteen)),
    ];
    for depth in [One, Two, Four, Eight] {
        let entries = match depth {
            One => 2,
            Two => 4,
            Four => 16,
            _ => 16,
        };
        let mut plain = PngSpec::new(37, 29, Indexed, depth);
        plain.palette = Some(palette[..entries * 3].to_vec());
        cases.push((
            match depth {
                One => "indexed1",
                Two => "indexed2",
                Four => "indexed4",
                _ => "indexed8",
            },
            plain,
        ));
        let mut keyed = PngSpec::new(37, 29, Indexed, depth);
        keyed.palette = Some(palette[..entries * 3].to_vec());
        keyed.trns = Some((0..entries.min(5)).map(|i| (i * 60) as u8).collect());
        cases.push((
            match depth {
                One => "indexed1+trns",
                Two => "indexed2+trns",
                Four => "indexed4+trns",
                _ => "indexed8+trns",
            },
            keyed,
        ));
    }
    let mut gray_key = PngSpec::new(37, 29, Grayscale, Eight);
    gray_key.trns = Some(vec![0, 90]);
    cases.push(("gray8+trns", gray_key));
    let mut rgb_key = PngSpec::new(37, 29, Rgb, Eight);
    rgb_key.trns = Some(vec![0, 10, 0, 20, 0, 30]);
    cases.push(("rgb8+trns", rgb_key));
    cases
}

/// A PNG decodes to the pixels and the alpha today's embedding gives it, in
/// every colour type and depth, and its description (size, colour space,
/// predictor) is the same; the source is held to the save. No Dart battery
/// sees it: it compares two engine paths, the streamed one and the whole-image
/// one that only the Rust builder API still takes.
#[test]
fn a_png_embeds_the_pixels_and_the_alpha_of_todays_path() {
    for (name, spec) in spec_set() {
        let png = png_bytes(&spec);
        let reference = images_of(&today(&png, spec.width, spec.height));
        let (saved, held_after_add, dropped) = through_ops(&png);
        let streamed = images_of(&saved);
        assert_eq!(streamed.len(), 1, "{name}: one image");
        let (got, want) = (&streamed[0], &reference[0]);
        assert_eq!(got.description, want.description, "{name}: the same description");
        assert_eq!(got.decoded, want.decoded, "{name}: the same pixels");
        match (&got.mask, &want.mask) {
            (None, None) => {},
            (Some(got), Some(want)) => {
                assert_eq!(got.description, want.description, "{name}: the same mask description");
                assert_eq!(got.decoded, want.decoded, "{name}: the same alpha");
            },
            _ => panic!("{name}: the soft mask differs from today's"),
        }
        assert!(held_after_add, "{name}: the add let go of its source");
        assert!(dropped, "{name}: the source is released after the save");
    }
}

/// An interlaced PNG, which the decoder can only produce whole, embeds as it
/// does today. No Dart battery sees it: the fallback is chosen from the IHDR
/// inside the engine, and only a comparison with the whole-image path tells it
/// from a streamed embedding.
#[test]
fn an_interlaced_png_embeds_as_it_does_today() {
    let png = interlaced_rgba(41, 33);
    let reference = images_of(&today(&png, 41, 33));
    let (saved, _, _) = through_ops(&png);
    let streamed = images_of(&saved);
    assert_eq!(streamed[0].description, reference[0].description);
    assert_eq!(streamed[0].decoded, reference[0].decoded, "the same pixels");
    assert_eq!(
        streamed[0].mask.as_ref().unwrap().decoded,
        reference[0].mask.as_ref().unwrap().decoded,
        "the same alpha"
    );
}

/// An Adam7 RGBA PNG of noise, written chunk by chunk here because the
/// encoder in use writes no interlaced images.
fn interlaced_rgba(width: u32, height: u32) -> Vec<u8> {
    const PASSES: [(u32, u32, u32, u32); 7] =
        [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];
    let mut scanlines = Vec::new();
    for (x0, y0, dx, dy) in PASSES {
        let (pass_w, pass_h) = ((width + dx - 1 - x0) / dx, (height + dy - 1 - y0) / dy);
        if x0 >= width || y0 >= height {
            continue;
        }
        for row in 0..pass_h {
            scanlines.push(0u8);
            scanlines.extend_from_slice(&noise_row(3, y0 + row * dy, pass_w as usize * 4));
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

/// Bytes that are not an image are refused at the add, as they are today, and
/// a truncated PNG fails the add or the save with a typed error, never a
/// panic and never a file with a broken image in it. No Dart battery sees it:
/// a battery gets the error either way, and not which op raised it.
#[test]
fn bytes_that_are_not_an_image_are_refused() {
    let mut state = EngineState::new();
    let handle = create(&mut state);
    let junk = vec![7u8; 100];
    let answer = add_image_page(&mut state, handle, std::io::Cursor::new(junk.clone()), junk.len());
    assert_eq!(answer[0], 0);
    assert!(message_of(&answer).contains("not a readable PNG or JPEG"), "{}", message_of(&answer));

    let png = png_bytes(&PngSpec::new(64, 64, png::ColorType::Rgb, png::BitDepth::Eight));
    let cut = png[..png.len() / 2].to_vec();
    let handle = create(&mut state);
    let added = add_image_page(&mut state, handle, std::io::Cursor::new(cut.clone()), cut.len());
    let saved = save(&mut state, handle, BoxedWriter::new(Collect::default()), &CancelCheck::none());
    assert!(added[0] == 0 || saved[0] == 0, "a truncated PNG was embedded");
}

/// What the engine's thread holds over its baseline while it adds the image
/// at `path` and saves it to a sink that forgets.
fn peak_of_embedding(path: &Scratch, jpeg_or_png: &str) -> usize {
    let mut state = EngineState::new();
    let handle = create(&mut state);
    let length = path.len();
    let sink = Count::default();
    let (answer, grew) = heap::growth(|| {
        let added = add_image_page(&mut state, handle, path.open(), length);
        assert_eq!(added[0], 1, "{jpeg_or_png}: added: {}", message_of(&added));
        save(&mut state, handle, BoxedWriter::new(sink.clone()), &CancelCheck::none())
    });
    assert_eq!(answer[0], 1, "{jpeg_or_png}: saved: {}", message_of(&answer));
    assert!(*sink.0.lock().unwrap() >= length / 2, "{jpeg_or_png}: the file was written");
    grew
}

/// A PNG of noise sixteen times the height embeds in the same few hundred KiB:
/// a row at a time, whatever the height. No Dart battery sees it: the
/// engine's heap is not on the wire, and tool/memprobe measures the process,
/// not one op on one thread.
#[test]
fn a_png_embeds_in_memory_that_does_not_grow_with_its_height() {
    let write = |name: &str, height: u32| {
        let scratch = Scratch::new(name);
        let spec = PngSpec::new(1024, height, png::ColorType::Rgba, png::BitDepth::Eight);
        write_png(&spec, BufWriter::new(File::create(&scratch.0).unwrap()));
        scratch
    };
    let (short, tall) = (write("short.png", 128), write("tall.png", 2048));
    let (a, b) = (peak_of_embedding(&short, "png"), peak_of_embedding(&tall, "png"));
    assert!(tall.len() > 7 * MIB, "the tall image is {} bytes", tall.len());
    assert!(b < 2 * MIB, "a {} byte PNG took {b} bytes to embed", tall.len());
    assert!(b <= a + 256 * KIB, "{b} bytes for 2048 rows against {a} for 128");
}

/// The same for a JPEG: its bytes pass from the source to the sink in pieces.
/// No Dart battery sees it, for the reason the PNG proof gives.
#[test]
fn a_jpeg_embeds_in_memory_that_does_not_grow_with_its_height() {
    let write = |name: &str, height: u16| {
        let scratch = Scratch::new(name);
        write_noise_jpeg(1024, height, BufWriter::new(File::create(&scratch.0).unwrap()));
        scratch
    };
    let (short, tall) = (write("short.jpg", 128), write("tall.jpg", 2048));
    let (a, b) = (peak_of_embedding(&short, "jpeg"), peak_of_embedding(&tall, "jpeg"));
    assert!(tall.len() > 4 * MIB, "the tall image is {} bytes", tall.len());
    assert!(b < MIB, "a {} byte JPEG took {b} bytes to embed", tall.len());
    assert!(b <= a + 128 * KIB, "{b} bytes for 2048 rows against {a} for 128");
}

/// The save polls for a cancel once per row of a PNG's colour pass and again
/// per row of its alpha pass, and a cancel stops it within a row. No Dart
/// battery sees it: a cancel timed from Dart lands where the timer says, and
/// only the count of polls tells a loop that polls from one that does not.
#[test]
fn the_save_of_a_png_polls_for_a_cancel_at_every_row() {
    let rows = 3000;
    let spec = PngSpec::new(8, rows, png::ColorType::Rgba, png::BitDepth::Eight);
    let png = png_bytes(&spec);

    let embed = |check: &CancelCheck| {
        let mut state = EngineState::new();
        let handle = create(&mut state);
        let added = add_image_page(&mut state, handle, std::io::Cursor::new(png.clone()), png.len());
        assert_eq!(added[0], 1, "{}", message_of(&added));
        save(&mut state, handle, BoxedWriter::new(Collect::default()), check)
    };

    let counting = CancelCheck::tripping_at(usize::MAX);
    assert_eq!(embed(&counting)[0], 1);
    assert!(counting.polls() >= 2 * rows as usize, "{} polls for {rows} rows in two passes", counting.polls());

    let tripping = CancelCheck::tripping_at(rows as usize / 2);
    let answer = embed(&tripping);
    assert_eq!(answer[0], 0, "the save was cancelled");
    assert!(message_of(&answer).contains("cancelled"), "{}", message_of(&answer));
    assert!(tripping.polls() <= rows as usize / 2 + 8, "{} polls after the cancel", tripping.polls());
}

/// The save of a JPEG polls once per piece it copies. No Dart battery sees it,
/// for the reason the PNG proof gives.
#[test]
fn the_save_of_a_jpeg_polls_for_a_cancel_at_every_piece() {
    let scratch = Scratch::new("polls.jpg");
    write_noise_jpeg(512, 1024, BufWriter::new(File::create(&scratch.0).unwrap()));
    let pieces = scratch.len() / (64 * KIB);
    assert!(pieces >= 8, "the fixture is {} bytes", scratch.len());

    let embed = |check: &CancelCheck| {
        let mut state = EngineState::new();
        let handle = create(&mut state);
        let added = add_image_page(&mut state, handle, scratch.open(), scratch.len());
        assert_eq!(added[0], 1, "{}", message_of(&added));
        save(&mut state, handle, BoxedWriter::new(Count::default()), check)
    };
    let counting = CancelCheck::tripping_at(usize::MAX);
    assert_eq!(embed(&counting)[0], 1);
    assert!(counting.polls() >= pieces, "{} polls for {pieces} pieces", counting.polls());

    let tripping = CancelCheck::tripping_at(4);
    let answer = embed(&tripping);
    assert_eq!(answer[0], 0, "the save was cancelled");
    assert!(tripping.polls() <= 4 + 2, "{} polls after the cancel", tripping.polls());
}

// ── Interlaced PNG ───────────────────────────────────────────────────────

/// The Adam7 passes: x offset, y offset, x step, y step.
const ADAM7: [(u32, u32, u32, u32); 7] =
    [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];

/// The interlaced PNG of `spec`: noise in every pass, written chunk by chunk
/// because the encoder in use writes no interlaced images. A pass that no
/// pixel of a small image falls in has no rows. Each row carries the filter
/// `y % 5`, so the unfiltering of a pass is exercised too: the filtered bytes
/// are what the decoder would unfilter, and noise is its own reconstruction,
/// so the row written is the raw row filtered by `filter`.
fn interlaced_png(spec: &PngSpec) -> Vec<u8> {
    let samples = match spec.color {
        png::ColorType::Grayscale | png::ColorType::Indexed => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
    };
    let bits_per_pixel = samples * spec.depth as usize;
    let bpp = bits_per_pixel.div_ceil(8);
    let mut scanlines = Vec::new();
    for (pass, (x0, y0, dx, dy)) in ADAM7.into_iter().enumerate() {
        if x0 >= spec.width || y0 >= spec.height {
            continue;
        }
        let pass_w = (spec.width + dx - 1 - x0) / dx;
        let pass_h = (spec.height + dy - 1 - y0) / dy;
        let row_bytes = (pass_w as usize * bits_per_pixel).div_ceil(8);
        let mut above = vec![0u8; row_bytes];
        for line in 0..pass_h {
            let raw = noise_row(3 + pass as u64, line, row_bytes);
            let filter = (line % 5) as u8;
            scanlines.push(filter);
            for (i, &x) in raw.iter().enumerate() {
                let left = if i >= bpp { raw[i - bpp] } else { 0 };
                let up = above[i];
                let up_left = if i >= bpp { above[i - bpp] } else { 0 };
                scanlines.push(match filter {
                    0 => x,
                    1 => x.wrapping_sub(left),
                    2 => x.wrapping_sub(up),
                    3 => x.wrapping_sub(((u16::from(left) + u16::from(up)) / 2) as u8),
                    _ => {
                        let p = i16::from(left) + i16::from(up) - i16::from(up_left);
                        let (pa, pb, pc) =
                            ((p - i16::from(left)).abs(), (p - i16::from(up)).abs(), (p - i16::from(up_left)).abs());
                        let paeth = if pa <= pb && pa <= pc { left } else if pb <= pc { up } else { up_left };
                        x.wrapping_sub(paeth)
                    },
                });
            }
            above = raw;
        }
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
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
    let colour_type = match spec.color {
        png::ColorType::Grayscale => 0,
        png::ColorType::Rgb => 2,
        png::ColorType::Indexed => 3,
        png::ColorType::GrayscaleAlpha => 4,
        png::ColorType::Rgba => 6,
    };
    let mut header = Vec::new();
    header.extend_from_slice(&spec.width.to_be_bytes());
    header.extend_from_slice(&spec.height.to_be_bytes());
    header.extend_from_slice(&[spec.depth as u8, colour_type, 0, 0, 1]);
    chunk(b"IHDR", &header);
    if let Some(palette) = &spec.palette {
        chunk(b"PLTE", palette);
    }
    if let Some(trns) = &spec.trns {
        chunk(b"tRNS", trns);
    }
    // Two IDATs, so a pass can begin and end inside a chunk and across one.
    let (first, second) = compressed.split_at(compressed.len() / 3);
    chunk(b"IDAT", first);
    chunk(b"IDAT", second);
    chunk(b"IEND", &[]);
    out
}

/// An interlaced PNG embeds the pixels and the alpha that today's whole read
/// of it gives, in every colour type and depth, in sizes that leave a pass
/// without a pixel, a pass of one pixel and a row narrower than a byte. No
/// Dart battery sees it: it compares two engine paths, the one that decodes
/// the passes in step and the whole-image one that only the Rust builder API
/// still takes.
#[test]
fn an_interlaced_png_of_each_colour_type_embeds_the_pixels_and_the_alpha_of_todays_path() {
    for (name, mut spec) in spec_set() {
        for (width, height) in [(37, 29), (1, 1), (3, 2), (8, 8), (9, 17), (1, 40)] {
            spec.width = width;
            spec.height = height;
            let png = interlaced_png(&spec);
            let reference = images_of(&today(&png, width, height));
            let (saved, _, dropped) = through_ops(&png);
            let streamed = images_of(&saved);
            let what = format!("{name} {width}x{height}");
            assert_eq!(streamed.len(), 1, "{what}: one image");
            let (got, want) = (&streamed[0], &reference[0]);
            assert_eq!(got.description, want.description, "{what}: the same description");
            assert!(got.decoded == want.decoded, "{what}: the same pixels");
            match (&got.mask, &want.mask) {
                (None, None) => {},
                (Some(got), Some(want)) => {
                    assert_eq!(got.description, want.description, "{what}: the same mask description");
                    assert!(got.decoded == want.decoded, "{what}: the same alpha");
                },
                _ => panic!("{what}: the soft mask differs from today's"),
            }
            assert!(dropped, "{what}: the source is released after the save");
        }
    }
}

/// An interlaced PNG of noise of 16 MiB embeds in the memory one of 8 MiB
/// does: the passes are read in step a row at a time, whatever the height,
/// and the image is not read whole. The seven decoders cost a few hundred KiB
/// each, a floor that the first rows of a short image do not yet reach, so
/// both heights are above it. No Dart battery sees it: the engine's heap is
/// not on the wire, and tool/memprobe measures the process, not one op on one
/// thread.
#[test]
fn an_interlaced_png_embeds_in_memory_that_does_not_grow_with_its_height() {
    let write = |name: &str, height: u32| {
        let scratch = Scratch::new(name);
        let spec = PngSpec::new(1024, height, png::ColorType::Rgba, png::BitDepth::Eight);
        std::fs::write(&scratch.0, interlaced_png(&spec)).unwrap();
        scratch
    };
    let (short, tall) = (write("short-adam7.png", 2048), write("tall-adam7.png", 4096));
    let (a, b) = (peak_of_embedding(&short, "interlaced png"), peak_of_embedding(&tall, "interlaced png"));
    assert!(tall.len() > 16 * MIB, "the tall image is {} bytes", tall.len());
    assert!(b < 3 * MIB, "a {} byte interlaced PNG took {b} bytes to embed", tall.len());
    assert!(b <= a + 256 * KIB, "{b} bytes for 4096 rows against {a} for 2048");
}
