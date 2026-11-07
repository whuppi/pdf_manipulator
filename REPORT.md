# t-575 report: the whole-buffer lint

Status: **not green.** `bash tool/check_whole_buffers.sh` exits 1 on 13 sites: the one the plan leaves
to t-582 (rule 3b) and 12 that rules 3 and 5 call unreached but whose proof the script cannot make.
I stopped on those 12 and did not allow them under any class (see Findings). Everything else in the
plan is done.

## What `--sites` listed before I started

`bash tool/check_whole_buffers.sh --sites` listed 73 sites (and 21 reads it does not count as sites: `take`
windows and whole-read helper bodies). 28 were already `table:` lines, 45 were red. The table below has all 73
with what each is now. "table, already allowed" lines are untouched.

| site | call | class | proof or kind |
|---|---|---|---|
| `src/api/pdf_builder.rs:1775` `to_bytes#1` | `fs::read(` | **not allowed (finding)** | script proof: none holds |
| `src/bin/analyze_pdf_features.rs:127` `analyze_raw_pdf_structure#1` | `fs::read(` | unreached | path: analyze_raw_pdf_structure takes a Path |
| `src/converters/office/docx.rs:57` `parse_document#1` | `read_to_string(` | **not allowed (finding)** | script proof: none holds |
| `src/converters/office/docx.rs:276` `parse_metadata#1` | `read_to_string(` | **not allowed (finding)** | script proof: none holds |
| `src/converters/office/mod.rs:145` `convert_docx#1` | `std::fs::read(` | unreached | path: convert_docx takes a Path |
| `src/converters/office/mod.rs:195` `convert_xlsx#1` | `std::fs::read(` | unreached | path: convert_xlsx takes a Path |
| `src/converters/office/mod.rs:233` `convert_pptx#1` | `std::fs::read(` | unreached | path: convert_pptx takes a Path |
| `src/converters/office/mod.rs:285` `convert#1` | `std::fs::read(` | unreached | path: convert takes a Path |
| `src/converters/office/pptx.rs:82` `parse_slide#1` | `read_to_string(` | **not allowed (finding)** | script proof: none holds |
| `src/converters/office/pptx.rs:200` `parse_metadata#1` | `read_to_string(` | **not allowed (finding)** | script proof: none holds |
| `src/decoders/ascii85.rs:20` `decode#1` | `decode_slice(` | **not allowed (finding)** | script proof: none holds |
| `src/decoders/ascii_hex.rs:19` `decode#1` | `decode_slice(` | **not allowed (finding)** | script proof: none holds |
| `src/decoders/brotli.rs:15` `decode#1` | `decode_slice(` | **not allowed (finding)** | script proof: none holds |
| `src/decoders/flate.rs:120` `decode#1` | `decode_slice(` | **not allowed (finding)** | script proof: none holds |
| `src/decoders/lzw.rs:58` `decode_lzw#1` | `decode_slice(` | **not allowed (finding)** | script proof: none holds |
| `src/decoders/mod.rs:187` `decode_stream_with_params#1` | `decode_slice(` | **not allowed (finding)** | script proof: none holds |
| `src/decoders/runlength.rs:21` `decode#1` | `decode_slice(` | **not allowed (finding)** | script proof: none holds |
| `src/document.rs:1204` `open#1` | `std::fs::read(` | unreached | path: open takes a Path |
| `src/document.rs:4884` `compute_output_intent_cmyk_profile#1` | `decode_stream_data(` | table, already allowed | the ICC profile of an output intent |
| `src/editor/document_editor.rs:2135` `merge_from#1` | `std::fs::read(` | unreached | path: merge_from takes a Path |
| `src/extractors/auto.rs:1083` `prefetch_models#1` | `read_to_string(` | unreached | gate: inside #[cfg(feature = "ocr")] in src/extractors/auto.rs |
| `src/extractors/auto.rs:1161` `http_fetch#1` | `read_to_end(` | unreached | gate: inside #[cfg(feature = "ocr")] in src/extractors/auto.rs |
| `src/extractors/images.rs:1291` `extract_image_from_xobject_at#1` | `decode_stream_data(` | **not allowed (finding)** | script proof: none holds |
| `src/extractors/images.rs:1741` `resolve_icc_profile_from_obj#1` | `decode_stream_data(` | table, already allowed | an ICC profile |
| `src/extractors/images.rs:2625` `decode_jbig2_bits#1` | `decode_stream_data(` | table, new | a JBIG2 stream and globals (the embedded stream), which hayro-jbig2 takes as &[u8] |
| `src/extractors/images.rs:2631` `decode_jbig2_bits#2` | `decode_stream_data(` | table, new | a JBIG2 stream and globals (the globals), which hayro-jbig2 takes as &[u8] |
| `src/extractors/images.rs:2815` `decode_jpx_image#1` | `decode_stream_data(` | table, new | a JPX codestream, which OpenJPEG takes as &[u8] |
| `src/ffi.rs:3613` `pdf_document_sign#1` | `source_bytes_whole(` | unreached | gate: module ffi is declared under #[cfg(all(not(target_arch = "wasm32"), feature = "public-api"))] in src/lib.rs |
| `src/ffi.rs:4554` `pdf_document_has_timestamp#1` | `source_bytes_whole(` | unreached | gate: module ffi is declared under #[cfg(all(not(target_arch = "wasm32"), feature = "public-api"))] in src/lib.rs |
| `src/ffi.rs:7646` `pdf_document_get_source_bytes#1` | `source_bytes_whole(` | unreached | gate: module ffi is declared under #[cfg(all(not(target_arch = "wasm32"), feature = "public-api"))] in src/lib.rs |
| `src/fonts/font_dict.rs:739` `from_dict#1` | `decode_stream_data(` | table, already allowed | a font program (FontFile2) |
| `src/fonts/font_dict.rs:782` `from_dict#2` | `decode_stream_data(` | table, already allowed | a font program (FontFile3) |
| `src/fonts/font_dict.rs:833` `from_dict#3` | `decode_stream_data(` | table, already allowed | a font program (FontFile) |
| `src/fonts/font_dict.rs:1060` `from_dict#4` | `decode_stream_data(` | table, already allowed | the CMap table (ToUnicode) |
| `src/fonts/font_dict.rs:1996` `extract_truetype_cmap_from_descriptor#1` | `decode_stream_data(` | table, already allowed | a font program, read for its cmap table |
| `src/fonts/font_dict.rs:2108` `extract_embedded_font_from_descriptor#1` | `decode_stream_data(` | table, already allowed | a font program |
| `src/fonts/font_dict.rs:2361` `resolve_encoding_writing_mode#1` | `decode_stream_data(` | table, already allowed | the CMap table of an embedded encoding |
| `src/fonts/unicode_fallback.rs:72` `load_unicode_fallback_bytes#1` | `std::fs::read(` | table, already allowed | a font program, a system fallback font |
| `src/fonts/unicode_fallback.rs:115` `load_cjk_fallback_bytes#1` | `std::fs::read(` | table, already allowed | a font program, a system fallback font |
| `src/host/font_subset/mod.rs:552` `subset#1` | `read_to_end(` | table, already allowed | a font program, read to cut it down to the glyphs shown |
| `src/host/ops/fonts.rs:28` `handle_register_fallback_font#1` | `read_all_from_reader(` | table, already allowed | a font program, the caller's fallback font |
| `src/host/redact_scrub.rs:421` `rewrite_font#1` | `read_to_end(` | table, already allowed | the CMap table (ToUnicode) |
| `src/ocr/detector.rs:40` `new#1` | `std::fs::read(` | unreached | gate: module ocr is declared under #[cfg(any(feature = "ocr", feature = "ocr-tract"))] in src/lib.rs |
| `src/ocr/recognizer.rs:60` `new#1` | `std::fs::read(` | unreached | gate: module ocr is declared under #[cfg(any(feature = "ocr", feature = "ocr-tract"))] in src/lib.rs |
| `src/ocr/recognizer.rs:63` `new#2` | `read_to_string(` | unreached | gate: module ocr is declared under #[cfg(any(feature = "ocr", feature = "ocr-tract"))] in src/lib.rs |
| `src/python.rs:2624` `convert_to_pdf_a#1` | `source_bytes_whole(` | unreached | gate: module python is declared under #[cfg(feature = "python")] in src/lib.rs |
| `src/rendering/mesh_shading.rs:92` `render_mesh_shading#1` | `decode_stream_data(` | table, already allowed | a sampled function (type 0) |
| `src/rendering/mesh_shading.rs:1033` `eval_pdf_function#1` | `decode_stream_data(` | table, already allowed | a sampled function (type 0) |
| `src/rendering/mesh_shading.rs:1117` `eval_type4#1` | `decode_stream_data(` | table, new | a PostScript calculator function (type 4), a program evaluated per sample in O(its own length) |
| `src/rendering/page_renderer.rs:10156` `parse_type0_transfer_function#1` | `decode_stream_data(` | table, already allowed | a sampled function (type 0) |
| `src/rendering/page_renderer.rs:10214` `parse_type4_transfer_function#1` | `decode_stream_data(` | table, new | a PostScript calculator function (type 4), a program evaluated per sample in O(its own length) |
| `src/rendering/page_renderer.rs:10389` `evaluate_bc_tint_function#1` | `decode_stream_data(` | table, new | a PostScript calculator function (type 4), a program evaluated per sample in O(its own length) |
| `src/rendering/page_renderer.rs:10677` `evaluate_type0_multi#1` | `decode_stream_data(` | table, already allowed | a sampled function (type 0) |
| `src/rendering/page_renderer.rs:10987` `project_iccbased_to_rgb#1` | `decode_stream_data(` | table, already allowed | an ICC profile |
| `src/rendering/resolution/color.rs:229` `resolve_iccbased#1` | `decode_stream_data(` | table, already allowed | an ICC profile (CMYK) |
| `src/rendering/resolution/color.rs:293` `resolve_iccbased#2` | `decode_stream_data(` | table, already allowed | an ICC profile (RGB) |
| `src/rendering/resolution/color.rs:346` `resolve_iccbased#3` | `decode_stream_data(` | table, already allowed | an ICC profile (gray) |
| `src/rendering/resolution/color.rs:985` `evaluate_type4#1` | `decode_stream_data(` | table, new | a PostScript calculator function (type 4), a program evaluated per sample in O(its own length) |
| `src/rendering/resolution/color.rs:1155` `evaluate_type0_sampled#1` | `decode_stream_data(` | table, already allowed | a sampled function (type 0) |
| `src/rendering/resolution/pipeline.rs:273` `eval_separation_alt_cmyk#1` | `decode_stream_data(` | table, new | a PostScript calculator function (type 4), a program evaluated per sample in O(its own length) |
| `src/rendering/separation_renderer.rs:304` `render_plates_for_inks#1` | `get_page_content_data(` | unreached | callers: render_plates_for_inks and its callers appear in no file under src/host or lib/ |
| `src/rendering/separation_renderer.rs:476` `collect_referenced_inks#1` | `get_page_content_data(` | unreached | callers: collect_referenced_inks and its callers appear in no file under src/host or lib/ |
| `src/rendering/separation_renderer.rs:533` `scan_operators_for_inks#1` | `decode_stream_data(` | unreached | callers: scan_operators_for_inks and its callers appear in no file under src/host or lib/ |
| `src/rendering/separation_renderer.rs:2325` `execute_separation_operators#1` | `decode_stream_data(` | unreached | callers: execute_separation_operators and its callers appear in no file under src/host or lib/ |
| `src/rendering/separation_renderer.rs:3179` `paint_image_mask_to_plates#1` | `decode_stream_data(` | unreached | callers: paint_image_mask_to_plates and its callers appear in no file under src/host or lib/ |
| `src/rendering/sidecar.rs:1417` `try_retarget_cmyk_via_embedded_profile#1` | `decode_stream_data(` | table, already allowed | an ICC profile |
| `src/signatures/pades/dss_read.rs:44` `stream_array#1` | `decode_stream_data(` | table, already allowed | the DSS (its certificate, OCSP and CRL streams) |
| `src/split_bookmarks.rs:321` `split_by_bookmarks_to_dir#1` | `std::fs::read(` | unreached | path: split_by_bookmarks_to_dir takes a Path |
| `src/wasm.rs:2574` `export_form_data#1` | `std::fs::read(` | unreached | gate: module wasm is declared under #[cfg(all(feature = "wasm", feature = "public-api"))] in src/lib.rs |
| `src/writer/font_discovery.rs:224` `resolve#1` | `std::fs::read(` | table, already allowed | a font program, a system font file |
| `src/writer/font_manager.rs:932` `from_file#1` | `std::fs::read(` | table, already allowed | a font program, a font file |
| `src/writer/image_handler.rs:199` `from_file#1` | `std::fs::read(` | unreached | path: from_file takes a Path |
| `src/xref.rs:874` `parse_xref_stream#1` | `read_whole_raw(` | table, already allowed | the xref stream |

