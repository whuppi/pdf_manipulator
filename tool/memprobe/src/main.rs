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
        // The file-level scans read in windows and an object stream gives up
        // one member at a time, so a broken xref and 64 MiB of packed
        // objects open and extract in the same memory as a sound file.
        ("open" | "extractAll", "damaged" | "objstm") => 16 * MIB,
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
        // The 60 MiB image is read through its filters a strip of rows at a
        // time and written as PNG rows; the progressive JPEG goes through
        // byte for byte in 64 KiB reads and 256 KiB data frames.
        ("extractImages", "huge" | "hugeJpeg") => 16 * MIB,
        // The same 60 MiB image, row by row.
        ("extractImages", "hugeFax") => 16 * MIB,
        // The same strips go through the row resampler to the placement's
        // size and into a JPEG encoder that takes MCU rows, so the host holds
        // a strip, the resampler's rows and the encoded output (under 1 MiB).
        ("reduceImages", "huge") => 16 * MIB,
        // The 50 MiB packed bitmap of the huge fax decodes two transition
        // lines and one row at a time, in render, extract and reduce alike.
        ("render" | "reduceImages", "hugeFax") => 16 * MIB,
        // The huge stencil is a 50 MiB ImageMask whose samples the row
        // cursor takes off its Flate reader one row at a time, in render
        // and reduce alike.
        ("render" | "reduceImages", "hugeStencil") => 16 * MIB,
        // The ruled exception: JBIG2 composes the page bitmap before its
        // callback runs (d-122), so the host holds the codestream, its
        // globals, the decoder's 1-bit page bitmap and the packed copy its
        // callback fills before that bitmap is freed. The targets are
        // the measured peaks rounded up to the next MiB; a decoder that
        // streams regions is the task that stops it being an exception.
        ("render", "hugeJbig2") => 98 * MIB,
        ("extractImages", "hugeJbig2") => 96 * MIB,
        // The ruled exception again: the progressive JPEG is decoded whole, by
        // the inverse DCT at the reduction that covers the placement.
        ("reduceImages", "hugeJpeg") => 56 * MIB,
        // A 2000 pt square at 300 dpi again, now with eight groups inside one
        // another, a soft mask, blends and patterns: each level holds one band
        // of its own, and the band is cut by the depth, so the host holds the
        // same few megabytes a page of plain paths does.
        ("render", "groups") => 16 * MIB,
        ("fullRewriteSave", "encrypted") => 16 * MIB,
        // The resource prune reads every content once, as operators.
        ("compress", "content-fl" | "content-a85" | "content-lzw" | "forms" | "encrypted") => {
            16 * MIB
        },
        ("redact", "content-fl" | "content-a85" | "content-lzw") => 16 * MIB,
        // The pass reads 64 MiB of rulings one operator at a time and writes
        // each cut or kept; 2,000 forms across the box are each redacted as a
        // copy of their own, streamed from their source into the save.
        ("redact", "paths" | "forms") => 16 * MIB,
        // The copy of a raw or fax image streams its rows, the covered ones
        // filled, into the save's encoder; a box over all of it writes none.
        ("redact", "huge" | "hugeFull" | "hugeFax") => 16 * MIB,
        // A JPEG is copied in the DCT domain, one MCU row of coefficients at a
        // time: no pixel is decoded.
        ("redact", "hugeJpeg") => 16 * MIB,
        // 10,000 widgets with appearance streams are flattened one appearance
        // at a time, each written as the stream it is stored as.
        ("flattenForms", "widgets") => 16 * MIB,
        // The 96 MiB content stream is read as operators and written as
        // operators; the stage holds one matrix.
        ("setImageBounds", "content-fl") => 16 * MIB,
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
    let damaged = dir.join("damaged.pdf");
    generate_damaged(&damaged)?;
    let objstm = dir.join("objstm.pdf");
    generate_objstm(&objstm)?;
    let forms = dir.join("forms.pdf");
    generate_forms(&forms)?;
    let paths = dir.join("paths.pdf");
    generate_paths(&paths)?;
    let widgets = dir.join("widgets.pdf");
    generate_widgets(&widgets)?;
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

    // The damaged file (no xref table, a startxref that points nowhere) and
    // the packed one (64 MiB of objects in object streams): opened and
    // extracted, then read back, the page count and every page's text.
    for (name, path) in [("damaged", &damaged), ("objstm", &objstm)] {
        step(&failed, "open", name, limit("open", name), |st| {
            let resp = call(st, re("open"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("open returns handleId");
            let pages = resp.i32("pageCount").expect("open returns pageCount");
            call(st, re("docDispose").put("handleId", i32v(hid)), vec![], vec![])?;
            if pages as usize != PACKED_PAGES {
                return Err(format!("{pages} pages, expected {PACKED_PAGES}"));
            }
            Ok(())
        });
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
        // The read-back runs after the steps, through the reader's whole-file
        // open, so its memory is not theirs.
        let checked = check_packed_text(path, &open_sink(name, "extractall"));
        match checked {
            Ok(()) => eprintln!(
                "memprobe: {name}: read back: {PACKED_PAGES} pages, the text of page {} and of every page on the wire",
                PACKED_PAGES / 2
            ),
            Err(e) => {
                eprintln!("memprobe: {name}: READ-BACK FAIL: {e}");
                failed.set(true);
            },
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
    // redact the left part of the first text line of each content source,
    // apply, save, then read the saved file back: the covered start of the
    // line is gone from the text and the uncovered end of it stays.
    for (name, path) in contents.iter().map(|(name, path)| (*name, path.as_path())) {
        // The measured step is the redaction and its save. The read-back below
        // runs after it, through the reader's whole-file open, so its memory
        // is not the redaction's.
        let saved_slot: std::cell::RefCell<Option<std::path::PathBuf>> = std::cell::RefCell::new(None);
        step(&failed, "redact", name, limit("redact", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            let mut mark = re("editorMutate")
                .put("handleId", i32v(hid))
                .put("editOp", strv("addRedaction"))
                .put("page", i32v(0));
            for (key, value) in [("x", 0.0), ("y", 8.0), ("width", 300.0), ("height", 16.0)] {
                mark = mark.put(key, wire::Value::F64(value));
            }
            call(st, mark, vec![], vec![])?;
            call(
                st,
                re("editorMutate")
                    .put("handleId", i32v(hid))
                    .put("editOp", strv("applyRedactions")),
                vec![],
                vec![],
            )?;
            let saved = open_sink(name, "redacted");
            save_full(st, hid, &saved, false)?;
            dispose(st, hid);
            *saved_slot.borrow_mut() = Some(saved);
            Ok(())
        });

        // The read-back: the covered start of the line is gone from the text,
        // the uncovered end of it stays.
        let Some(saved) = saved_slot.into_inner() else {
            continue;
        };
        let text_of = |p: &Path| -> Result<String, String> {
            pdf_oxide::PdfDocument::open(p)
                .and_then(|doc| doc.extract_text(0))
                .map_err(|e| e.to_string())
        };
        let checked = (|| -> Result<(), String> {
            let before = text_of(path)?;
            let after = text_of(&saved)?;
            let (Some(covered), Some(kept)) = (before.get(..12), before.get(80..92)) else {
                return Err(format!("the source line is {} characters, too short", before.len()));
            };
            if after.contains(covered) || !after.contains(kept) {
                return Err(format!("the redacted text is wrong: {after:.120}"));
            }
            eprintln!("memprobe: redact {name}: text check ok, the covered start is gone and the end stays");
            Ok(())
        })();
        if let Err(e) = checked {
            eprintln!("memprobe: redact {name}: TEXT FAIL: {e}");
            failed.set(true);
        }
    }

    // redact a box over the paths input and over the forms input (every form's
    // box is the whole page, so every form is cut across the box): the saved
    // file reopens, and the box renders as the fill.
    for (name, path, rect) in [
        ("paths", paths.as_path(), [150.0, 300.0, 200.0, 150.0]),
        ("forms", forms.as_path(), [150.0, 300.0, 200.0, 150.0]),
    ] {
        let saved = open_sink(name, "redacted");
        step(&failed, "redact", name, limit("redact", name), |st| {
            redact_box_to_sink(st, path, rect, &saved)
        });
        match read_back_fill(&saved, rect) {
            Ok(covered) => eprintln!(
                "memprobe: redact {name}: read back: the file reopens, {covered} pixels of the box are the fill"
            ),
            Err(e) => {
                eprintln!("memprobe: redact {name}: READ-BACK FAIL: {e}");
                failed.set(true);
            },
        }
    }

    // flattenForms on 10,000 widgets: the page renders their appearances and
    // keeps no widget.
    {
        let saved = open_sink("widgets", "flattened");
        step(&failed, "flattenForms", "widgets", limit("flattenForms", "widgets"), |st| {
            let resp = call(st, re("editorOpen"), vec![source(&widgets)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            call(
                st,
                re("editorMutate")
                    .put("handleId", i32v(hid))
                    .put("editOp", strv("flattenForms")),
                vec![],
                vec![],
            )?;
            save_full(st, hid, &saved, false)?;
            dispose(st, hid);
            Ok(())
        });
        match read_back_flattened(&saved) {
            Ok(dark) => eprintln!(
                "memprobe: flattenForms widgets: read back: no widget is left, {dark} pixels show the appearances"
            ),
            Err(e) => {
                eprintln!("memprobe: flattenForms widgets: READ-BACK FAIL: {e}");
                failed.set(true);
            },
        }
    }

    // setImageBounds on the content input: the image Im1 its first section
    // draws moves, in a stream of 96 MiB.
    {
        let (name, path) = (contents[0].0, contents[0].1.as_path());
        let saved = open_sink(name, "moved");
        step(&failed, "setImageBounds", name, limit("setImageBounds", name), |st| {
            let resp = call(st, re("editorOpen"), vec![source(path)], vec![])?;
            let hid = resp.i32("handleId").expect("editorOpen returns handleId");
            let mut edit = re("editorMutate")
                .put("handleId", i32v(hid))
                .put("editOp", strv("setImageBounds"))
                .put("page", i32v(0))
                .put("imageName", strv("Im1"));
            for (key, value) in ["x", "y", "width", "height"].into_iter().zip(MOVED_IMAGE) {
                edit = edit.put(key, wire::Value::F64(value));
            }
            call(st, edit, vec![], vec![])?;
            save_full(st, hid, &saved, false)?;
            dispose(st, hid);
            Ok(())
        });
        match read_back_image_bounds(&saved, MOVED_IMAGE) {
            Ok(()) => eprintln!("memprobe: setImageBounds {name}: read back: Im1 is at its new bounds"),
            Err(e) => {
                eprintln!("memprobe: setImageBounds {name}: READ-BACK FAIL: {e}");
                failed.set(true);
            },
        }
    }

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

    // extractImages and reduceImages of the same two pages. Each is read
    // back after the measured step: the extraction's wire from its first
    // frame to the terminator, the reduction by extracting the file it wrote.
    // The reduced sizes are the placement's: 612 x 792 pt at 72 ppi.
    for (name, path, extracted, reduced) in [
        (
            "huge",
            &huge_drawn,
            ExpectedImage { width: 8192, height: 7680, encoding: "raw", color: "gray", depth: 8 },
            ExpectedImage { width: 845, height: 792, encoding: "jpeg", color: "gray", depth: 8 },
        ),
        (
            "hugeJpeg",
            &huge_jpeg,
            ExpectedImage { width: 4096, height: 3072, encoding: "jpeg", color: "rgb", depth: 8 },
            ExpectedImage { width: 1056, height: 792, encoding: "jpeg", color: "rgb", depth: 8 },
        ),
    ] {
        let extract_file = open_sink(name, "extract");
        step(&failed, "extractImages", name, limit("extractImages", name), |st| {
            extract_to_sink(st, path, &extract_file)
        });
        let stored = (name == "hugeJpeg").then(|| stored_jpeg_length(path)).transpose();
        match stored {
            Ok(stored) => verify_extract_wire(&failed, name, &extract_file, &extracted, stored),
            Err(e) => {
                eprintln!("memprobe: extractImages {name}: {e}");
                failed.set(true);
            },
        }
        let reduced_file = open_sink(name, "reduced");
        step(&failed, "reduceImages", name, limit("reduceImages", name), |st| {
            reduce_to_sink(st, path, &reduced_file)
        });
        let again = open_sink(name, "reduced-extract");
        let mut state = pdf_oxide::host::lane_state::LaneState::new();
        let written = if reduced_file.exists() {
            extract_to_sink(&mut state, &reduced_file, &again)
        } else {
            Err("the reduction wrote no file".to_string())
        };
        match written {
            Ok(()) => verify_extract_wire(&failed, &format!("{name} reduced"), &again, &reduced, None),
            Err(e) => {
                eprintln!("memprobe: reduceImages {name}: read back: {e}");
                failed.set(true);
            },
        }
        eprintln!(
            "memprobe: reduceImages {name}: {} bytes in, {} bytes out",
            std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            std::fs::metadata(&reduced_file).map(|m| m.len()).unwrap_or(0)
        );
    }

    // The huge fax: a 50 MiB packed bitmap that decodes a row at a time.
    // Render and the extraction read back; the reduction by extracting the
    // file it wrote.
    let huge_fax = dir.join("huge-fax.pdf");
    generate_huge_fax(&huge_fax)?;
    step(&failed, "render", "hugeFax", limit("render", "hugeFax"), |st| {
        render_to_sink(st, &huge_fax, 150, &open_sink("hugeFax", "render"))
    });
    verify_render_wire(&failed, "hugeFax", &open_sink("hugeFax", "render"));
    let fax_extract = open_sink("hugeFax", "extract");
    step(&failed, "extractImages", "hugeFax", limit("extractImages", "hugeFax"), |st| {
        extract_to_sink(st, &huge_fax, &fax_extract)
    });
    verify_extract_wire(
        &failed,
        "hugeFax",
        &fax_extract,
        &ExpectedImage {
            width: 20000,
            height: 20000,
            encoding: "ccitt",
            color: "bilevel",
            depth: 1,
        },
        None,
    );
    let fax_reduced = open_sink("hugeFax", "reduced");
    step(&failed, "reduceImages", "hugeFax", limit("reduceImages", "hugeFax"), |st| {
        reduce_to_sink(st, &huge_fax, &fax_reduced)
    });
    let fax_again = open_sink("hugeFax", "reduced-extract");
    let mut state = pdf_oxide::host::lane_state::LaneState::new();
    let written_fax = if fax_reduced.exists() {
        extract_to_sink(&mut state, &fax_reduced, &fax_again)
    } else {
        Err("the reduction wrote no file".to_string())
    };
    match written_fax {
        Ok(()) => {
            // The reduction re-encodes a bilevel image as Group 4 at the
            // mono resolutions the step set.
            verify_extract_wire(
                &failed,
                "hugeFax reduced",
                &fax_again,
                &                ExpectedImage {
                    // The reducer scales both axes by the placement's ppi
                    // under the placement mono 300 ppi (20000/792 pt), so
                    // the square bitmap comes out 3300 x 3300.
                    width: 3300,
                    height: 3300,
                    encoding: "ccitt",
                    color: "bilevel",
                    depth: 1,
                },
                None,
            );
        },
        Err(e) => {
            eprintln!("memprobe: reduceImages hugeFax: read back: {e}");
            failed.set(true);
        },
    }
    // The huge stencil: a 50 MiB ImageMask whose packed bitmap the row
    // cursor never builds. Render and the reduction read back; the
    // reduction by extracting the file it wrote.
    let huge_stencil = dir.join("huge-stencil.pdf");
    generate_huge_stencil(&huge_stencil)?;
    step(&failed, "render", "hugeStencil", limit("render", "hugeStencil"), |st| {
        render_to_sink(st, &huge_stencil, 150, &open_sink("hugeStencil", "render"))
    });
    verify_render_wire(&failed, "hugeStencil", &open_sink("hugeStencil", "render"));
    let stencil_reduced = open_sink("hugeStencil", "reduced");
    step(&failed, "reduceImages", "hugeStencil", limit("reduceImages", "hugeStencil"), |st| {
        reduce_to_sink(st, &huge_stencil, &stencil_reduced)
    });
    let stencil_again = open_sink("hugeStencil", "reduced-extract");
    let mut state = pdf_oxide::host::lane_state::LaneState::new();
    let written_stencil = if stencil_reduced.exists() {
        extract_to_sink(&mut state, &stencil_reduced, &stencil_again)
    } else {
        Err("the reduction wrote no file".to_string())
    };
    match written_stencil {
        Ok(()) => {
            // The reduction re-encodes a bilevel image as Group 4 at the
            // mono resolutions the step set, the square bitmap the huge
            // fax's reduction came out at.
            verify_extract_wire(
                &failed,
                "hugeStencil reduced",
                &stencil_again,
                &ExpectedImage {
                    width: 3300,
                    height: 3300,
                    encoding: "ccitt",
                    color: "bilevel",
                    depth: 1,
                },
                None,
            );
        },
        Err(e) => {
            eprintln!("memprobe: reduceImages hugeStencil: read back: {e}");
            failed.set(true);
        },
    }

    // The image redactions of the huge inputs: a box over the lower-left
    // quarter of each page, then a box over all of it. The measured step is
    // the redaction and its save; the saved file is read back after it.
    let quarter = [0.0, 0.0, 306.0, 396.0];
    let whole = [0.0, 0.0, 612.0, 792.0];
    for (name, path, rect) in [
        ("huge", &huge_drawn, quarter),
        ("hugeFull", &huge_drawn, whole),
        ("hugeJpeg", &huge_jpeg, quarter),
        ("hugeFax", &huge_fax, quarter),
    ] {
        let saved = open_sink(name, "redacted");
        step(&failed, "redact", name, limit("redact", name), |st| {
            redact_box_to_sink(st, path, rect, &saved)
        });
        let checked: Result<(), String> = (|| {
            if !saved.exists() {
                return Err("the redaction wrote no file".to_string());
            }
            match name {
                "hugeFull" => {
                    let doc = pdf_oxide::PdfDocument::open(&saved).map_err(|e| e.to_string())?;
                    let images = doc.extract_images(0).map_err(|e| e.to_string())?;
                    if !images.is_empty() {
                        return Err(format!("{} images left under the full box", images.len()));
                    }
                    eprintln!("memprobe: redact hugeFull: read back: no image on page 0");
                    Ok(())
                },
                "hugeJpeg" => {
                    let doc = pdf_oxide::PdfDocument::open(&saved).map_err(|e| e.to_string())?;
                    let mut images = doc.extract_images(0).map_err(|e| e.to_string())?;
                    if images.len() != 1 {
                        return Err(format!("{} images on page 0, expected 1", images.len()));
                    }
                    let image = images.remove(0);
                    let (w, h) = (image.width(), image.height());
                    match image.data() {
                        pdf_oxide::extractors::ImageData::Jpeg(bytes) => {
                            if (w, h) != (4096, 3072) {
                                return Err(format!("the JPEG is {w}x{h}, expected 4096x3072"));
                            }
                            if bytes.get(..2) != Some(&[0xFF, 0xD8]) || bytes.get(bytes.len() - 2..) != Some(&[0xFF, 0xD9]) {
                                return Err("the extracted JPEG does not run from FFD8 to FFD9".to_string());
                            }
                            eprintln!(
                                "memprobe: redact hugeJpeg: read back: JPEG {w}x{h}, {} bytes",
                                bytes.len()
                            );
                            Ok(())
                        },
                        pdf_oxide::extractors::ImageData::Raw { .. } => {
                            Err("the image extracted as raw pixels, expected JPEG".to_string())
                        },
                    }
                },
                "hugeFax" => {
                    // The copy read back through the wire `extractImages`, the
                    // product's path: its PNG decoded here, 1-bit gray.
                    let wire_file = open_sink("hugeFax", "redacted-wire");
                    let mut state = pdf_oxide::host::lane_state::LaneState::new();
                    extract_to_sink(&mut state, &saved, &wire_file)?;
                    let (w, h, depth, rows) = decode_gray_png(&wire_png(&wire_file)?)?;
                    if (w, h, depth) != (20000, 20000, 1) {
                        return Err(format!("the wire PNG is {w}x{h} at {depth} bits, expected 20000x20000 at 1"));
                    }
                    let row = w.div_ceil(8) as usize;
                    // The source is the band generate_huge_fax draws: black (0)
                    // from `left` for 1000 columns, white (1) elsewhere.
                    let band = |x: u32, y: u32| -> u8 {
                        let left = (y as usize * 997) % (20000 - 1000);
                        let x = x as usize;
                        if x >= left && x < left + 1000 { 0 } else { 1 }
                    };
                    let copy_bit = |x: u32, y: u32| -> u8 {
                        (rows[y as usize * row + x as usize / 8] >> (7 - x % 8)) & 1
                    };
                    let cover = Cover::new(page_image_ctm(), quarter);
                    let covered = check_exact((w, h), &cover, 0, band, copy_bit)?;
                    eprintln!("memprobe: redact hugeFax: wire read back: {covered} covered pixels are the fill, every other pixel is the source's");
                    Ok(())
                },
                _ => {
                    let (w, h, source) = read_back_raster(path)?;
                    let (cw, ch, copy) = read_back_raster(&saved)?;
                    if (cw, ch) != (w, h) {
                        return Err(format!("the copy is {cw}x{ch}, the source {w}x{h}"));
                    }
                    let cover = Cover::new(page_image_ctm(), quarter);
                    let covered = check_exact(
                        (w, h),
                        &cover,
                        0,
                        |x, y| source[(y * w + x) as usize],
                        |x, y| copy[(y * w + x) as usize],
                    )?;
                    eprintln!("memprobe: redact {name}: read back: {covered} covered samples are the fill, every other sample is the source's");
                    Ok(())
                },
            }
        })();
        if let Err(e) = checked {
            eprintln!("memprobe: redact {name}: READ-BACK FAIL: {e}");
            failed.set(true);
        }
    }

    // The ruled exception: JBIG2 holds one page bitmap. Render and
    // extraction at the measured peak.
    let huge_jbig2 = dir.join("huge-jbig2.pdf");
    generate_huge_jbig2(&huge_jbig2)?;
    step(&failed, "render", "hugeJbig2", limit("render", "hugeJbig2"), |st| {
        render_to_sink(st, &huge_jbig2, 150, &open_sink("hugeJbig2", "render"))
    });
    verify_render_wire(&failed, "hugeJbig2", &open_sink("hugeJbig2", "render"));
    let jbig2_extract = open_sink("hugeJbig2", "extract");
    step(&failed, "extractImages", "hugeJbig2", limit("extractImages", "hugeJbig2"), |st| {
        extract_to_sink(st, &huge_jbig2, &jbig2_extract)
    });
    verify_extract_wire(
        &failed,
        "hugeJbig2",
        &jbig2_extract,
        &ExpectedImage {
            width: 20000,
            height: 20000,
            encoding: "jbig2",
            color: "bilevel",
            depth: 1,
        },
        None,
    );

    // render of one large page of transparency groups nested eight deep, a
    // soft mask, blends and tiling patterns, one of them over the raster
    // budget, at 300 dpi.
    let groups = dir.join("groups.pdf");
    generate_groups(&groups)?;
    step(&failed, "render", "groups", limit("render", "groups"), |st| {
        render_to_sink(st, &groups, 300, &open_sink("groups", "render"))
    });
    verify_render_wire(&failed, "groups", &open_sink("groups", "render"));

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
                let (depth, colour) = walk.finish(w, h)?;
                if depth != 8 || colour != 6 {
                    return Err(format!("IHDR depth {depth} colour type {colour}, not 8-bit RGBA"));
                }
                pages += 1;
            },
            other => return Err(format!("unknown frame kind {other:?}")),
        }
    }
}

/// Opens `path`, writes every image drawn on it to `sink_path` as the chunked
/// wire, and disposes the document: the host path a Dart `extractImages`
/// takes.
fn extract_to_sink(
    state: &mut pdf_oxide::host::lane_state::LaneState,
    path: &Path,
    sink_path: &Path,
) -> Result<(), String> {
    let resp = call(state, re("open"), vec![source(path)], vec![])?;
    let hid = resp.i32("handleId").expect("open returns handleId");
    call(
        state,
        re("extractImages")
            .put("handleId", i32v(hid))
            .put("hasSink", wire::Value::Bool(true)),
        vec![],
        vec![sink(sink_path)],
    )?;
    call(state, re("docDispose").put("handleId", i32v(hid)), vec![], vec![])?;
    Ok(())
}

/// Opens `path` as an editor, reduces its images to 72 ppi (the screen
/// preset's resolutions and quality), saves the document to `sink_path` and
/// disposes the editor: the host path a Dart `reduceImages` and a save take.
fn reduce_to_sink(
    state: &mut pdf_oxide::host::lane_state::LaneState,
    path: &Path,
    sink_path: &Path,
) -> Result<(), String> {
    let resp = call(state, re("editorOpen"), vec![source(path)], vec![])?;
    let hid = resp.i32("handleId").expect("editorOpen returns handleId");
    call(
        state,
        re("editorMutate")
            .put("handleId", i32v(hid))
            .put("editOp", strv("reduceImages"))
            .put("colorPpi", wire::Value::F64(72.0))
            .put("grayPpi", wire::Value::F64(72.0))
            .put("monoPpi", wire::Value::F64(300.0))
            .put("jpegQuality", i32v(60))
            .put("allowLossy", wire::Value::Bool(true))
            .put("recompressJpeg", wire::Value::Bool(true)),
        vec![],
        vec![],
    )?;
    save_full(state, hid, sink_path, false)?;
    dispose(state, hid);
    Ok(())
}

/// Opens `path` as an editor, adds the redaction box `rect` (page points,
/// x y width height) on page 0, applies it, saves to `sink_path` and disposes
/// the editor: the host path a Dart `redact` and its save take.
fn redact_box_to_sink(
    state: &mut pdf_oxide::host::lane_state::LaneState,
    path: &Path,
    rect: [f64; 4],
    sink_path: &Path,
) -> Result<(), String> {
    let resp = call(state, re("editorOpen"), vec![source(path)], vec![])?;
    let hid = resp.i32("handleId").expect("editorOpen returns handleId");
    let mut mark = re("editorMutate")
        .put("handleId", i32v(hid))
        .put("editOp", strv("addRedaction"))
        .put("page", i32v(0));
    for (key, value) in ["x", "y", "width", "height"].into_iter().zip(rect) {
        mark = mark.put(key, wire::Value::F64(value));
    }
    call(state, mark, vec![], vec![])?;
    call(
        state,
        re("editorMutate")
            .put("handleId", i32v(hid))
            .put("editOp", strv("applyRedactions")),
        vec![],
        vec![],
    )?;
    save_full(state, hid, sink_path, false)?;
    dispose(state, hid);
    Ok(())
}

/// Reopens the redacted file `path` and renders page 0 at 72 dpi, outside any
/// measured step: every pixel inside `rect` (x, y, width, height in page
/// points, y up), inset by two pixels, is one opaque colour that is not the
/// white page. Returns how many pixels were read.
fn read_back_fill(path: &Path, rect: [f64; 4]) -> Result<usize, String> {
    use pdf_oxide::rendering::{render_page, ImageFormat, RenderOptions};
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| e.to_string())?;
    let mut options = RenderOptions::with_dpi(72);
    options.format = ImageFormat::RawRgba8;
    let image = render_page(&doc, 0, &options).map_err(|e| e.to_string())?;
    let (width, height) = (image.width as usize, image.height as usize);
    let at = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * width + x) * 4;
        [image.data[i], image.data[i + 1], image.data[i + 2], image.data[i + 3]]
    };
    let (x0, x1) = (rect[0] as usize + 2, (rect[0] + rect[2]) as usize - 2);
    let (y0, y1) = (height - (rect[1] + rect[3]) as usize + 2, height - rect[1] as usize - 2);
    let fill = at(x0, y0);
    if fill[3] != 255 || fill[..3] == [255, 255, 255] {
        return Err(format!("the box shows {fill:?}, not a fill"));
    }
    let mut covered = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            if at(x, y) != fill {
                return Err(format!("pixel ({x}, {y}) is {:?}, the fill is {fill:?}", at(x, y)));
            }
            covered += 1;
        }
    }
    Ok(covered)
}

