//! Memory wall probe for the pdf_manipulator host layer.
//!
//! Generates two inputs once (unmeasured), then runs each wire step on
//! each of them through pdf_oxide's `test-support` probe door, tracking
//! the allocator's high-water mark over the allocation level the
//! process already holds when the step starts. One line per step:
//! `<op> <input> peak_over_baseline=<bytes> limit=<bytes> ok|FAIL`.
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
        // Target 16 MiB, t-403: the merge rebuilds every imported object
        // and holds the second document until the save (163 MB measured).
        ("merge", "many") => 176 * MIB,
        // Target 16 MiB, t-403: the merge imports the second copy's 60 MB
        // stream whole (189 MB measured).
        ("merge", "huge") => 192 * MIB,
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
    eprintln!(
        "memprobe: inputs ready: many={} huge={} bytes",
        std::fs::metadata(&many).map(|m| m.len()).unwrap_or(0),
        std::fs::metadata(&huge).map(|m| m.len()).unwrap_or(0),
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
            save_full(st, hid, &open_sink(name, "full"))?;
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
            save_full(st, hid, &open_sink(name, "compressed"))?;
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
            save_full(st, hid, &open_sink(name, "merged"))?;
            dispose(st, hid);
            Ok(())
        });
    }

    let _ = std::fs::remove_dir_all(&dir);
    if failed.get() {
        return Err("one or more steps exceeded their memory limit".to_string());
    }
    Ok(())
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

/// Full-rewrite save of an open editor to `sink` (compress + GC).
fn save_full(
    state: &mut pdf_oxide::host::lane_state::LaneState,
    hid: i32,
    sink_path: &Path,
) -> Result<(), String> {
    call(
        state,
        re("editorSave")
            .put("handleId", i32v(hid))
            .put("compress", wire::Value::Bool(true))
            .put("garbageCollect", wire::Value::Bool(true))
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
    let result = body(&mut state);
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
        "{op} {input} peak_over_baseline={peak} limit={limit} {}",
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