## What changed

- `tool/check_whole_buffers.sh`: the three kinds (`PostScript calculator function`, `JBIG2 stream and globals`,
  `JPX codestream`) and the `unreached:` class with the three proofs (gate, path, callers). `--sites` prints the
  proof that holds beside each site. Any class but `table:` and `unreached:` (so `bounded:` too) is refused.
- `tool/whole_buffer_allow.txt`: 8 new `table:` lines and 24 `unreached:` lines, each with its why.
- `vendor/pdf_oxide/src/host/engine_proofs/whole_reads.rs` and `whole_reads.txt` deleted, with `mod whole_reads;`
  and the two comments (`mem_xmp.rs`, `mem_xfa.rs`) that named the wall; they name the lint now.
- `docs/ARCHITECTURE.md`: "Memory" describes the one lint (site, kinds, `take` window, `unreached:` proofs) and no
  longer names `engine_proofs/whole_reads.txt`; "The promise" gained the Office sentence; the convertFromOffice row
  says the package stays in its source and office_oxide holds the whole document model; the tool tree lists the
  script and drops `whole_reads.txt`. `README.md` gained the Office sentence beside **Memory**.
- `tool/memprobe/src/main.rs`: rows `convertToPdf docx64` (530 MiB) and `convertToPdf pptx64` (387 MiB), with the
  sentence as their comment. Inputs are a stored-ZIP package holding one 64 MiB JPEG (4096 x 3620 noise, the
  `jpeg64` generator); the docx also holds 20,000 paragraphs. Measured peaks over baseline: docx 554,721,799 bytes
  (529.02 MiB), pptx 404,794,459 bytes (386.04 MiB), identical on two runs; rounded up to the next MiB. Each output
  is read back (`%PDF-` header, `%%EOF` end).