/// Where `setImageBounds` puts the content input's image `Im1` (x, y, width,
/// height in page points).
const MOVED_IMAGE: [f64; 4] = [30.0, 40.0, 70.0, 80.0];

/// The page 0 of `path` draws `Im1` at `want` (x, y, width, height): the
/// matrix in force at its first `Do`, read as operators, outside any
/// measured step.
fn read_back_image_bounds(path: &Path, want: [f64; 4]) -> Result<(), String> {
    use pdf_oxide::content::operators::Operator;
    type M = [f64; 6];
    let mul = |m: M, n: M| -> M {
        [
            m[0] * n[0] + m[1] * n[2],
            m[0] * n[1] + m[1] * n[3],
            m[2] * n[0] + m[3] * n[2],
            m[2] * n[1] + m[3] * n[3],
            m[4] * n[0] + m[5] * n[2] + n[4],
            m[4] * n[1] + m[5] * n[3] + n[5],
        ]
    };
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| e.to_string())?;
    let mut ctm: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut saved = Vec::new();
    for op in doc.page_operators(0).map_err(|e| e.to_string())? {
        match op.map_err(|e| e.to_string())? {
            Operator::SaveState => saved.push(ctm),
            Operator::RestoreState => {
                if let Some(m) = saved.pop() {
                    ctm = m;
                }
            },
            Operator::Cm { a, b, c, d, e, f } => {
                ctm = mul([a.into(), b.into(), c.into(), d.into(), e.into(), f.into()], ctm);
            },
            Operator::Do { name } if name == "Im1" => {
                let got = [ctm[4], ctm[5], ctm[0], ctm[3]];
                return if got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01) {
                    Ok(())
                } else {
                    Err(format!("Im1 is drawn at {got:?}, wanted {want:?}"))
                };
            },
            _ => {},
        }
    }
    Err("the page never draws Im1".to_string())
}

