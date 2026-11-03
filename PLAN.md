You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

## Setup

```bash
git submodule update --init vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
```

The engine is the fork in `vendor/pdf_oxide` (Rust). It builds only beside the other two submodules (path dependencies), so always work from this repository. Test command, from the repository root:

```bash
cargo test --manifest-path vendor/pdf_oxide/Cargo.toml \
  --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts" \
  --lib <filter>
```

The first build takes several minutes. Run it in the foreground and read the summary line.

## The fork's rules

- A change to upstream code (any file outside `vendor/pdf_oxide/src/host/`) is wrapped in patch markers, opening and closing on their own lines, never nested, every opener closed:
  `// ── pdf_manipulator patch: <what and why, one line> ──` … `// ── end pdf_manipulator patch ──`
- Tests of ours never go in upstream files or in `vendor/pdf_oxide/tests/`. They go in a new file under `vendor/pdf_oxide/src/host/engine_proofs/`, registered in its `mod.rs` like the siblings (read two, for example `stencil_proof.rs` and `band_proof.rs`, and follow how they build a small PDF in memory and render or read it). Every `#[test]` there has a `///` doc comment saying what it proves and why no Dart-level test can observe it.
- Show each new test can fail: break the fix on purpose, run, see the failure, restore, and `git diff` to confirm nothing injected is left.
- No edit to an existing test to make it pass. If an existing test pins the old behaviour, stop and write that in REPORT.md instead of changing it.


## The memory probe