- `Makefile`: `./tool/check_whole_buffers.sh` after `check_cancel_polls.sh` in `test-guards`, and a bullet in the guard
  comment list.

## Findings (stopped on; not in the allow list)

The lint is red on exactly these 13 sites:

```
src/api/pdf_builder.rs:1775: reads a whole stream, file or page into one buffer: fs::read(
src/converters/office/docx.rs:57: reads a whole stream, file or page into one buffer: read_to_string(
src/converters/office/docx.rs:276: reads a whole stream, file or page into one buffer: read_to_string(
src/converters/office/pptx.rs:82: reads a whole stream, file or page into one buffer: read_to_string(
src/converters/office/pptx.rs:200: reads a whole stream, file or page into one buffer: read_to_string(
src/decoders/ascii85.rs:20: reads a whole stream, file or page into one buffer: decode_slice(
src/decoders/ascii_hex.rs:19: reads a whole stream, file or page into one buffer: decode_slice(
src/decoders/brotli.rs:15: reads a whole stream, file or page into one buffer: decode_slice(
src/decoders/flate.rs:120: reads a whole stream, file or page into one buffer: decode_slice(
src/decoders/lzw.rs:58: reads a whole stream, file or page into one buffer: decode_slice(
src/decoders/mod.rs:187: reads a whole stream, file or page into one buffer: decode_slice(
src/decoders/runlength.rs:21: reads a whole stream, file or page into one buffer: decode_slice(
src/extractors/images.rs:1291: reads a whole stream, file or page into one buffer: decode_stream_data(
13 site(s) read a whole buffer that is not a table and that no proof shows unreached
```