/// The flattened widgets file: no widget on page 0, and the appearances
/// render. Returns how many dark pixels the page shows at 72 dpi.
fn read_back_flattened(path: &Path) -> Result<usize, String> {
    use pdf_oxide::rendering::{render_page, ImageFormat, RenderOptions};
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| e.to_string())?;
    let left = doc.get_annotations(0).map_err(|e| e.to_string())?.len();
    if left != 0 {
        return Err(format!("{left} annotations are left on page 0"));
    }
    let mut options = RenderOptions::with_dpi(72);
    options.format = ImageFormat::RawRgba8;
    let image = render_page(&doc, 0, &options).map_err(|e| e.to_string())?;
    let dark = image.data.chunks_exact(4).filter(|p| p[0] < 64 && p[3] == 255).count();
    // 10,000 widgets of 5 by 6 points over a page of 612 by 792.
    if dark < 150_000 {
        return Err(format!("only {dark} dark pixels: the appearances do not render"));
    }
    Ok(dark)
}

/// The one image page 0 of `path` paints, decoded by the library reader
/// outside any measured step. Its raw pixels are one byte a sample.
fn read_back_raster(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    use pdf_oxide::extractors::ImageData;
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| e.to_string())?;
    let mut images = doc.extract_images(0).map_err(|e| e.to_string())?;
    if images.len() != 1 {
        return Err(format!("{} images on page 0, expected 1", images.len()));
    }
    let image = images.remove(0);
    let (w, h) = (image.width(), image.height());
    match image.data() {
        ImageData::Raw { pixels, .. } => Ok((w, h, pixels.clone())),
        ImageData::Jpeg(_) => Err("the image extracted as JPEG, expected raw pixels".to_string()),
    }
}

