//! Memory wall probe for the pdf_manipulator host layer.
//!
//! Generates two inputs once (unmeasured), then runs each wire step on
//! each of them through pdf_oxide's `test-support` probe door, tracking
//! the allocator's high-water mark over the allocation level the
//! process already holds when the step starts. One line per step:
//! `<op> <input> peak_over_baseline=<bytes> limit=<bytes> ok|FAIL wall_ms=<n>`.
//! Exits 1 on any FAIL, and removes its temp dir at the end.

mod alloc;
mod wire;

use std::fs::File;
use std::io::Write;
use std::path::Path;

const MIB: usize = 1024 * 1024;

/// Each step's ceiling: today's measured peak rounded up to 16 MiB, so a
/// change that holds more memory fails `make check`. A step whose target
/// is lower names the target and the task that brings it there; when that
/// task lands, its limits drop to the target in the same commit.
fn limit(op: &str, input: &str) -> usize {
    match (op, input) {
        // Streaming today; the target is the ceiling.
        ("incrementalSave" | "sign", "many") => 16 * MIB,
        ("open" | "incrementalSave" | "sign", "huge") => 16 * MIB,
        ("extractPages" | "fullRewriteSave" | "compress", "huge") => 16 * MIB,
        ("open" | "extractPages" | "fullRewriteSave" | "compress", "many") => 16 * MIB,
        ("merge", "many" | "huge") => 16 * MIB,
        ("extractAfterMerge", "many") => 16 * MIB,
        ("extractAll", "many" | "huge") => 16 * MIB,
        // The text is read as operators, one token at a time.
        ("extractAll", "content-fl" | "content-a85" | "content-lzw" | "forms") => 16 * MIB,
        // Rulings are quantised as they are painted and dropped; the grid they
        // make is bounded by the page, never by the paths.
        ("extractAll", "paths") => 16 * MIB,
        // The page's operators are read once per band and its rows go to the
        // PNG encoder as they are rastered: a band, three rows and one data
        // frame are all the host holds.
        ("render", "content-fl" | "content-a85" | "content-lzw" | "forms") => 16 * MIB,
        // A 2000 pt square at 300 dpi is 69 million pixels, 278 MiB of raster.
        ("render", "bigpage-300dpi") => 16 * MIB,
        // A 60 MiB image is read through its filters a strip of rows at a
        // time and resampled one row at a time to its 1275 x 1650 device size.
        ("render", "huge") => 16 * MIB,
        // The ruled exception: a progressive JPEG is decoded whole, by the
        // inverse DCT at the reduction that covers the footprint, so the host
        // holds its coefficients and that one raster. The ceiling is the
        // measured peak (53.3 MiB; the renderer before this held 170 MiB);
        // the target is a decoder that streams progressive scans.
        ("render", "hugeJpeg") => 56 * MIB,
        ("fullRewriteSave", "encrypted") => 16 * MIB,
        // The resource prune reads every content once, as operators.
        ("compress", "content-fl" | "content-a85" | "content-lzw" | "forms" | "encrypted") => {
            16 * MIB
        },
        _ => panic!("no memory limit for {op} on {input}: add one to limit()"),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("memprobe: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let failed = std::cell::Cell::new(false);

    let dir = std::env::temp_dir().join(format!("memprobe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;

    // ── generation phase: not measured ──
    let many = dir.join("many.pdf");
    let huge = dir.join("huge.pdf");
    eprintln!("memprobe: generating inputs ...");
    generate_many(&many)?;
    generate_huge(&huge)?;
    let encrypted = dir.join("encrypted.pdf");
    generate_encrypted(&huge, &encrypted)?;
    let mut contents = Vec::new();
    for (name, filter) in [
        ("content-fl", ContentFilter::Flate),
        ("content-a85", ContentFilter::Ascii85Flate),
        ("content-lzw", ContentFilter::Lzw),
    ] {
        let path = dir.join(format!("{name}.pdf"));
        generate_content(&path, filter)?;
        contents.push((name, path));
    }
    let forms = dir.join("forms.pdf");
    generate_forms(&forms)?;
    let paths = dir.join("paths.pdf");
    generate_paths(&paths)?;
    eprintln!(
        "memprobe: inputs ready: many={} huge={} encrypted={} forms={} bytes",
        std::fs::metadata(&many).map(|m| m.len()).unwrap_or(0),
        std::fs::metadata(&huge).map(|m| m.len()).unwrap_or(0),
        std::fs::metadata(&encrypted).map(|m| m.len()).unwrap_or(0),
        std::fs::metadata(&forms).map(|m| m.len()).unwrap_or(0),
    );

    let (cert, key) = credentials();
    // One output file per step (suffixed by input), so no step reuses
    // another step's sink.
    let open_sink = |n: &str, op: &str| dir.join(format!("{n}_{op}.pdf"));

    for (name, path) in [("many", &many), ("huge", &huge)] {

        // open: the read-only open plus dispose, on the same lane.
        step(&failed, "open", name, limit("open", name), |st| {
            let resp = call(st, re("open"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("open returns handleId");
            call(st, re("docDispose").put("handleId", i32v(hid)), vec![], vec![])?;
            Ok(())
        });

        // extract of all pages to a sink: one frame per page, so the
        // engine holds one page of text at a time.
        step(&failed, "extractAll", name, limit("extractAll", name), |st| {
            let resp = call(st, re("open"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("open returns handleId");
            call(
                st,
                re("extract")
                    .put("handleId", i32v(hid))
                    .put("format", strv("plainText")),
                vec![],
                vec![sink(&open_sink(name, "extractall"))],
            )?;
            call(st, re("docDispose").put("handleId", i32v(hid)), vec![], vec![])?;
            Ok(())
        });

        // extractPages of page 0 to a sink; the page-0 save's prune plan
        // drops everything only the other pages reach (#261).
        step(&failed, "extractPages", name, limit("extractPages", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            call(
                st,
                re("editorExtractPages")
                    .put("handleId", i32v(hid))
                    .put("pages", wire::Value::IntList(vec![0])),
                vec![],
                vec![sink(&open_sink(name, "extract"))],
            )?;
            call(st, re("editorDispose").put("handleId", i32v(hid)), vec![], vec![])?;
            Ok(())
        });

        // edit + full-rewrite save with GC
        step(&failed, "fullRewriteSave", name, limit("fullRewriteSave", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            call(
                st,
                re("editorMutate")
                    .put("handleId", i32v(hid))
                    .put("editOp", strv("rotateAll"))
                    .put("degrees", i32v(90)),
                vec![],
                vec![],
            )?;
            save_full(st, hid, &open_sink(name, "full"), false)?;
            dispose(st, hid);
            Ok(())
        });

        // edit + setTitle + incremental save
        step(&failed, "incrementalSave", name, limit("incrementalSave", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            call(
                st,
                re("editorMutate")
                    .put("handleId", i32v(hid))
                    .put("editOp", strv("setTitle"))
                    .put("title", strv("memprobe")),
                vec![],
                vec![],
            )?;
            call(
                st,
                re("editorSave")
                    .put("handleId", i32v(hid))
                    .put("saveMode", i32v(1)),
                vec![],
                vec![sink(&open_sink(name, "incremental"))],
            )?;
            dispose(st, hid);
            Ok(())
        });

        // sign with the fixture PEM credentials, both files through the
        // streaming path.
        step(&failed, "sign", name, limit("sign", name), |st| {
            call(
                st,
                re("sign")
                    .put("certPem", wire::Value::Str(cert.to_string()))
                    .put("keyPem", wire::Value::Str(key.to_string()))
                    .put("signedAt", wire::Value::I64(1_700_000_000)),
                vec![source(path)],
                vec![sink(&open_sink(name, "signed"))],
            )?;
            Ok(())
        });

        // compress — the standalone compress entry resolves to editorSave
        // with the full-rewrite and compression fields (compress true, GC
        // on), the same pdf_sugar group the Dart API ends in.
        step(&failed, "compress", name, limit("compress", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            save_full(st, hid, &open_sink(name, "compressed"), true)?;
            dispose(st, hid);
            Ok(())
        });

        // merge of two copies to a sink
        step(&failed, "merge", name, limit("merge", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            call(
                st,
                re("editorMergeFrom").put("handleId", i32v(hid)),
                vec![source(path)],
                vec![],
            )?;
            save_full(st, hid, &open_sink(name, "merged"), false)?;
            dispose(st, hid);
            Ok(())
        });

        // merge a copy, then extract the first page of the merged part to a
        // sink: the extract streams one page, never the merged document.
        if name == "many" {
            step(&failed, "extractAfterMerge", name, limit("extractAfterMerge", name), |st| {
                let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
                let hid = resp.i32("handleId").expect("editorOpen returns handleId");
                let merged_first = call(
                    st,
                    re("editorGetMetadata").put("handleId", i32v(hid)),
                    vec![],
                    vec![],
                )?
                .i32("pageCount")
                .expect("editorGetMetadata returns pageCount");
                call(
                    st,
                    re("editorMergeFrom").put("handleId", i32v(hid)),
                    vec![source(path)],
                    vec![],
                )?;
                call(
                    st,
                    re("editorExtractPages")
                        .put("handleId", i32v(hid))
                        .put("pages", wire::Value::IntList(vec![merged_first])),
                    vec![],
                    vec![sink(&open_sink(name, "extract_merged"))],
                )?;
                dispose(st, hid);
                Ok(())
            });
        }
    }

    // The encrypted source: every stream is decrypted while it is copied, so
    // the save holds the same few buffers as the plain one.
    step(&failed, "fullRewriteSave", "encrypted", limit("fullRewriteSave", "encrypted"), |st| {
        let resp = call(st, re("editorOpen"), vec![source(&encrypted)], vec![])?;
        let hid = resp.i32("handleId").expect("editorOpen returns handleId");
        call(
            st,
            re("editorMutate")
                .put("handleId", i32v(hid))
                .put("editOp", strv("rotateAll"))
                .put("degrees", i32v(90)),
            vec![],
            vec![],
        )?;
        save_full(st, hid, &open_sink("encrypted", "full"), false)?;
        dispose(st, hid);
        Ok(())
    });

    // compress of the sources the resource prune reads: one page of 96 MiB
    // of path content and inline images in each of three encodings, 2,000
    // forms of 64 KiB each, and the encrypted source.
    let compress_inputs: Vec<(&str, &Path)> = contents
        .iter()
        .map(|(name, path)| (*name, path.as_path()))
        .chain([("forms", forms.as_path()), ("encrypted", encrypted.as_path())])
        .collect();
    for (name, path) in compress_inputs {
        step(&failed, "compress", name, limit("compress", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            save_full(st, hid, &open_sink(name, "compressed"), true)?;
            dispose(st, hid);
            Ok(())
        });
    }

    // extractAll of the same sources: pages of a few KiB of text among 96 MiB
    // of path content and 2,000 forms, read as operators and never as one
    // page's bytes.
    let extract_inputs: Vec<(&str, &Path)> = contents
        .iter()
        .map(|(name, path)| (*name, path.as_path()))
        .chain([("forms", forms.as_path()), ("paths", paths.as_path())])
        .collect();
    for (name, path) in extract_inputs {
        step(&failed, "extractAll", name, limit("extractAll", name), |st| {
            let resp = call(st, re("open"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("open returns handleId");
            call(
                st,
                re("extract")
                    .put("handleId", i32v(hid))
                    .put("format", strv("plainText")),
                vec![],
                vec![sink(&open_sink(name, "extractall"))],
            )?;
            call(st, re("docDispose").put("handleId", i32v(hid)), vec![], vec![])?;
            Ok(())
        });
    }

    // render of the same sources: one page of 96 MiB of path content, and
    // 2,000 forms, painted band by band from the operators.
    let render_inputs: Vec<(&str, &Path)> = contents
        .iter()
        .map(|(name, path)| (*name, path.as_path()))
        .chain([("forms", forms.as_path())])
        .collect();
    for (name, path) in render_inputs {
        step(&failed, "render", name, limit("render", name), |st| {
            render_to_sink(st, path, 150, &open_sink(name, "render"))
        });
        verify_render_wire(&failed, name, &open_sink(name, "render"));
    }

    // render of one page far bigger than the host may hold, at 300 dpi.
    let bigpage = dir.join("bigpage.pdf");
    generate_bigpage(&bigpage)?;
    step(&failed, "render", "bigpage-300dpi", limit("render", "bigpage-300dpi"), |st| {
        render_to_sink(st, &bigpage, 300, &open_sink("bigpage", "render"))
    });
    verify_render_wire(&failed, "bigpage-300dpi", &open_sink("bigpage", "render"));

    // render of one page filled by a 60 MiB image, and of one filled by a
    // progressive JPEG, at 150 dpi.
    let huge_drawn = dir.join("huge-drawn.pdf");
    generate_huge_drawn(&huge_drawn)?;
    step(&failed, "render", "huge", limit("render", "huge"), |st| {
        render_to_sink(st, &huge_drawn, 150, &open_sink("huge", "render"))
    });
    verify_render_wire(&failed, "huge", &open_sink("huge", "render"));
    let huge_jpeg = dir.join("huge-jpeg.pdf");
    generate_huge_jpeg(&huge_jpeg)?;
    step(&failed, "render", "hugeJpeg", limit("render", "hugeJpeg"), |st| {
        render_to_sink(st, &huge_jpeg, 150, &open_sink("hugeJpeg", "render"))
    });
    verify_render_wire(&failed, "hugeJpeg", &open_sink("hugeJpeg", "render"));

    let _ = std::fs::remove_dir_all(&dir);
    if failed.get() {
        return Err("one or more steps exceeded their memory limit".to_string());
    }
    Ok(())
}

/// Opens `path`, renders every page to `sink_path` as PNG at `dpi`, and
/// disposes the document: the host path a Dart `render` takes.
fn render_to_sink(
    state: &mut pdf_oxide::host::lane_state::LaneState,
    path: &Path,
    dpi: i32,
    sink_path: &Path,
) -> Result<(), String> {
    let resp = call(state, re("open"), vec![source(path)], vec![])?;
    let hid = resp.i32("handleId").expect("open returns handleId");
    call(
        state,
        re("render")
            .put("handleId", i32v(hid))
            .put("sizeKind", strv("dpi"))
            .put("dpi", i32v(dpi))
            .put("format", strv("png"))
            .put("hasSink", wire::Value::Bool(true)),
        vec![],
        vec![sink(sink_path)],
    )?;
    call(state, re("docDispose").put("handleId", i32v(hid)), vec![], vec![])?;
    Ok(())
}

/// Reads the wire a render left in `sink_path` from its first frame to the
/// terminator, after the measured step: every page is a header, data frames
/// of at most 256 KiB and an end frame, its data is one whole PNG (signature,
/// an IHDR that matches the header, chunks to IEND, nothing after it), and
/// the terminator is the last four bytes of the file. Frames are read one at
/// a time and counted, never kept. A failure fails the probe.
fn verify_render_wire(failed: &std::cell::Cell<bool>, input: &str, sink_path: &Path) {
    let started = std::time::Instant::now();
    match check_render_wire(sink_path) {
        Ok(summary) => eprintln!(
            "memprobe: render {input}: wire ok, {summary} (read back in {} ms)",
            started.elapsed().as_millis()
        ),
        Err(e) => {
            eprintln!("memprobe: render {input}: WIRE FAIL: {e}");
            failed.set(true);
        },
    }
}

fn check_render_wire(sink_path: &Path) -> Result<String, String> {
    use std::io::Read;
    let file = File::open(sink_path).map_err(|e| format!("open sink: {e}"))?;
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut file = std::io::BufReader::new(file);
    let mut read = 0u64;
    let mut pages = 0usize;
    let mut open: Option<(i32, i32, PngWalk)> = None;
    let (mut data_bytes, mut data_frames, mut largest, mut first) = (0u64, 0usize, 0usize, String::new());
    loop {
        let mut len = [0u8; 4];
        file.read_exact(&mut len).map_err(|e| format!("terminator missing: {e}"))?;
        read += 4;
        let len = u32::from_le_bytes(len) as usize;
        if len == 0 {
            if open.is_some() {
                return Err("the terminator came inside a page".into());
            }
            if read != total {
                return Err(format!("{} bytes follow the terminator", total - read));
            }
            return Ok(format!(
                "{pages} page(s), first {first}, {data_frames} data frames (largest {largest} bytes) \
                 summing {data_bytes} PNG bytes, {total} bytes on the wire"
            ));
        }
        let mut frame = vec![0u8; len];
        file.read_exact(&mut frame).map_err(|e| format!("truncated frame: {e}"))?;
        read += len as u64;
        let frame = wire::parse_response(&frame)?;
        match frame.str("kind") {
            Some("header") => {
                if open.is_some() {
                    return Err("a header inside a page".into());
                }
                let (w, h) = (frame.i32("width").unwrap_or(0), frame.i32("height").unwrap_or(0));
                if pages == 0 {
                    first = format!("{w}x{h} px");
                }
                open = Some((w, h, PngWalk::new()));
            },
            Some("data") => {
                let (_, _, walk) = open.as_mut().ok_or("data outside a page")?;
                let bytes = frame.bytes("data").ok_or("a data frame without data")?;
                if bytes.len() > 256 * 1024 {
                    return Err(format!("a data frame of {} bytes", bytes.len()));
                }
                walk.feed(bytes)?;
                data_bytes += bytes.len() as u64;
                data_frames += 1;
                largest = largest.max(bytes.len());
            },
            Some("end") => {
                let (w, h, walk) = open.take().ok_or("an end outside a page")?;
                walk.finish(w, h)?;
                pages += 1;
            },
            other => return Err(format!("unknown frame kind {other:?}")),
        }
    }
}

/// Walks the chunks of a PNG as its bytes arrive, keeping only the 13 bytes
/// of IHDR: signature, chunks, IEND last, nothing after it.
struct PngWalk {
    phase: PngPhase,
    ihdr: Vec<u8>,
    /// Bytes fed.
    consumed: u64,
    /// Bytes the chunk lengths account for: signature plus each chunk's
    /// length, type, body and checksum.
    walked: u64,
    idat: u64,
}

enum PngPhase {
    Signature(usize),
    Head([u8; 8], usize),
    Body { kind: [u8; 4], left: u64 },
    Done,
}

impl PngWalk {
    fn new() -> Self {
        Self { phase: PngPhase::Signature(0), ihdr: Vec::new(), consumed: 0, walked: 0, idat: 0 }
    }

    fn feed(&mut self, mut bytes: &[u8]) -> Result<(), String> {
        const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        self.consumed += bytes.len() as u64;
        while !bytes.is_empty() {
            match &mut self.phase {
                PngPhase::Signature(n) => {
                    let take = (8 - *n).min(bytes.len());
                    if bytes[..take] != SIGNATURE[*n..*n + take] {
                        return Err("not a PNG signature".into());
                    }
                    *n += take;
                    bytes = &bytes[take..];
                    if *n == 8 {
                        self.walked = 8;
                        self.phase = PngPhase::Head([0; 8], 0);
                    }
                },
                PngPhase::Head(head, n) => {
                    let take = (8 - *n).min(bytes.len());
                    head[*n..*n + take].copy_from_slice(&bytes[..take]);
                    *n += take;
                    bytes = &bytes[take..];
                    if *n == 8 {
                        let len = u32::from_be_bytes(head[..4].try_into().unwrap()) as u64;
                        let kind: [u8; 4] = head[4..].try_into().unwrap();
                        self.walked += 12 + len;
                        self.phase = PngPhase::Body { kind, left: len + 4 };
                    }
                },
                PngPhase::Body { kind, left } => {
                    let take = (*left).min(bytes.len() as u64) as usize;
                    if kind == b"IHDR" && self.ihdr.len() < 13 {
                        let want = (13 - self.ihdr.len()).min(take);
                        self.ihdr.extend_from_slice(&bytes[..want]);
                    }
                    if kind == b"IDAT" {
                        self.idat += take as u64;
                    }
                    *left -= take as u64;
                    bytes = &bytes[take..];
                    if *left == 0 {
                        self.phase = if kind == b"IEND" {
                            PngPhase::Done
                        } else {
                            PngPhase::Head([0; 8], 0)
                        };
                    }
                },
                PngPhase::Done => return Err("bytes after IEND".into()),
            }
        }
        Ok(())
    }

    /// The page ended: the PNG must be whole and say the size its header did.
    fn finish(self, width: i32, height: i32) -> Result<(), String> {
        if !matches!(self.phase, PngPhase::Done) {
            return Err("the page ended before IEND".into());
        }
        if self.walked != self.consumed {
            return Err(format!(
                "the data frames hold {} bytes, the PNG's chunks {}",
                self.consumed, self.walked
            ));
        }
        let ihdr = self.ihdr;
        let (w, h) = (
            u32::from_be_bytes(ihdr[0..4].try_into().unwrap()) as i32,
            u32::from_be_bytes(ihdr[4..8].try_into().unwrap()) as i32,
        );
        if (w, h) != (width, height) {
            return Err(format!("IHDR says {w}x{h}, the header said {width}x{height}"));
        }
        if ihdr[8] != 8 || ihdr[9] != 6 {
            return Err(format!("IHDR depth {} colour type {}, not 8-bit RGBA", ihdr[8], ihdr[9]));
        }
        if self.idat == 0 {
            return Err("no IDAT".into());
        }
        Ok(())
    }
}

fn i32v(n: i32) -> wire::Value {
    wire::Value::I32(n)
}

fn strv(s: &str) -> wire::Value {
    wire::Value::Str(s.to_string())
}

fn re(op: &str) -> wire::Request {
    wire::Request::new(op)
}

fn source(path: &Path) -> File {
    File::open(path).expect("probe source file opens")
}

fn sink(path: &Path) -> File {
    File::create(path).expect("probe sink file opens")
}

/// One wire request through the probe door, response decoded or its
/// error message text returned.
fn call(
    state: &mut pdf_oxide::host::lane_state::LaneState,
    req: wire::Request,
    sources: Vec<File>,
    sinks: Vec<File>,
) -> Result<wire::OkResponse, String> {
    let bytes = req.encode();
    let resp = pdf_oxide::host::probe::run(state, &bytes, sources, sinks);
    wire::parse_response(&resp)
}

/// Full-rewrite save of an open editor to `sink` (compress + GC), dropping
/// the unused resource entries when `prune` is set.
fn save_full(
    state: &mut pdf_oxide::host::lane_state::LaneState,
    hid: i32,
    sink_path: &Path,
    prune: bool,
) -> Result<(), String> {
    call(
        state,
        re("editorSave")
            .put("handleId", i32v(hid))
            .put("compress", wire::Value::Bool(true))
            .put("garbageCollect", wire::Value::Bool(true))
            .put("pruneResources", wire::Value::Bool(prune))
            .put("saveMode", i32v(0))
            .put("encryptMode", i32v(0)),
        vec![],
        vec![sink(sink_path)],
    )?;
    Ok(())
}

fn dispose(state: &mut pdf_oxide::host::lane_state::LaneState, hid: i32) {
    let _ = call(state, re("editorDispose").put("handleId", i32v(hid)), vec![], vec![]);
}

/// One measured step: fresh LaneState, peak tracking from the starting
/// level, peak measured once the step's last request returns. The
/// closure errors make the step FAIL before the limit check matters.
fn step(
    failed: &std::cell::Cell<bool>,
    op: &str,
    input: &str,
    limit: usize,
    body: impl FnOnce(&mut pdf_oxide::host::lane_state::LaneState) -> Result<(), String>,
) {
    alloc::reset_peak();
    let base = alloc::current();
    let mut state = pdf_oxide::host::lane_state::LaneState::new();
    let started = std::time::Instant::now();
    let result = body(&mut state);
    let wall_ms = started.elapsed().as_millis();
    let peak = alloc::peak_over(base);
    drop(state);
    if let Err(e) = &result {
        eprintln!("memprobe: {op} {input}: {e}");
    }
    let ok = result.is_ok() && peak <= limit;
    if !ok {
        failed.set(true);
    }
    println!(
        "{op} {input} peak_over_baseline={peak} limit={limit} {} wall_ms={wall_ms}",
        if ok { "ok" } else { "FAIL" }
    );
}

// ── input generation ────────────────────────────────────────────────

/// xorshift64 with a fixed seed per call site; pseudo-random but
/// reproducible, so Flate cannot shrink a generated run below its size.
struct Xorshift(u64);

impl Xorshift {
    fn new(seed: u64) -> Self {
        Xorshift(seed ^ 0x2545_F491_4F6C_DD1D)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Fills `buf` with pseudo-random hex characters.
    fn fill_hex(&mut self, buf: &mut String, n: usize) {
        const ALPHABET: &[u8] = b"0123456789abcdef";
        let mut remaining = n;
        while remaining > 0 {
            let mut x = self.next();
            while remaining > 0 && x != 0 {
                buf.push(ALPHABET[(x & 0xF) as usize] as char);
                x >>= 4;
                remaining -= 1;
            }
        }
    }

    /// Appends line segments (`x y m x y l n`) to `buf` until it holds at
    /// least `target` bytes: path content that Flate cannot shrink, that shows
    /// no text and paints nothing, so no path is left for table detection to
    /// keep.
    fn fill_paths(&mut self, buf: &mut String, target: usize) {
        while buf.len() < target {
            let n = self.next();
            buf.push_str(&format!(
                "{}.{} {}.{} m {}.{} {}.{} l n\n",
                n & 0x3FF,
                (n >> 10) & 0x3F,
                (n >> 16) & 0x3FF,
                (n >> 26) & 0x3F,
                (n >> 32) & 0x3FF,
                (n >> 42) & 0x3F,
                (n >> 48) & 0x3FF,
                (n >> 58) & 0x3F,
            ));
        }
    }

    /// Fills `buf` with pseudo-random bytes.
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for b in buf.iter_mut() {
            *b = (self.next() >> 24) as u8;
        }
    }
}

/// 10,000 pages, each with a 6 KiB content stream of incompressible
/// `(<hex>) Tj` text, built through the probe door with the builder ops
/// and saved uncompressed. Handles the ~64 MiB the plan calls for.
fn generate_many(path: &Path) -> Result<(), String> {
    const PAGES: usize = 10_000;

    let mut state = pdf_oxide::host::lane_state::LaneState::new();
    let resp = call(&mut state, re("builderCreate"), vec![], vec![])?;
    let hid = resp.i32("handleId").expect("builderCreate returns handleId");

    for page in 0..PAGES {
        call(&mut state, re("builderAddPage").put("handleId", i32v(hid)), vec![], vec![])
            .map_err(|e| format!("builderAddPage page {page}: {e}"))?;
        call(
            &mut state,
            re("builderPageOp")
                .put("handleId", i32v(hid))
                .put("pageOp", strv("font"))
                .put("name", strv("Helvetica"))
                .put("size", wire::Value::F64(10.0)),
            vec![],
            vec![],
        )
            .map_err(|e| format!("font page {page}: {e}"))?;
        call(
            &mut state,
            re("builderPageOp")
                .put("handleId", i32v(hid))
                .put("pageOp", strv("at"))
                .put("x", wire::Value::F64(10.0))
                .put("y", wire::Value::F64(800.0)),
            vec![],
            vec![],
        )
            .map_err(|e| format!("at page {page}: {e}"))?;
        // Six text ops of 1024 hex characters each — the content stream,
        // so Flate sees a file of near-random symbols.
        let mut rng = Xorshift::new(page as u64 * 1_000_003);
        for chunk in 0..6 {
            let mut payload = String::with_capacity(1024);
            rng.fill_hex(&mut payload, 1024);
            call(
                &mut state,
                re("builderPageOp")
                    .put("handleId", i32v(hid))
                    .put("pageOp", strv("text"))
                    .put("text", wire::Value::Str(payload)),
                vec![],
                vec![],
            )
                .map_err(|e| format!("chunk {chunk} page {page}: {e}"))?;
        }
        call(&mut state, re("builderPageDone").put("handleId", i32v(hid)), vec![], vec![])
            .map_err(|e| format!("done page {page}: {e}"))?;
        if page % 2000 == 0 {
            eprintln!("memprobe: many: page {page}/{PAGES}");
        }
    }

    call(
        &mut state,
        re("builderSave").put("handleId", i32v(hid)),
        vec![],
        vec![sink(path)],
    )?;
    Ok(())
}

/// Incompressible bytes, `left` of them, read in whatever pieces are asked for.
struct Noise {
    rng: Xorshift,
    left: usize,
}

impl std::io::Read for Noise {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let take = buf.len().min(self.left);
        self.rng.fill_bytes(&mut buf[..take]);
        self.left -= take;
        Ok(take)
    }
}

/// The page `huge` is, with the content that paints its image: one Letter
/// page filled by a 60 MiB DeviceGray image (8192 x 7680) of incompressible
/// bytes. `huge` itself paints nothing, so a render of it is blank; this is
/// the input a render of a huge image is measured on.
fn generate_huge_drawn(path: &Path) -> Result<(), String> {
    const IMAGE_BYTES: usize = 60 * MIB;
    let content = "q 612 0 0 792 0 0 cm /Im0 Do Q";
    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    out.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>",
    )?;
    out.stream(4, "", content.len() as u64, content.as_bytes())?;
    out.stream(
        5,
        "/Type /XObject /Subtype /Image /Width 8192 /Height 7680 /ColorSpace /DeviceGray \
         /BitsPerComponent 8",
        IMAGE_BYTES as u64,
        Noise { rng: Xorshift::new(0xDEADBEEF), left: IMAGE_BYTES },
    )?;
    out.finish()
}

/// A one-page Letter PDF filled by one progressive JPEG of 4096 x 3072 RGB.
/// A progressive JPEG cannot be decoded a band at a time, and the decoder
/// holds its coefficients for the whole image; this is the ruled exception to
/// the image rows being read as they are drawn.
fn generate_huge_jpeg(path: &Path) -> Result<(), String> {
    const W: usize = 4096;
    const H: usize = 3072;
    let mut rng = Xorshift::new(0x1F6);
    let mut rgb = vec![0u8; W * H * 3];
    for y in 0..H {
        for x in 0..W {
            let n = rng.next();
            let at = (y * W + x) * 3;
            rgb[at] = ((x * 255 / W) as u64 + (n & 0x1F)).min(255) as u8;
            rgb[at + 1] = ((y * 255 / H) as u64 + ((n >> 8) & 0x1F)).min(255) as u8;
            rgb[at + 2] = (((x + y) * 255 / (W + H)) as u64 + ((n >> 16) & 0x1F)).min(255) as u8;
        }
    }
    let mut jpeg = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut jpeg, 70);
    encoder.set_progressive(true);
    encoder
        .encode(&rgb, W as u16, H as u16, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| format!("hugeJpeg: {e}"))?;
    let content = "q 612 0 0 792 0 0 cm /Im0 Do Q";
    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    out.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>",
    )?;
    out.stream(4, "", content.len() as u64, content.as_bytes())?;
    out.stream(
        5,
        "/Type /XObject /Subtype /Image /Width 4096 /Height 3072 /ColorSpace /DeviceRGB \
         /BitsPerComponent 8 /Filter /DCTDecode",
        jpeg.len() as u64,
        std::io::Cursor::new(jpeg),
    )?;
    out.finish()
}

/// A one-page PDF holding one image XObject stream of 60 MiB of
/// incompressible bytes, written by hand (header, 4 objects, xref with
/// real offsets, trailer), streamed to the file in 64 KiB chunks.
fn generate_huge(path: &Path) -> Result<(), String> {
    use std::io::BufWriter;
    const CHUNK: usize = 64 * 1024;
    const IMAGE_BYTES: usize = 60 * 1024 * 1024; // 8192 x 7680 DeviceGray

    struct Out {
        w: BufWriter<File>,
        at: u64,
        offsets: [u64; 5],
    }

    impl Out {
        fn put(&mut self, s: &str) -> Result<(), String> {
            self.w.write_all(s.as_bytes()).map_err(|e| format!("huge: {e}"))?;
            self.at += s.len() as u64;
            Ok(())
        }
        fn mark(&mut self, object: usize) {
            self.offsets[object] = self.at;
        }
        fn flush(&mut self) -> Result<(), String> {
            use std::io::Write as _;
            self.w.flush().map_err(|e| format!("huge: {e}"))
        }
    }

    let mut out = Out {
        w: BufWriter::new(File::create(path).map_err(|e| format!("huge: {e}"))?),
        at: 0,
        offsets: [0; 5],
    };

    out.put("%PDF-1.4\n")?;
    out.mark(1);
    out.put("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;
    out.mark(2);
    out.put("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n")?;
    out.mark(3);
    out.put("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /XObject << /Im0 4 0 R >> >> >>\nendobj\n")?;
    out.mark(4);
    out.put(&format!(
        "4 0 obj\n<< /Type /XObject /Subtype /Image /Width 8192 /Height 7680 /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {IMAGE_BYTES} >>\nstream\n"
    ))?;

    let mut rng = Xorshift::new(0xDEADBEEF);
    let mut chunk = vec![0u8; CHUNK.min(IMAGE_BYTES)];
    let mut left = IMAGE_BYTES;
    while left > 0 {
        let take = CHUNK.min(left);
        rng.fill_bytes(&mut chunk[..take]);
        out.w.write_all(&chunk[..take]).map_err(|e| format!("huge: {e}"))?;
        out.at += take as u64;
        left -= take;
    }
    out.put("\nendstream\nendobj\n")?;

    let xref_at = out.at;
    let mut table = format!("xref\n0 {}\n", out.offsets.len() + 1);
    table.push_str("0000000000 65535 f \n");
    for offset in out.offsets.iter().skip(1) {
        table.push_str(&format!("{offset:010} 00000 n \n"));
    }
    out.put(&table)?;
    out.put("trailer\n<< /Size 5 /Root 1 0 R >>\n")?;
    out.put(&format!("startxref\n{xref_at}\n%%EOF\n"))?;
    out.flush()?;
    Ok(())
}

/// A hand-written PDF: objects are marked as they start, and the table is
/// written from the marks.
struct PdfOut {
    w: std::io::BufWriter<File>,
    at: u64,
    offsets: Vec<u64>,
}

impl PdfOut {
    fn create(path: &Path) -> Result<Self, String> {
        let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut out = Self { w: std::io::BufWriter::new(file), at: 0, offsets: vec![0] };
        out.put("%PDF-1.4\n")?;
        Ok(out)
    }

    fn put(&mut self, s: &str) -> Result<(), String> {
        self.w.write_all(s.as_bytes()).map_err(|e| format!("pdf: {e}"))?;
        self.at += s.len() as u64;
        Ok(())
    }

    /// Starts object `id`, which must be the next one.
    fn begin(&mut self, id: usize) -> Result<(), String> {
        assert_eq!(id, self.offsets.len(), "objects are written in order");
        self.offsets.push(self.at);
        self.put(&format!("{id} 0 obj\n"))
    }

    fn object(&mut self, id: usize, body: &str) -> Result<(), String> {
        self.begin(id)?;
        self.put(body)?;
        self.put("\nendobj\n")
    }

    /// A stream object whose data is `data`, from a reader of `len` bytes.
    fn stream(
        &mut self,
        id: usize,
        dict: &str,
        len: u64,
        mut data: impl std::io::Read,
    ) -> Result<(), String> {
        self.begin(id)?;
        self.put(&format!("<< {dict} /Length {len} >>\nstream\n"))?;
        let copied = std::io::copy(&mut data, &mut self.w).map_err(|e| format!("pdf: {e}"))?;
        assert_eq!(copied, len, "the stream is as long as it was said to be");
        self.at += copied;
        self.put("\nendstream\nendobj\n")
    }

    /// The table and trailer; the catalog is object 1.
    fn finish(mut self) -> Result<(), String> {
        let xref_at = self.at;
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", self.offsets.len());
        for offset in self.offsets.iter().skip(1) {
            table.push_str(&format!("{offset:010} 00000 n \n"));
        }
        self.put(&table)?;
        self.put(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            self.offsets.len()
        ))?;
        self.w.flush().map_err(|e| format!("pdf: {e}"))
    }
}

/// How the `content` input is encoded.
#[derive(Clone, Copy)]
enum ContentFilter {
    Flate,
    Ascii85Flate,
    Lzw,
}

impl ContentFilter {
    fn entry(self) -> &'static str {
        match self {
            ContentFilter::Flate => "/Filter /FlateDecode",
            ContentFilter::Ascii85Flate => "/Filter [/ASCII85Decode /FlateDecode]",
            ContentFilter::Lzw => "/Filter /LZWDecode",
        }
    }
}

/// Decoded bytes of the `content` input's one content stream.
const CONTENT_BYTES: usize = 96 * MIB;

/// The decoded content, produced as it is read: a few KiB of `(<hex>) Tj`
/// text first, then sections of line segments, each followed by an inline
/// image whose raw data holds `\nEI\n`. The page shows a few KiB of text, so
/// extracting it holds that text and not the 96 MiB the stream is.
struct ContentText {
    rng: Xorshift,
    produced: usize,
    section: Vec<u8>,
    at: usize,
}

impl ContentText {
    fn new() -> Self {
        Self { rng: Xorshift::new(0xC0FFEE), produced: 0, section: Vec::new(), at: 0 }
    }

    fn next_section(&mut self) {
        let mut text = String::new();
        if self.produced == 0 {
            text.push_str("BT /F1 10 Tf 10 10 Td\n");
            for _ in 0..40 {
                text.push('(');
                self.rng.fill_hex(&mut text, 96);
                text.push_str(") Tj\n");
            }
            text.push_str("ET\n");
        } else {
            self.rng.fill_paths(&mut text, 4096);
        }
        text.push_str("q BI /W 8 /H 8 /BPC 8 /CS /G ID ");
        let mut image = [0u8; 64];
        self.rng.fill_bytes(&mut image);
        image[20..24].copy_from_slice(b"\nEI\n");
        self.section.clear();
        self.section.extend_from_slice(text.as_bytes());
        self.section.extend_from_slice(&image);
        self.section.extend_from_slice(b"\nEI Q\n");
        self.produced += self.section.len();
        self.at = 0;
    }
}

impl std::io::Read for ContentText {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.at == self.section.len() {
            if self.produced >= CONTENT_BYTES {
                return Ok(0);
            }
            self.next_section();
        }
        let n = buf.len().min(self.section.len() - self.at);
        buf[..n].copy_from_slice(&self.section[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// Writes ASCII85 groups of what it is given; `finish` ends the data.
struct Ascii85Writer<W: Write> {
    inner: W,
    group: [u8; 4],
    held: usize,
}

impl<W: Write> Ascii85Writer<W> {
    fn new(inner: W) -> Self {
        Self { inner, group: [0; 4], held: 0 }
    }

    fn digits(value: u32) -> [u8; 5] {
        let mut value = value;
        let mut digits = [0u8; 5];
        for digit in digits.iter_mut().rev() {
            *digit = (value % 85) as u8 + b'!';
            value /= 85;
        }
        digits
    }

    fn finish(mut self) -> std::io::Result<W> {
        if self.held > 0 {
            let mut padded = self.group;
            padded[self.held..].fill(0);
            let digits = Self::digits(u32::from_be_bytes(padded));
            self.inner.write_all(&digits[..self.held + 1])?;
        }
        self.inner.write_all(b"~>")?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for Ascii85Writer<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for &byte in buf {
            self.group[self.held] = byte;
            self.held += 1;
            if self.held == 4 {
                self.held = 0;
                match u32::from_be_bytes(self.group) {
                    0 => self.inner.write_all(b"z")?,
                    value => self.inner.write_all(&Self::digits(value))?,
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// A one-page PDF whose one content stream is [`CONTENT_BYTES`] of text and
/// inline images, encoded in `filter`. The page lists a second font that
/// nothing uses. The encoded stream is written beside the file first, since
/// its length goes before it.
fn generate_content(path: &Path, filter: ContentFilter) -> Result<(), String> {
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::BufWriter;

    let body_path = path.with_extension("body");
    let io = |e: std::io::Error| format!("content: {e}");
    {
        let mut body = BufWriter::new(File::create(&body_path).map_err(io)?);
        let mut text = ContentText::new();
        match filter {
            ContentFilter::Flate => {
                let mut encoder = ZlibEncoder::new(&mut body, Compression::fast());
                std::io::copy(&mut text, &mut encoder).map_err(io)?;
                encoder.finish().map_err(io)?;
            },
            ContentFilter::Ascii85Flate => {
                let mut encoder =
                    ZlibEncoder::new(Ascii85Writer::new(&mut body), Compression::fast());
                std::io::copy(&mut text, &mut encoder).map_err(io)?;
                encoder.finish().map_err(io)?.finish().map_err(io)?;
            },
            ContentFilter::Lzw => {
                let mut encoder = weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
                encoder
                    .into_stream(&mut body)
                    .encode_all(std::io::BufReader::new(&mut text))
                    .status
                    .map_err(io)?;
            },
        }
        body.flush().map_err(io)?;
    }
    let len = std::fs::metadata(&body_path).map_err(io)?.len();

    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    out.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> /Contents 4 0 R >>",
    )?;
    out.stream(4, filter.entry(), len, File::open(&body_path).map_err(io)?)?;
    out.object(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
    out.object(6, "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>")?;
    out.finish()?;
    std::fs::remove_file(&body_path).map_err(io)
}

/// A one-page PDF that paints 2,000 forms of 64 KiB of path content, each
/// with three two-character words of text (a character costs the extractor
/// a record of a few hundred bytes, so the page's text stays small) and
/// resources of its own that list a font its text does not use.
fn generate_forms(path: &Path) -> Result<(), String> {
    const FORMS: usize = 2000;
    const FIRST_FORM: usize = 7;

    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    let mut names = String::new();
    for form in 0..FORMS {
        names.push_str(&format!("/Fm{form} {} 0 R ", FIRST_FORM + form));
    }
    out.object(
        3,
        &format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /XObject << {names}>> >> /Contents 4 0 R >>"
        ),
    )?;
    let page: String = (0..FORMS).map(|form| format!("/Fm{form} Do\n")).collect();
    out.stream(4, "", page.len() as u64, page.as_bytes())?;
    out.object(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
    out.object(6, "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>")?;
    let mut rng = Xorshift::new(0xF0F0);
    for form in 0..FORMS {
        let mut text = String::from("BT /F1 10 Tf 10 10 Td\n");
        for _ in 0..3 {
            text.push('(');
            rng.fill_hex(&mut text, 2);
            text.push_str(") Tj\n");
        }
        text.push_str("ET\n");
        rng.fill_paths(&mut text, 64 * 1024);
        out.stream(
            FIRST_FORM + form,
            "/Type /XObject /Subtype /Form /BBox [0 0 612 792] \
             /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >>",
            text.len() as u64,
            text.as_bytes(),
        )?;
        if form % 500 == 0 {
            eprintln!("memprobe: forms: form {form}/{FORMS}");
        }
    }
    out.finish()
}

/// A one-page PDF 2000 points square whose ink covers its whole height: 2,000
/// coloured strokes, one per point of height, each sloped differently across
/// the page, and a row of text every 40 points. Its content is small (about
/// 150 KB), its raster is not, and no band of it is blank.
fn generate_bigpage(path: &Path) -> Result<(), String> {
    let mut rng = Xorshift::new(0xB16);
    let mut content = String::new();
    for y in 0..2000 {
        let n = rng.next();
        content.push_str(&format!(
            "{:.2} {:.2} {:.2} RG {:.1} w 0 {y} m 2000 {} l S\n",
            (n & 0xFF) as f32 / 255.0,
            ((n >> 8) & 0xFF) as f32 / 255.0,
            ((n >> 16) & 0xFF) as f32 / 255.0,
            0.5 + ((n >> 24) & 0x7) as f32 * 0.4,
            y as i64 + ((n >> 32) % 200) as i64 - 100,
        ));
    }
    for row in 0..50 {
        content.push_str(&format!(
            "BT /F1 18 Tf 0 g 40 {} Td (Row {row}: the quick brown fox jumps over the lazy dog) Tj ET\n",
            20 + row * 40
        ));
    }
    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    out.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 2000 2000] \
         /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
    )?;
    out.stream(4, "", content.len() as u64, content.as_bytes())?;
    out.object(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
    out.finish()
}

/// Decoded bytes of the `paths` input's one content stream.
const PATHS_BYTES: usize = 64 * MIB;

/// A one-page Letter PDF whose content is [`PATHS_BYTES`] of `x y m x y l S`
/// rulings: a lattice of 374 small tables (3 x 3 cells, 8 rulings and 9
/// numbers each) drawn over and over with a jitter of a tenth of a point, so
/// the page holds about two million ruling primitives and a few thousand
/// words. The grid they make is bounded by the page; the paths are not.
fn generate_paths(path: &Path) -> Result<(), String> {
    use std::io::BufWriter;

    const PITCH: f32 = 36.0;
    const CELL: f32 = 10.0;
    let body_path = path.with_extension("body");
    let io = |e: std::io::Error| format!("paths: {e}");
    let mut rng = Xorshift::new(0x7AB1E);
    let produced = std::cell::Cell::new(0usize);
    {
        let mut body = BufWriter::new(File::create(&body_path).map_err(io)?);
        let emit = |body: &mut BufWriter<File>, s: &str| -> std::io::Result<()> {
            produced.set(produced.get() + s.len());
            body.write_all(s.as_bytes())
        };
        emit(&mut body, "0.5 w\n").map_err(io)?;
        // The words, once.
        for row in 0..22 {
            for col in 0..17 {
                let (x, y) = (12.0 + col as f32 * PITCH, 12.0 + row as f32 * PITCH);
                for r in 0..3 {
                    for c in 0..3 {
                        let word = format!(
                            "BT /F1 6 Tf {} {} Td ({}.5) Tj ET\n",
                            x + c as f32 * CELL + 2.0,
                            y + r as f32 * CELL + 3.0,
                            r * 3 + c
                        );
                        emit(&mut body, &word).map_err(io)?;
                    }
                }
            }
        }
        // The rulings, again and again.
        while produced.get() < PATHS_BYTES {
            for row in 0..22 {
                for col in 0..17 {
                    let (x, y) = (12.0 + col as f32 * PITCH, 12.0 + row as f32 * PITCH);
                    let jitter = (rng.next() % 100) as f32 / 1000.0;
                    let mut lines = String::new();
                    for i in 0..=3 {
                        let d = i as f32 * CELL + jitter;
                        let far = 3.0 * CELL;
                        lines.push_str(&format!("{x:.2} {:.2} m {:.2} {:.2} l S\n", y + d, x + far, y + d));
                        lines.push_str(&format!("{:.2} {y:.2} m {:.2} {:.2} l S\n", x + d, x + d, y + far));
                    }
                    emit(&mut body, &lines).map_err(io)?;
                }
            }
        }
        body.flush().map_err(io)?;
    }
    let len = std::fs::metadata(&body_path).map_err(io)?.len();

    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    out.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
    )?;
    out.stream(4, "", len, File::open(&body_path).map_err(io)?)?;
    out.object(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
    out.finish()?;
    std::fs::remove_file(&body_path).map_err(io)
}

/// `huge`, AES-256 encrypted under an empty user password: the same 60 MiB
/// image stream, stored as ciphertext. Built whole in memory, which is
/// outside the measured steps.
fn generate_encrypted(huge: &Path, path: &Path) -> Result<(), String> {
    use pdf_oxide::editor::{DocumentEditor, EncryptionAlgorithm, EncryptionConfig, SaveOptions};

    let plain = std::fs::read(huge).map_err(|e| format!("encrypted: {e}"))?;
    let mut editor = DocumentEditor::from_bytes(plain).map_err(|e| format!("encrypted: {e}"))?;
    let config = EncryptionConfig::new("", "owner").with_algorithm(EncryptionAlgorithm::Aes256);
    let bytes = editor
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            ..SaveOptions::with_encryption(config)
        })
        .map_err(|e| format!("encrypted: {e}"))?;
    std::fs::write(path, bytes).map_err(|e| format!("encrypted: {e}"))
}

/// The PEM text between the fixture's markers — the key material keeps
/// its one home in test/fixtures/handwritten.dart.
fn credentials() -> (String, String) {
    let text = std::fs::read_to_string("test/fixtures/handwritten.dart")
        .expect("test/fixtures/handwritten.dart — run memprobe from the package root");
    let cert = between(&text, "testCertPem");
    let key = between(&text, "testKeyPem");
    (cert, key)
}

/// The text between `const <name> = '''` and the next `'''`.
fn between(text: &str, name: &str) -> String {
    let marker = format!("const {name} = '''");
    let start = text.find(&marker).expect("PEM opening marker in fixture");
    let after = &text[start + marker.len()..];
    let end = after.find("'''").expect("PEM closing marker in fixture");
    after[..end].trim().to_string()
}