- `src/extractors/images.rs:1291` `extract_image_from_xobject_at#1`: the t-582 site (rule 3b). Left out as told; the
  assistant adds its line when t-582 lands.
- `src/api/pdf_builder.rs:1775` `to_bytes#1` (the builder API, `Pdf::to_bytes`): no gate is off (the module is
  not cfg-gated), `to_bytes` takes no `Path`, and the name appears in `src/host/dispatch.rs:729`. That one is
  `FdfWriter::to_bytes`, a different method, so the claim is probably true, but the proof is by name and does not
  hold.
- `src/converters/office/docx.rs:57,276` and `pptx.rs:82,200` (`parse_document`, `parse_slide`, `parse_metadata`):
  office is on in both feature strings and these take an archive, not a path. The names are clean, but each is
  called by `convert` (`docx.rs:37,40`, `pptx.rs:38`), and the word `convert` appears in `src/host` and `lib/`
  (the `convert` op module: `host/ops/registry.rs:9,24-26`, `host/ops/mod.rs:24`; `std::convert`; Dart
  `.convert(` calls in `lib/`; a comment at `host/dispatch.rs:1788`). None of those is this method, so the claim is
  probably true, but the proof is by name and fails on the caller.
- `src/decoders/{ascii85,ascii_hex,brotli,flate,runlength}.rs` `decode#1`, `lzw.rs` `decode_lzw#1` (called by
  `decode`), `mod.rs` `decode_stream_with_params#1` (rule 5/d-133 calls the `Decoder::decode` callers unreached):
  `decode` appears in `src/host/images/dct.rs:648` (the Huffman decoder's own `decode`) and elsewhere. These
  seven are the t-580 sites. `decode_stream_with_params` is the one I would not call unreached at all: it is
  called by `xref.rs:974` (the xref-stream parser every open of such a file runs) and by `Object::decode_stream_data`
  (`object.rs:513`), the engine's own whole-read helper. The script's message names `host/content/parts.rs:15`
  for `decode_stream_data`, which is only a doc comment; the reachability rests on `xref.rs:974`. So the rule-5
  claim for the `Decoder::decode` callers is not supported by the third proof as written, and for
  `decode_stream_with_params` the code points the other way.

To make the lint green the choice is the maintainer's: give those names a proof the script can run (a type-qualified
caller check, or a gate on the office and builder modules), move the seven decoder callers to bounded reads, or
rename the colliding host names. I changed none of that.