/// The CTM of the page's one image: the unit square drawn over the whole
/// Letter page, `q 612 0 0 792 0 0 cm /Im Do Q`.
fn page_image_ctm() -> pdf_oxide::content::graphics_state::Matrix {
    pdf_oxide::content::graphics_state::Matrix { a: 612.0, b: 0.0, c: 0.0, d: 792.0, e: 0.0, f: 0.0 }
}

/// The pixels the pass covers in an image placed by `ctm` under the page
/// rectangle `rect` (x, y, width, height in page points). It is the pass's own
/// decision: `classify_image_placement` over the region padded by the default
/// edge padding, and the fill rule the copy takes (a pixel is the fill when its
/// centre lies in the covered rectangle the decision returns).
enum Cover {
    Nothing,
    Everything,
    Rect { u0: f32, v0: f32, u1: f32, v1: f32 },
}

impl Cover {
    fn new(ctm: pdf_oxide::content::graphics_state::Matrix, rect: [f64; 4]) -> Cover {
        use pdf_oxide::redaction::image_prune::{classify_image_placement, ImageRedaction};
        use pdf_oxide::redaction::{RedactionRegion, RegionSet, DEFAULT_EDGE_PADDING};
        let [x, y, width, height] = rect;
        let mut regions = RegionSet::new(0);
        regions.push(RedactionRegion::from_rect(
            x as f32,
            y as f32,
            (x + width) as f32,
            (y + height) as f32,
            None,
        ));
        match classify_image_placement(&ctm, &regions, DEFAULT_EDGE_PADDING) {
            ImageRedaction::Keep => Cover::Nothing,
            ImageRedaction::DeleteFull => Cover::Everything,
            ImageRedaction::Overwrite { u0, v0, u1, v1 } => Cover::Rect { u0, v0, u1, v1 },
        }
    }