`tool/memprobe` (a Rust binary in this repository; read `tool/memprobe/src/main.rs`, its `limit()` table and rows, and how the Makefile's `test-memory` target runs it) measures each operation's peak against a 16 MiB ceiling. An earlier run wrote two generators and rows for it. Apply them first, from the appendix at the end of this file: save the lines between the `<<<MEMPROBE-DIFF` and `>>>MEMPROBE-DIFF` markers to `/tmp/memprobe.diff` and run `git apply /tmp/memprobe.diff` at the repository root. Both rows (`extractAll fonts`, `extractAll tagged`) fail today: ~450 MiB and ~1.75 GiB. This task makes its own row pass (named below); report the other row's number as you find it, do not work on it.

Build and run memprobe in release as the Makefile does (if the target needs Dart, run the binary directly with `cargo run --release --manifest-path tool/memprobe/Cargo.toml` and read its own argument handling). Paste the rows it prints.

## Rulings already made (the plan's authority, so these are not "editing a test to make it pass")

- The test `parse_budget_bounded_caps_element_count` (`src/structure/parser.rs`, near line 880) pins the cap this task removes (`MAX_STRUCT_ELEMENTS`). Replace it with a test that parses a structure tree far larger than the old cap in bounded memory (allocator counter, as below).
- XFA's whole-string fields (`template_xml`, `datasets_xml` in `XfaForm`) are replaced by readers or events: nothing outside `src/xfa/` and the host's `dispatch::doc_xfa` / `dispatch::convert_xfa_to_acroform` reads them (check with grep and say so in REPORT.md; if something else does, port it).

## The task: the structure tree, tagged extractAll and XFA (t-511, part B)

1. **The structure tree streams.** Remove `MAX_STRUCT_ELEMENTS` (`src/structure/parser.rs` near line 99). Walk the tree without holding it whole: visit elements depth-first through an explicit stack of object references, resolve each element when visited and drop it after, keep a visited set of object ids (u32 pairs, small) so cycles end. Whatever the consumers need (reading order, the marked-content-id to element mapping that extraction uses) is produced as the walk goes, bounded by the page being extracted, not by the whole document.
2. **extractAll on a tagged page** (600,000 elements): the `extractAll tagged` row must pass under 16 MiB. Measure first and write in REPORT.md what held the ~1.75 GiB (file and line).
3. **XFA as events.** `src/xfa/extractor.rs` reads each packet whole (four `read_whole_raw` callers) and `XfaParser::parse` keeps whole strings. Read packets through the stream decoders' readers and parse them with `quick_xml` events, as `src/extractors/xmp.rs` now does (read it: it is the pattern). `analyze_xfa_document` and `convert_xfa_document` keep their results. Remove the XFA readers from `src/host/engine_proofs/whole_reads.txt` as that file's rule says.

Proofs in new files under `src/host/engine_proofs/` (`mem_structure.rs`, `mem_xfa.rs`): a structure tree of 200,000 elements parsed with growth far below its size; a cyclic tree that ends; a 64 MiB XFA datasets packet analysed with growth under 1 MiB. Use the allocator counter the sibling proofs use, single-threaded where the counter is process-wide (see `filter_readers.rs`). Each shown to fail.

Gate: the test command with filters `engine_proofs`, `structure`, `xfa`, `tagged`, `extract` (paste every summary line); the memprobe row `extractAll tagged` (must pass) with its number.

## Done

- Your tests pass, and the wider filters named above pass (paste every summary line).
- Write `REPORT.md` at the repository root: the cause you found (file and line), what you changed, each test, each injected failure and its failing message, the summary lines, every choice you made.
- Inside `vendor/pdf_oxide`: `git add -N` your new files, then `git diff > ../../cloud.patch`. Do not commit inside the submodule.
- In the outer repository also run `git diff -- tool/memprobe > memprobe.patch`; then commit `cloud.patch`, `memprobe.patch` and `REPORT.md` only (not the submodule pointer), and push this branch. No push to any other branch or repository.

## Appendix: the memprobe diff

<<<MEMPROBE-DIFF
diff --git a/tool/memprobe/src/main.rs b/tool/memprobe/src/main.rs
index 59bda6b..a62181d 100644
--- a/tool/memprobe/src/main.rs
+++ b/tool/memprobe/src/main.rs
@@ -42,6 +42,11 @@ fn limit(op: &str, input: &str) -> usize {
         ("open", "thousand_page") => 16 * MIB,
         // The text is read as operators, one token at a time.
         ("extractAll", "content-fl" | "content-a85" | "content-lzw" | "forms") => 16 * MIB,
+        // 20,000 fonts, each with a ToUnicode CMap of its own, each shown once:
+        // the fonts are read as the page names them and none is held after.
+        ("extractAll", "fonts") => 16 * MIB,
+        // 600,000 structure elements, each marking one run of text.
+        ("extractAll", "tagged") => 16 * MIB,
         // Rulings are quantised as they are painted and dropped; the grid they
         // make is bounded by the page, never by the paths.
         ("extractAll", "paths") => 16 * MIB,
@@ -162,6 +167,10 @@ fn run() -> Result<(), String> {
     generate_paths(&paths)?;
     let widgets = dir.join("widgets.pdf");
     generate_widgets(&widgets)?;
+    let fonts = dir.join("fonts.pdf");
+    generate_fonts(&fonts)?;
+    let tagged = dir.join("tagged.pdf");
+    generate_tagged(&tagged)?;
     eprintln!(
         "memprobe: inputs ready: many={} huge={} encrypted={} forms={} bytes",
         std::fs::metadata(&many).map(|m| m.len()).unwrap_or(0),
@@ -444,7 +453,12 @@ fn run() -> Result<(), String> {
     let extract_inputs: Vec<(&str, &Path)> = contents
         .iter()
         .map(|(name, path)| (*name, path.as_path()))
-        .chain([("forms", forms.as_path()), ("paths", paths.as_path())])
+        .chain([
+            ("forms", forms.as_path()),
+            ("paths", paths.as_path()),
+            ("fonts", fonts.as_path()),
+            ("tagged", tagged.as_path()),
+        ])
         .collect();
     // redact the left part of the first text line of each content source,
     // apply, save, then read the saved file back: the covered start of the
@@ -2874,6 +2888,133 @@ fn generate_forms(path: &Path) -> Result<(), String> {
     out.finish()
 }
 
+/// A one-page PDF of 20,000 simple fonts, each with a ToUnicode CMap of its
+/// own (64 mappings that differ per font), each shown once: about 64 MiB of
+/// font dictionaries and CMaps.
+fn generate_fonts(path: &Path) -> Result<(), String> {
+    const FONTS: usize = 20_000;
+    const FIRST: usize = 5;
+    let mut out = PdfOut::create(path)?;
+    out.object(1, "<< /Type /Catalog /Pages 2 0 R >>")?;
+    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
+    let names: String = (0..FONTS).map(|i| format!("/F{i} {} 0 R ", FIRST + 2 * i)).collect();
+    out.object(
+        3,
+        &format!(
+            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
+             /Resources << /Font << {names}>> >> /Contents 4 0 R >>"
+        ),
+    )?;
+    let page: String =
+        (0..FONTS).map(|i| format!("BT /F{i} 10 Tf 10 {} Td (AB) Tj ET\n", 780 - i % 700)).collect();
+    out.stream(4, "", page.len() as u64, page.as_bytes())?;
+    let padding = "x".repeat(2000);
+    for i in 0..FONTS {
+        let font = FIRST + 2 * i;
+        out.object(
+            font,
+            &format!(
+                "<< /Type /Font /Subtype /Type1 /BaseFont /Probe{i} /FirstChar 65 /LastChar 66 \
+                 /Widths [600 600] /ToUnicode {} 0 R /Note ({padding}) >>",
+                font + 1
+            ),
+        )?;
+        let mut cmap = String::from(
+            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
+             1 begincodespacerange\n<00> <FF>\nendcodespacerange\n64 beginbfchar\n",
+        );
+        for code in 0..64u32 {
+            cmap.push_str(&format!("<{:02X}> <{:04X}>\n", code + 0x20, 0x4E00 + ((i as u32 * 64 + code) % 20_000)));
+        }
+        cmap.push_str("endbfchar\nendcmap end end\n");
+        out.stream(font + 1, "", cmap.len() as u64, cmap.as_bytes())?;
+        if i % 5000 == 0 {
+            eprintln!("memprobe: fonts: font {i}/{FONTS}");
+        }
+    }
+    out.finish()
+}
+
+/// A tagged one-page PDF: 600,000 paragraph structure elements under 6,000
+/// sections of 100, each marking one run of text, and an article thread of
+/// 5,000 beads over the page: about 64 MiB.
+fn generate_tagged(path: &Path) -> Result<(), String> {
+    const ELEMENTS: usize = 600_000;
+    const PER_SECTION: usize = 100;
+    const SECTIONS: usize = ELEMENTS / PER_SECTION;
+    const BEADS: usize = 5_000;
+    // 1 catalog, 2 pages, 3 page, 4 contents, 5 font, 6 struct root,
+    // 7 document, then the sections, the paragraphs, the thread and its beads.
+    const DOCUMENT: usize = 7;
+    const FIRST_SECTION: usize = 8;
+    const FIRST_PARAGRAPH: usize = FIRST_SECTION + SECTIONS;
+    const THREAD: usize = FIRST_PARAGRAPH + ELEMENTS;
+    const FIRST_BEAD: usize = THREAD + 1;
+    let mut out = PdfOut::create(path)?;
+    out.object(
+        1,
+        &format!(
+            "<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 6 0 R \
+             /Threads [{THREAD} 0 R] >>"
+        ),
+    )?;
+    out.object(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")?;
+    out.object(
+        3,
+        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /StructParents 0 \
+         /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
+    )?;
+    let mut content = String::new();
+    for mcid in 0..ELEMENTS {
+        content.push_str(&format!(
+            "/P <</MCID {mcid}>> BDC BT /F1 6 Tf 10 {} Td (t{mcid}) Tj ET EMC\n",
+            780 - mcid % 760
+        ));
+    }
+    out.stream(4, "", content.len() as u64, content.as_bytes())?;
+    drop(content);
+    out.object(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")?;
+    out.object(6, &format!("<< /Type /StructTreeRoot /K {DOCUMENT} 0 R >>"))?;
+    let sections: String = (0..SECTIONS).map(|s| format!("{} 0 R ", FIRST_SECTION + s)).collect();
+    out.object(
+        DOCUMENT,
+        &format!("<< /Type /StructElem /S /Document /P 6 0 R /K [{sections}] >>"),
+    )?;
+    for s in 0..SECTIONS {
+        let kids: String = (0..PER_SECTION)
+            .map(|k| format!("{} 0 R ", FIRST_PARAGRAPH + s * PER_SECTION + k))
+            .collect();
+        out.object(
+            FIRST_SECTION + s,
+            &format!("<< /Type /StructElem /S /Sect /P {DOCUMENT} 0 R /K [{kids}] >>"),
+        )?;
+    }
+    for e in 0..ELEMENTS {
+        out.object(
+            FIRST_PARAGRAPH + e,
+            &format!(
+                "<< /Type /StructElem /S /P /P {} 0 R /Pg 3 0 R /K {e} >>",
+                FIRST_SECTION + e / PER_SECTION
+            ),
+        )?;
+        if e % 100_000 == 0 {
+            eprintln!("memprobe: tagged: element {e}/{ELEMENTS}");
+        }
+    }
+    out.object(THREAD, &format!("<< /Type /Thread /F {FIRST_BEAD} 0 R >>"))?;
+    for b in 0..BEADS {
+        let next = FIRST_BEAD + (b + 1) % BEADS;
+        let prev = FIRST_BEAD + (b + BEADS - 1) % BEADS;
+        out.object(
+            FIRST_BEAD + b,
+            &format!(
+                "<< /Type /Bead /T {THREAD} 0 R /N {next} 0 R /V {prev} 0 R /P 3 0 R /R [0 0 100 100] >>"
+            ),
+        )?;
+    }
+    out.finish()
+}
+
 /// A one-page PDF with 10,000 text widgets in a grid, each with an appearance
 /// stream that fills its 5 by 6 point rectangle.
 fn generate_widgets(path: &Path) -> Result<(), String> {
>>>MEMPROBE-DIFF