## The third proof is by name and one level

It follows the plan: a name appears in no file of `src/host` (test code and `engine_proofs/` removed, comments
kept: "appears") or `lib/`, and the same for each function that calls it. That is a conservative test for a
short name and a weak one for a deep call graph. `--sites` shows it: `src/xref.rs` `parse_xref_stream` and
the `rendering/` table sites print `[unreached by callers]` although every lane reaches them, because the
host names a top-level function and not these. So the proof says nothing against a `table:` line, and the five
separation renderer lines rest on a one-level name check. I used it only for them (as ruled).

## Injected failures (each restored; `diff` of `tool/whole_buffer_allow.txt` against the copy saved before is empty)

1. `unreached:` on a reachable site (`src/host/font_subset/mod.rs::subset#1`, a host function, in place of its table line):

```
tool/whole_buffer_allow.txt:38: src/host/font_subset/mod.rs::subset#1 is not unreached (src/host/font_subset/mod.rs:552): no cfg gate that is off for the build; subset takes no Path; callers: subset appears at vendor/pdf_oxide/src/host/font_optimizer.rs:65
```

2. A `table:` line with an unknown kind (`src/xref.rs::parse_xref_stream#1  # table: a thing that is big`):

```
tool/whole_buffer_allow.txt:71: the why names none of the kinds that are tables by nature: font program, sampled function, PostScript calculator function, ICC profile, CMap table, DER certificate, DSS, xref stream, object-stream member, progressive JPEG raster, JPX recompressed at full size, JBIG2 stream and globals, JPX codestream
```