    fn contains(&self, x: u32, y: u32, w: u32, h: u32) -> bool {
        match *self {
            Cover::Nothing => false,
            Cover::Everything => true,
            Cover::Rect { u0, v0, u1, v1 } => {
                let u = (f64::from(x) + 0.5) / f64::from(w);
                let v = 1.0 - (f64::from(y) + 0.5) / f64::from(h);
                f64::from(u0) <= u && u < f64::from(u1) && f64::from(v0) <= v && v < f64::from(v1)
            },
        }
    }
}

/// Every pixel of a `w x h` copy against its expected value: a covered pixel
/// is `fill`, an uncovered one is the source's. Returns the covered count.
fn check_exact(
    (w, h): (u32, u32),
    cover: &Cover,
    fill: u8,
    source: impl Fn(u32, u32) -> u8,
    copy: impl Fn(u32, u32) -> u8,
) -> Result<u64, String> {
    let mut covered = 0u64;
    for y in 0..h {
        for x in 0..w {
            let got = copy(x, y);
            if cover.contains(x, y, w, h) {
                covered += 1;
                if got != fill {
                    return Err(format!("covered pixel ({x}, {y}) is {got}, the fill is {fill}"));
                }
            } else if got != source(x, y) {
                return Err(format!("uncovered pixel ({x}, {y}) is {got}, the source's is {}", source(x, y)));
            }
        }
    }
    if covered == 0 {
        return Err("no pixel is covered".to_string());
    }
    Ok(covered)
}

/// The PNG one `extractImages` item carries on the wire: its data frames in
/// order, read the way the frames are laid down, with exactly one item.
fn wire_png(sink_path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = File::open(sink_path).map_err(|e| format!("open sink: {e}"))?;
    let mut file = std::io::BufReader::new(file);
    let (mut png, mut items) = (Vec::new(), 0usize);
    loop {
        let mut len = [0u8; 4];
        file.read_exact(&mut len).map_err(|e| format!("terminator missing: {e}"))?;
        let len = u32::from_le_bytes(len) as usize;
        if len == 0 {
            break;
        }
        let mut frame = vec![0u8; len];
        file.read_exact(&mut frame).map_err(|e| format!("truncated frame: {e}"))?;
        let frame = wire::parse_response(&frame)?;
        match frame.str("kind") {
            Some("header") => items += 1,
            Some("data") => png.extend_from_slice(frame.bytes("data").ok_or("a data frame without data")?),
            Some("end") => {},
            other => return Err(format!("unknown frame kind {other:?}")),
        }
    }
    if items != 1 {
        return Err(format!("{items} items, expected 1"));
    }
    Ok(png)
}

/// The width, height, bit depth and packed rows of a gray PNG, unfiltered by
/// the PNG filter each row names (ISO 15948 §9). A row is `ceil(width * depth
/// / 8)` bytes; the filter's byte distance is one byte at these depths.
fn decode_gray_png(png: &[u8]) -> Result<(u32, u32, u8, Vec<u8>), String> {
    use std::io::Read;
    if png.get(..8) != Some(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Err("the wire item is not a PNG".to_string());
    }
    let (mut at, mut ihdr, mut idat) = (8usize, None, Vec::new());
    while at + 8 <= png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().expect("four bytes")) as usize;
        let kind = &png[at + 4..at + 8];
        let data = png.get(at + 8..at + 8 + len).ok_or("a PNG chunk runs past the item")?;
        match kind {
            b"IHDR" => ihdr = Some(data.to_vec()),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {},
        }
        at += 12 + len;
    }
    let ihdr = ihdr.ok_or("the PNG has no IHDR")?;
    let w = u32::from_be_bytes(ihdr[0..4].try_into().expect("four bytes"));
    let h = u32::from_be_bytes(ihdr[4..8].try_into().expect("four bytes"));
    let (depth, colour) = (ihdr[8], ihdr[9]);
    if colour != 0 {
        return Err(format!("the PNG's colour type is {colour}, expected gray (0)"));
    }
    let row = (w as usize * depth as usize).div_ceil(8);
    let mut raw = Vec::with_capacity((row + 1) * h as usize);
    flate2::read::ZlibDecoder::new(&idat[..])
        .read_to_end(&mut raw)
        .map_err(|e| format!("the PNG's IDAT does not inflate: {e}"))?;
    if raw.len() != (row + 1) * h as usize {
        return Err(format!("{} filtered bytes, {h} rows of {} expected", raw.len(), row + 1));
    }
    let mut out = vec![0u8; row * h as usize];
    for y in 0..h as usize {
        let filter = raw[y * (row + 1)];
        let src = &raw[y * (row + 1) + 1..(y + 1) * (row + 1)];
        for x in 0..row {
            let left = if x >= 1 { out[y * row + x - 1] } else { 0 };
            let (up, up_left) = if y >= 1 {
                let prev = (y - 1) * row;
                (out[prev + x], if x >= 1 { out[prev + x - 1] } else { 0 })
            } else {
                (0, 0)
            };
            let predicted = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => {
                    let (a, b, c) = (i16::from(left), i16::from(up), i16::from(up_left));
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    (if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }) as u8
                },
                other => return Err(format!("PNG filter type {other}")),
            };
            out[y * row + x] = src[x].wrapping_add(predicted);
        }
    }
    Ok((w, h, depth, out))
}

/// What the descriptor of an extracted image must say.
struct ExpectedImage {
    width: i32,
    height: i32,
    encoding: &'static str,
    color: &'static str,
    /// The PNG's bit depth: 8 for the gray and RGB rasters, 1 for a bilevel
    /// one.
    depth: u8,
}

/// The length of the one DCT stream a probe input holds, from its
/// dictionary's `/Length`.
fn stored_jpeg_length(path: &Path) -> Result<u64, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let at = bytes
        .windows(10)
        .position(|w| w == b"/DCTDecode")
        .ok_or("no DCT stream in the input")?;
    let rest = &bytes[at..(at + 64).min(bytes.len())];
    let text = String::from_utf8_lossy(rest);
    text.split("/Length ")
        .nth(1)
        .and_then(|t| t.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| "no /Length after the DCT stream's filter".to_string())
}

fn verify_extract_wire(
    failed: &std::cell::Cell<bool>,
    input: &str,
    sink_path: &Path,
    expect: &ExpectedImage,
    stored_jpeg: Option<u64>,
) {
    let started = std::time::Instant::now();
    match check_extract_wire(sink_path, expect, stored_jpeg) {
        Ok(summary) => eprintln!(
            "memprobe: extractImages {input}: wire ok, {summary} (read back in {} ms)",
            started.elapsed().as_millis()
        ),
        Err(e) => {
            eprintln!("memprobe: extractImages {input}: WIRE FAIL: {e}");
            failed.set(true);
        },
    }
}

/// Reads the wire an extraction left in `sink_path` from its first frame to
/// the terminator: one item, a header that says what `expect` says, data
/// frames of at most 256 KiB and an end frame. A JPEG's data is one stream
/// from FFD8 to FFD9 of the stored length, any other image's one whole PNG
/// of the descriptor's size, 8-bit, gray for `gray` and RGB for `rgb`.
fn check_extract_wire(
    sink_path: &Path,
    expect: &ExpectedImage,
    stored_jpeg: Option<u64>,
) -> Result<String, String> {
    use std::io::Read;
    let file = File::open(sink_path).map_err(|e| format!("open sink: {e}"))?;
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut file = std::io::BufReader::new(file);
    let mut read = 0u64;
    let mut items = 0usize;
    let mut open: Option<(i32, i32, bool, PngWalk)> = None;
    let (mut data_bytes, mut data_frames, mut largest) = (0u64, 0usize, 0usize);
    let (mut head, mut tail) = ([0u8; 2], [0u8; 2]);
    loop {
        let mut len = [0u8; 4];
        file.read_exact(&mut len).map_err(|e| format!("terminator missing: {e}"))?;
        read += 4;
        let len = u32::from_le_bytes(len) as usize;
        if len == 0 {
            if open.is_some() {
                return Err("the terminator came inside an item".into());
            }
            if read != total {
                return Err(format!("{} bytes follow the terminator", total - read));
            }
            if items != 1 {
                return Err(format!("{items} items, expected 1"));
            }
            if expect.encoding == "jpeg" {
                if head != [0xFF, 0xD8] || tail != [0xFF, 0xD9] {
                    return Err(format!("a JPEG from {head:02x?} to {tail:02x?}"));
                }
                if let Some(stored) = stored_jpeg {
                    if stored != data_bytes {
                        return Err(format!("{data_bytes} JPEG bytes, the file stores {stored}"));
                    }
                }
            }
            return Ok(format!(
                "{} {}x{} {}, {data_frames} data frames (largest {largest} bytes) summing \
                 {data_bytes} bytes, {total} bytes on the wire",
                expect.encoding, expect.width, expect.height, expect.color
            ));
        }
        let mut frame = vec![0u8; len];
        file.read_exact(&mut frame).map_err(|e| format!("truncated frame: {e}"))?;
        read += len as u64;
        let frame = wire::parse_response(&frame)?;
        match frame.str("kind") {
            Some("header") => {
                if open.is_some() {
                    return Err("a header inside an item".into());
                }
                let (w, h) = (frame.i32("width").unwrap_or(0), frame.i32("height").unwrap_or(0));
                let said = (w, h, frame.str("encoding").unwrap_or(""), frame.str("color").unwrap_or(""));
                if said != (expect.width, expect.height, expect.encoding, expect.color) {
                    return Err(format!("the header says {said:?}"));
                }
                if (frame.i32("page"), frame.i32("index")) != (Some(0), Some(items as i32)) {
                    return Err("the header's page or index is wrong".into());
                }
                open = Some((w, h, expect.encoding == "jpeg", PngWalk::new()));
            },
            Some("data") => {
                let (_, _, jpeg, walk) = open.as_mut().ok_or("data outside an item")?;
                let bytes = frame.bytes("data").ok_or("a data frame without data")?;
                if bytes.len() > 256 * 1024 {
                    return Err(format!("a data frame of {} bytes", bytes.len()));
                }
                if *jpeg {
                    if data_bytes == 0 && bytes.len() >= 2 {
                        head.copy_from_slice(&bytes[..2]);
                    }
                    for &b in &bytes[bytes.len().saturating_sub(2)..] {
                        tail = [tail[1], b];
                    }
                } else {
                    walk.feed(bytes)?;
                }
                data_bytes += bytes.len() as u64;
                data_frames += 1;
                largest = largest.max(bytes.len());
            },
            Some("end") => {
                let (w, h, jpeg, walk) = open.take().ok_or("an end outside an item")?;
                if !jpeg {
                    let (depth, colour) = walk.finish(w, h)?;
                    let want = match expect.color {
                        "gray" | "bilevel" => 0,
                        _ => 2,
                    };
                    if depth != expect.depth || colour != want {
                        return Err(format!(
                            "IHDR depth {depth} colour type {colour}, expected {}-bit type {want}",
                            expect.depth
                        ));
                    }
                }
                items += 1;
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

    /// The item ended: the PNG must be whole and say the size its header did.
    /// Returns its bit depth and colour type.
    fn finish(self, width: i32, height: i32) -> Result<(u8, u8), String> {
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
        if self.idat == 0 {
            return Err("no IDAT".into());
        }
        Ok((ihdr[8], ihdr[9]))
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

/// A one-page PDF 2000 points square of transparency groups nested eight deep
/// (isolated and not, knockout at the third and sixth), each holding rows of
/// small alpha rectangles, the fifth under a luminosity soft mask; on it a
/// blend mode, a small tiling pattern and a tiling pattern whose 600 point
/// cell is a raster of 25 MiB at 300 dpi, drawn tile by tile.
fn generate_groups(path: &Path) -> Result<(), String> {
    const LEVELS: usize = 8;
    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    // Objects 5 to 12 are the groups, 13 the mask group, 14 and 15 the patterns.
    out.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 2000 2000] /Contents 4 0 R \
         /Resources << /XObject << /N1 5 0 R >> /Pattern << /P1 14 0 R /P2 15 0 R >> \
         /ExtGState << /Mul << /BM /Multiply /ca 0.8 >> /Hue << /BM /Hue >> >> >> >>",
    )?;
    let content = "/Pattern cs /P2 scn 0 0 2000 2000 re f\n\
                   q /Mul gs 1 0 0 1 100 100 cm /N1 Do Q\n\
                   q /Hue gs 0.9 0.4 0.1 rg 200 200 1600 300 re f Q\n\
                   /Pattern cs /P1 scn 300 1500 1400 300 re f\n";
    out.stream(4, "", content.len() as u64, content.as_bytes())?;
    for level in 1..=LEVELS {
        let size = 1800 - 120 * level;
        let isolated = if level % 2 == 1 { "true" } else { "false" };
        let knockout = if level == 3 || level == 6 { " /K true" } else { "" };
        let mask = if level == 5 { "/SM gs " } else { "" };
        let mut body = format!("{:.2} 0.5 0.6 rg 0 0 {size} {size} re f /Half gs\n", 0.2 + 0.08 * level as f32);
        // A knockout group replays its content before each element, so it
        // holds few.
        let (rows, columns) = if knockout.is_empty() { (30, 8) } else { (5, 4) };
        for row in 0..rows {
            for column in 0..columns {
                body.push_str(&format!(
                    "{:.2} {:.2} 0.3 rg {} {} 80 20 re f\n",
                    0.1 * column as f32,
                    0.03 * row as f32,
                    20 + column * 100,
                    20 + row * 40
                ));
            }
        }
        if level < LEVELS {
            body.push_str(&format!("/Full gs q {mask}0.98 0.1 -0.1 0.98 40 30 cm /N{} Do Q\n", level + 1));
        }
        let resources = format!(
            "/XObject << /N{} {} 0 R >> /ExtGState << /Half << /ca 0.7 >> /Full << /ca 1 >> \
             /SM << /SMask << /Type /Mask /S /Luminosity /G 13 0 R >> >> >>",
            level + 1,
            5 + level
        );
        out.stream(
            4 + level,
            &format!(
                "/Type /XObject /Subtype /Form /BBox [0 0 {size} {size}] \
                 /Group << /S /Transparency /I {isolated}{knockout} >> /Resources << {resources} >>"
            ),
            body.len() as u64,
            body.as_bytes(),
        )?;
    }
    let mask = "0.4 g 0 0 1000 1000 re f q 0 0 1000 600 re W n /Sh sh Q";
    out.stream(
        13,
        "/Type /XObject /Subtype /Form /BBox [0 0 1000 1000] \
         /Group << /S /Transparency /CS /DeviceGray >> \
         /Resources << /Shading << /Sh << /ShadingType 2 /ColorSpace /DeviceGray \
         /Coords [0 0 1000 1000] /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> \
         /Extend [true true] >> >> >>",
        mask.len() as u64,
        mask.as_bytes(),
    )?;
    let small = "1 0 0 rg 0 0 10 10 re f 0 0 1 rg 10 10 10 10 re f";
    out.stream(
        14,
        "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /Resources << >> \
         /BBox [0 0 20 20] /XStep 20 /YStep 20",
        small.len() as u64,
        small.as_bytes(),
    )?;
    let large = "0.9 0.95 1 rg 0 0 600 600 re f 0.3 0.3 0.7 RG 8 w 20 20 560 560 re S 0.7 0.4 0.2 rg 150 150 m 450 200 l 300 480 l f";
    out.stream(
        15,
        "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /Resources << >> \
         /BBox [0 0 600 600] /XStep 620 /YStep 620",
        large.len() as u64,
        large.as_bytes(),
    )?;
    out.finish()
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

/// A one-page Letter PDF filled by one 20000 x 20000 Group 4 fax image. An
/// inked band of 1000 px drifts across the page as the rows go, and the
/// codestream is written row by row as each row is made: the packed bitmap it
/// decodes to is 50 MiB, which no step may hold.
fn generate_huge_fax(path: &Path) -> Result<(), String> {
    const W: u32 = 20000;
    const H: u32 = 20000;
    let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
    for y in 0..H {
        let left = ((y as usize * 997) % (W as usize - 1000)) as usize;
        let row = std::iter::repeat(fax::Color::White)
            .take(left)
            .chain(std::iter::repeat(fax::Color::Black).take(1000))
            .chain(std::iter::repeat(fax::Color::White).take(W as usize - left - 1000));
        encoder.encode_line(row, W).map_err(|e| format!("hugeFax: {e:?}"))?;
    }
    let code = encoder.finish().expect("the fax finishes").finish();
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
        "/Type /XObject /Subtype /Image /Width 20000 /Height 20000 /ColorSpace /DeviceGray \
         /BitsPerComponent 1 /Filter /CCITTFaxDecode \
         /DecodeParms << /K -1 /Columns 20000 /Rows 20000 >>",
        code.len() as u64,
        std::io::Cursor::new(code),
    )?;
    out.finish()
}

/// A one-page Letter PDF painted with one 20000 x 20000 ImageMask stencil,
/// Flate coded row by row with a PNG predictor. The row cursor reads its
/// samples a row at a time; the packed bitmap the whole decode would build
/// never exists.
fn generate_huge_stencil(path: &Path) -> Result<(), String> {
    use flate2::write::ZlibEncoder;
    const W: u32 = 20000;
    const ROW_BYTES: usize = (W as usize).div_ceil(8);
    let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    for y in 0..W {
        // One predictor row a source row: a filter tag of 0 (none), then
        // the row's sample bits. Under the default /Decode a clear bit
        // paints, so the same band of painted pixels the huge fax draws.
        encoder.write_all(&[0]).map_err(|e| format!("hugeStencil: {e}"))?;
        let mut row = vec![0xFFu8; ROW_BYTES];
        let left = ((y as usize * 997) % ((W as usize) - 1000)) as usize;
        for x in left..left + 1000 {
            row[x / 8] &= !(0x80 >> (x % 8));
        }
        encoder.write_all(&row).map_err(|e| format!("hugeStencil: {e}"))?;
    }
    let code = encoder.finish().map_err(|e| format!("hugeStencil: {e}"))?;
    let content = "0.5 0.3 0.8 rg q 612 0 0 792 0 0 cm /St Do Q";
    let mut out = PdfOut::create(path)?;
    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    out.object(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Resources << /XObject << /St 5 0 R >> >> /Contents 4 0 R >>",
    )?;
    out.stream(4, "", content.len() as u64, content.as_bytes())?;
    out.stream(
        5,
        "/Type /XObject /Subtype /Image /ImageMask true /Width 20000 /Height 20000 \
         /BitsPerComponent 1 /Filter /FlateDecode \
         /DecodeParms << /Predictor 12 /Colors 1 /BitsPerComponent 1 /Columns 20000 >>",
        code.len() as u64,
        std::io::Cursor::new(code),
    )?;
    out.finish()
}

/// A one-page Letter PDF filled by one 20000 x 20000 JBIG2 image: the picture
/// `generate_huge_fax` draws, in one immediate generic region coded with MMR
/// (T.6), beside the page-information segment that declares the size. This is
/// the ruled exception (d-122): JBIG2 composes the page bitmap before its
/// callback runs, so the page bitmap is held.
fn generate_huge_jbig2(path: &Path) -> Result<(), String> {
    const W: u32 = 20000;
    const H: u32 = 20000;
    let mut page = Vec::new();
    for v in [W, H, 0, 0] {
        page.extend_from_slice(&v.to_be_bytes());
    }
    // Flags and striping: bitmap-owned page, single strip.
    page.extend_from_slice(&[0, 0, 0]);
    let mut region = Vec::new();
    for v in [W, H, 0, 0] {
        region.extend_from_slice(&v.to_be_bytes());
    }
    // Flags: generic region, MMR overwrite of the default template.
    region.extend_from_slice(&[0, 0x01]);
    let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
    for y in 0..H {
        let left = ((y as usize * 997) % (W as usize - 1000)) as usize;
        let row = std::iter::repeat(fax::Color::White)
            .take(left)
            .chain(std::iter::repeat(fax::Color::Black).take(1000))
            .chain(std::iter::repeat(fax::Color::White).take(W as usize - left - 1000));
        encoder.encode_line(row, W).map_err(|e| format!("hugeJbig2: {e:?}"))?;
    }
    let mmr = encoder.finish().expect("the MMR finishes").finish();
    region.extend_from_slice(&mmr);
    let bytes = {
        let mut out = Vec::new();
        for (number, kind, data) in [(0u32, 48u8, page), (1u32, 38u8, region)] {
            out.extend_from_slice(&number.to_be_bytes());
            out.extend_from_slice(&[kind, 0x00, 0x01]);
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(&data);
        }
        out
    };
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
        "/Type /XObject /Subtype /Image /Width 20000 /Height 20000 /ColorSpace /DeviceGray \
         /BitsPerComponent 1 /Filter /JBIG2Decode",
        bytes.len() as u64,
        std::io::Cursor::new(bytes),
    )?;
    out.finish()
}

/// Pages of the `damaged` and `objstm` inputs. Page `n` shows `page_token(n)`.
const PACKED_PAGES: usize = 10_000;

fn page_token(n: usize) -> String {
    format!("PG{n:06}")
}

/// The read-back of `damaged` and `objstm`: the engine's whole-file open reads
/// the page count and the text of the middle page, and the text an
/// extraction wrote holds every page's token, once, in page order.
fn check_packed_text(path: &Path, extracted: &Path) -> Result<(), String> {
    let doc = pdf_oxide::PdfDocument::open(path).map_err(|e| e.to_string())?;
    let pages = doc.page_count().map_err(|e| e.to_string())?;
    if pages != PACKED_PAGES {
        return Err(format!("{pages} pages, expected {PACKED_PAGES}"));
    }
    let middle = PACKED_PAGES / 2;
    let text = doc.extract_text(middle).map_err(|e| e.to_string())?;
    if !text.contains(&page_token(middle)) {
        return Err(format!("page {middle} reads {text:.80}"));
    }
    let wire = std::fs::read(extracted).map_err(|e| format!("{}: {e}", extracted.display()))?;
    let wire = String::from_utf8_lossy(&wire);
    let mut next = 0usize;
    for (at, _) in wire.match_indices("PG") {
        let Some(digits) = wire.get(at + 2..at + 8) else { break };
        if digits != &page_token(next)[2..] {
            return Err(format!("page token {digits} where page {next} belongs"));
        }
        next += 1;
    }
    if next != PACKED_PAGES {
        return Err(format!("{next} page tokens on the wire, expected {PACKED_PAGES}"));
    }
    Ok(())
}

/// 10,000 pages of about 6 KiB of path content each, about 64 MiB, written
/// as a file whose cross-reference table was lost: the objects, a trailer,
/// and a `startxref` that points into the middle of a content stream.
fn generate_damaged(path: &Path) -> Result<(), String> {
    use std::io::BufWriter;
    let mut w = BufWriter::new(File::create(path).map_err(|e| format!("damaged: {e}"))?);
    let mut put = |s: &str| w.write_all(s.as_bytes()).map_err(|e| format!("damaged: {e}"));
    put("%PDF-1.4\n")?;
    put("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;
    let mut kids = String::new();
    for n in 0..PACKED_PAGES {
        kids.push_str(&format!("{} 0 R ", 4 + 2 * n));
    }
    put(&format!(
        "2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {PACKED_PAGES} >>\nendobj\n"
    ))?;
    put("3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n")?;
    for n in 0..PACKED_PAGES {
        put(&format!(
            "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 842] /Contents {} 0 R \
             /Resources << /Font << /F1 3 0 R >> >> >>\nendobj\n",
            4 + 2 * n,
            5 + 2 * n
        ))?;
        let mut content = format!("BT /F1 10 Tf 10 800 Td ({}) Tj ET\n", page_token(n));
        Xorshift::new(n as u64 * 1_000_003 + 17).fill_paths(&mut content, 6_300);
        put(&format!(
            "{} 0 obj\n<< /Length {} >>\nstream\n{content}\nendstream\nendobj\n",
            5 + 2 * n,
            content.len() + 1
        ))?;
        if n % 2000 == 0 {
            eprintln!("memprobe: damaged: page {n}/{PACKED_PAGES}");
        }
    }
    put(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n12345\n%%EOF\n", 4 + 2 * PACKED_PAGES))?;
    w.flush().map_err(|e| format!("damaged: {e}"))
}

/// 10,000 pages whose dictionaries and resources (6.6 KiB of text each, about
/// 64 MiB in all) sit in object streams of 100 members, behind a
/// cross-reference stream; the content streams are ordinary objects.
fn generate_objstm(path: &Path) -> Result<(), String> {
    use std::io::BufWriter;
    const PER_STREAM: usize = 50;
    const PAD: usize = 6_600;
    enum Entry {
        Free,
        At(u64),
        In(usize, usize),
    }
    let groups = PACKED_PAGES.div_ceil(PER_STREAM);
    let page_id = |n: usize| 4 + 3 * n;
    let resources_id = |n: usize| 5 + 3 * n;
    let content_id = |n: usize| 6 + 3 * n;
    let stream_base = 4 + 3 * PACKED_PAGES;
    let xref_id = stream_base + groups;
    let size = xref_id + 1;
    let mut entries: Vec<Entry> = (0..size).map(|_| Entry::Free).collect();

    let mut w = BufWriter::new(File::create(path).map_err(|e| format!("objstm: {e}"))?);
    let at = std::cell::Cell::new(0u64);
    let put = |w: &mut BufWriter<File>, bytes: &[u8]| -> Result<(), String> {
        at.set(at.get() + bytes.len() as u64);
        w.write_all(bytes).map_err(|e| format!("objstm: {e}"))
    };
    put(&mut w, b"%PDF-1.5\n")?;
    let deflate = |bytes: &[u8]| -> Result<Vec<u8>, String> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(bytes).map_err(|e| format!("objstm: {e}"))?;
        z.finish().map_err(|e| format!("objstm: {e}"))
    };

    for group in 0..groups {
        let mut members: Vec<(usize, String)> = Vec::new();
        if group == 0 {
            members.push((1, "<< /Type /Catalog /Pages 2 0 R >>".to_string()));
            let mut kids = String::new();
            for n in 0..PACKED_PAGES {
                kids.push_str(&format!("{} 0 R ", page_id(n)));
            }
            members.push((
                2,
                format!("<< /Type /Pages /Kids [{kids}] /Count {PACKED_PAGES} >>"),
            ));
            members.push((3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string()));
        }
        for n in group * PER_STREAM..((group + 1) * PER_STREAM).min(PACKED_PAGES) {
            members.push((
                page_id(n),
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 842] /Contents {} 0 R \
                     /Resources {} 0 R >>",
                    content_id(n),
                    resources_id(n)
                ),
            ));
            let mut pad = String::with_capacity(PAD);
            Xorshift::new(n as u64 * 1_000_003 + 31).fill_hex(&mut pad, PAD);
            members.push((resources_id(n), format!("<< /Font << /F1 3 0 R >> /Pad ({pad}) >>")));
            let content = format!("BT /F1 10 Tf 10 800 Td ({}) Tj ET\n", page_token(n));
            entries[content_id(n)] = Entry::At(at.get());
            put(
                &mut w,
                format!(
                    "{} 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n",
                    content_id(n),
                    content.len()
                )
                .as_bytes(),
            )?;
        }
        let stream_id = stream_base + group;
        let (mut table, mut body) = (String::new(), String::new());
        for (index, (id, text)) in members.iter().enumerate() {
            table.push_str(&format!("{id} {} ", body.len()));
            body.push_str(text);
            body.push('\n');
            entries[*id] = Entry::In(stream_id, index);
        }
        let packed = deflate([table.as_bytes(), body.as_bytes()].concat().as_slice())?;
        entries[stream_id] = Entry::At(at.get());
        put(
            &mut w,
            format!(
                "{stream_id} 0 obj\n<< /Type /ObjStm /N {} /First {} /Filter /FlateDecode /Length {} >>\nstream\n",
                members.len(),
                table.len(),
                packed.len()
            )
            .as_bytes(),
        )?;
        put(&mut w, &packed)?;
        put(&mut w, b"\nendstream\nendobj\n")?;
        if group % 40 == 0 {
            eprintln!("memprobe: objstm: stream {group}/{groups}");
        }
    }

    let xref_at = at.get();
    entries[xref_id] = Entry::At(xref_at);
    let mut rows = Vec::with_capacity(size * 7);
    for entry in &entries {
        match entry {
            Entry::Free => rows.extend_from_slice(&[0, 0, 0, 0, 0, 0xFF, 0xFF]),
            Entry::At(offset) => {
                rows.push(1);
                rows.extend_from_slice(&(*offset as u32).to_be_bytes());
                rows.extend_from_slice(&[0, 0]);
            },
            Entry::In(stream, index) => {
                rows.push(2);
                rows.extend_from_slice(&(*stream as u32).to_be_bytes());
                rows.extend_from_slice(&(*index as u16).to_be_bytes());
            },
        }
    }
    let packed = deflate(&rows)?;
    put(
        &mut w,
        format!(
            "{xref_id} 0 obj\n<< /Type /XRef /Size {size} /W [1 4 2] /Root 1 0 R /Filter /FlateDecode /Length {} >>\nstream\n",
            packed.len()
        )
        .as_bytes(),
    )?;
    put(&mut w, &packed)?;
    put(&mut w, format!("\nendstream\nendobj\nstartxref\n{xref_at}\n%%EOF\n").as_bytes())?;
    w.flush().map_err(|e| format!("objstm: {e}"))
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
            text.push_str("q 40 0 0 40 500 20 cm /Im1 Do Q\n");
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
         /Resources << /Font << /F1 5 0 R /F2 6 0 R >> /XObject << /Im1 7 0 R >> >> \
         /Contents 4 0 R >>",
    )?;
    out.stream(4, filter.entry(), len, File::open(&body_path).map_err(io)?)?;
    out.object(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
    out.object(6, "<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>")?;
    out.stream(
        7,
        "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray \
         /BitsPerComponent 8",
        1,
        &[128u8][..],
    )?;
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

/// A one-page PDF with 10,000 text widgets in a grid, each with an appearance
/// stream that fills its 5 by 6 point rectangle.
fn generate_widgets(path: &Path) -> Result<(), String> {
    const WIDGETS: usize = 10_000;
    const FIRST: usize = 5;
    let mut out = PdfOut::create(path)?;
    let refs: String = (0..WIDGETS).map(|i| format!("{} 0 R ", FIRST + 2 * i)).collect();
    out.object(
        1,
        &format!("<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [{refs}] >> >>"),
    )?;
    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
    out.object(
        3,
        &format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Annots [{refs}] >>"
        ),
    )?;
    out.stream(4, "", 4, &b"q Q\n"[..])?;
    let appearance = b"0 g 0 0 5 6 re f\n";
    for i in 0..WIDGETS {
        let (x, y) = ((i % 100) * 6, (i / 100) * 7);
        out.object(
            FIRST + 2 * i,
            &format!(
                "<< /Type /Annot /Subtype /Widget /FT /Tx /T (w{i}) /V (v) \
                 /Rect [{x} {y} {} {}] /F 4 /P 3 0 R /AP << /N {} 0 R >> >>",
                x + 5,
                y + 6,
                FIRST + 2 * i + 1
            ),
        )?;
        out.stream(
            FIRST + 2 * i + 1,
            "/Type /XObject /Subtype /Form /BBox [0 0 5 6]",
            appearance.len() as u64,
            &appearance[..],
        )?;
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