3. A stale line (`src/xref.rs::gone_fn#1  # table: the xref stream`):

```
tool/whole_buffer_allow.txt:72: src/xref.rs::gone_fn#1 matches no flagged site
```

4. A `bounded:` class (rule 2):

```
tool/whole_buffer_allow.txt:71: an allowed site is `# table: <kind>` or `# unreached: <why>`; a read of anything else is bounded instead
```

## Summary lines

- `bash tool/check_whole_buffers.sh` (exit 1): `13 site(s) read a whole buffer that is not a table and that no proof shows unreached`.
  With the 12 findings allowed it would be one site, the t-582 one.
- `make test-guards DART=dart`: the Dart guards pass (`+117 ~2: All tests passed!`), then `check_changelog_shape`,
  `check_migration`, `check_migration_snippets`, `check_fork_tests`, `check_patch_markers` and `check_cancel_polls`
  pass, then `check_whole_buffers` fails with the line above (`make: *** [Makefile:186: test-guards] Error 1`), so
  the `dart:io` and later steps did not run.
- `bash tool/check_patch_markers.sh`: exit 0, no output. `bash tool/check_fork_tests.sh`: exit 0, no output.
- Rust suite: `Summary [ 559.697s] 6791 tests run: 6791 passed (8 slow), 5 skipped`.
- memprobe: a full run (before the docx namespace fix below) had 80 steps `ok` and only the two new office rows
  failing, at their placeholder limit 0 (docx failed early: wrong namespace). After the fix the two rows were run
  alone through a temporary early exit, since removed, and print
  `convertToPdf docx64 peak_over_baseline=554721799 limit=555745280 ok` and
  `convertToPdf pptx64 peak_over_baseline=404794459 limit=405798912 ok`. I did not re-run the whole probe or
  `cargo test --release` of memprobe on the final tree.

## Choices where the plan left one open

- `build.json` features: I take the closure of both strings through the `[features]` table of the engine's
  Cargo.toml (plus `default`), which is stricter than "in neither string". `public-api` is in neither string
  and implied by none, so `ffi.rs` and `wasm.rs` pass the gate proof and need no third proof.
- The gate proof reads the `#[cfg]` of the item, and of the `mod x;` line that declares the file's module in its parent,
  up to `lib.rs`. `not(..)` and non-feature gates prove nothing; `any` is off when all are, `all` when one is.
- A `Path` parameter means `Path`, `PathBuf` or `impl AsRef<Path>` in the parameters or generics (not the return type).
- A caller is found by the word in non-test code of any file in `src/` outside `engine_proofs/`; a use outside any
  function fails the proof.
- `writer/font_manager.rs` `from_file#1` stays the `table:` line it already was; only `image_handler.rs` `from_file#1` is new.
- `evaluate_bc_tint_function#1` reads a Type 4 program, so it is a PostScript calculator table with `eval_type4`,
  `parse_type4_transfer_function`, `evaluate_type4` and `eval_separation_alt_cmyk`. `decode_jbig2_bits#1` is the
  JBIG2 stream and `#2` its globals.
- `save_as_jpeg` and `to_base64_data_uri` match none of the patterns, so rule 5 needs no line for them.
- The memprobe office inputs live in `convert_office` (called from `run`), with a hand-written stored ZIP
  (`StoredZip`) because the probe has no zip dependency. The first docx used a wrong WordprocessingML namespace
  and office_oxide refused it; fixed.
- `cargo-nextest` is not in the image and its download host answers 403 through the proxy, so I installed it with
  `cargo install cargo-nextest --locked`.
- Commit message has no co-author or session trailer, as AGENTS.md says ("No AI co-author lines in commits"),
  which overrides the attribution reminder.
