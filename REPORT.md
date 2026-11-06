# t-442 report

Part 1 is done and green except one fuel row that was already red. Part 2's lint is built and wired, and it fails on 59 sites, as the plan says it may: those are listed below and I stopped there.

## Part 1: redaction is cancellable

### Fuel sizes and failures

Sizes did not need to grow. `REDACT_LINES` stays 20,000. The hole was that the `redact text` case never ran the save, and the save is where the page content is rewritten with no poll. I added `save(...)` to `("redact","text")` and a new `("redact","form")` case (a page that draws a form of 20,000 lines of text; the apply's dry run and the save both read the form). Cost of the probe: the whole `make test-fuel` is about 2 m 17 s on 4 cores; `redact text` burns 7.9e9 fuel and `redact form` 1.13e10, both under `MAX_TOTAL` (2e10).

On the code before the fix (only the poll in `transform.rs` removed):

    redact  text   max_gap 4434902633  FAIL: gap above 1100000000
    redact  form   max_gap 7867366379  FAIL: gap above 1100000000

After the fix:

    redact  text   max_gap 2581049   total 7895689975   polls 80190
    redact  form   max_gap 2579034   total 11325393523  polls 120046

`("applyRedactions","regions")` was not grown. Before the fix its gap is 30.7M and it does not move: its pages are small and polled per page and per 256 operators, so no size makes the poll count the wall. Left as is.

### The fix

One poll, `crate::cancel::poll()?`, at the top of the operator loop of `RedactPass::process` (`src/redaction/transform.rs`). Every redaction read goes through that loop: the page in the dry run, the page and the appended overlay in the save, every form (the dry run's nested pass, the form copies of the save, the flattened-widget prep). The job's check is the one `bridge_api::handle_request` enters for the whole request, so it is the op's `CancelCheck`. I chose the thread-local rather than threading a `CancelCheck` through `write_redacted_content`, `write_form_copy`, `prepare_flat_redactions` and `redact_form`: the same files already poll that way (`redact_scrub.rs`, the save in `document_editor.rs`), and it is one line instead of six signatures. The existing `with_cancel_every(.., 256)` on the dry run's page stream stays.

### Every `Operators::new(` outside tests and `engine_proofs`

| site | how it is polled |
|---|---|
| `redaction/transform.rs:538` (`run`, an empty stream) | nothing to read |
| `editor/document_editor.rs:779` (`DocForms::content`) | read by `RedactPass::process`, which now polls |
| `editor/document_editor.rs:4167` (appended overlay) | same, `run_then` calls `process` |
| `editor/document_editor.rs:12378` | `#[cfg(test)]` only |
| `document.rs:19089` (`walk_form_xobject_tree_for_inks`) | `Stride::job()` in the loop that reads it |
| `document.rs:20349` `page_operators` and `:20372` `form_operators` (providers) | each caller, below |
| `rendering/page_renderer.rs:11396` (`form_operator_source`) | `execute_operator_stream` polls with `Stride::job()` (comment in the loop: a form, glyph or pattern cell has no check of its own); covered by the `render knockout` and `render type3` rows |
| `extractors/text.rs:69` (`bytes_source`) | public byte-slice extraction, not a host op; its loop is `run_content_stream` |
| `host/resource_prune.rs:838, :855` | `.with_cancel(cancel.clone())` on the same chain at :860 |
| `host/image_mods.rs:162` | `poll_io()` in the loop |
| `host/content/operators.rs:736..878` | tests |

Callers of `page_operators` / `form_operators` that I checked one by one:

- polled already: `document.rs` `gather_page_signals` (:2044), `page_may_contain_text` (:20412), `extract_paths_into_cancellable`, `process_form_xobject_paths`, `page_image_handles`, `extract_images_filtered` and the two form variants (all `Stride`); `compliance/validators.rs:320` (`with_cancel`); `rendering/page_renderer.rs:758` (`with_cancel_every(.., 1)`); `host/redact_scrub.rs` `count_stream` (`Stride::job()`); `editor/document_editor.rs:12351` (`get_page_images`, `Stride::job()`)
- `compliance/converter.rs:846`: only `.is_err()`, no iteration
- `bin/export_to_markdown.rs`: command-line tool, no lane
- not polled, host-reachable, so wired and given a fuel case (the three below)

New holes found by this audit, each with a failing fuel row before the fix and a passing one after:

| case | fix | before | after |
|---|---|---|---|
| `setImageBounds long` (300,000 triples + one image, then save) | `Stride::job().tick()` in `write_image_mods_content` | gap 9732841099 | 47424620 |
| `extractAll form` (a form of 300,000 triples + one text line) | `TextExtractor::set_cancel`; the form's `Operators` get `with_cancel_every(cancel, 256)`; `extract_spans_impl` and the char path pass the job's check | gap 4924994240 | 15428085 |
| `convert long` (docx of a page of 100,000 triples) | `Stride::job()` in the two loops of `converters/form_xobject_finder.rs` (they return `Vec`, so a cancel ends the walk and the caller's next poll ends the job) | gap 1300839857 | 23129024 |

I first wired the text extraction with a thread-local `poll()` in `run_content_stream`. `page_list_edits_cancelled_at_any_poll_change_nothing` then failed (a cancel at poll 226 of 234 did not stop `addStamp`): a non-cancellable caller of `extract_spans` swallows the error. I took the poll out and carried the explicit `CancelCheck` instead, which only the cancellable entries set; the test passes again.

Not fixed, out of scope: with a form of 20,000 lines of text the `extractAll form` gap is 4.96e9, from the reading-order XY-cut (`pipeline::reading_order::xycut`, `postprocess_spans`), a layout pass over spans, not an operator read. The new fixture keeps the form's text to one line so it measures the stream read. That pass needs its own task.

### Tests (written before the fix, seen to fail)

`src/host/engine_proofs/cancel_polls.rs`, two new tests, with the failure on the unfixed code:

    redaction_polls_while_it_reads_the_operators_of_a_page_and_of_a_form
      save, in_form false: 22 polls for 20000 operators, 22 for 10
    redaction_cancelled_at_any_poll_of_its_stream_reads_changes_nothing
      a full run polls at least 3 times, once for each page of each loop: 2

Both pass after the fix. The second first took 255 s at 600 operators (a poll for every operator, cancelled at each); I cut it to 40.

Break-the-fix checks: the redaction poll was removed (file restored from the saved copy) and `redact text`/`redact form` failed as above; with no poll in the form read at all, `extractAll form` failed at 4924994240 (measured with the thread-local variant removed; the explicit variant then measured 15428085); the image-mods and finder polls are the code the failing rows above measured. Each restore was copied back and `git diff` shows only the intended lines.

## Part 2: the whole-buffer lint

`tool/check_whole_buffers.sh`, `tool/whole_buffer_patterns.txt`, `tool/whole_buffer_allow.txt`; wired into `make test-guards` after `check_cancel_polls.sh`; one paragraph in `docs/ARCHITECTURE.md` "Memory".

### Patterns

`read_to_end(`, `read_to_string(`, `std::fs::read(`, `fs::read(`, `read_whole_raw(`, `decode_stream_data(`, `decoded_data(` (no function of that name exists today; kept as asked, it matches nothing), and the whole-read helpers I found: `read_whole(` (`decoders::stream`, `host::image_embed`), `read_whole_loose(`, `read_all_from_reader(` (`host/ops/support.rs`), `source_bytes_whole(`, `get_page_content_data(`, `cached_page_content(`, `raw_compressed_bytes(`.

Two rules the plan did not state, both mine:
- A `read_to_end(`/`read_to_string(` on a chain with `.take(` in its statement is a bounded window, not a site. Three exist: `host/images/execute.rs:147` (`HEAD_BYTES`), `host/images/redact_copy.rs:455` (the JPX main header, a loop of bounded takes), `objstm.rs:316` (the object stream's offset table, `first` checked to at most 10,000,000).
- A call inside a function that is itself one of the patterns is the helper's own body; its callers are the sites. 13 such calls (`object.rs`, `decoders/stream.rs`, `document.rs` `cached_page_content`/`get_page_content_data`/`source_bytes_whole`, `extractors/images.rs` `raw_compressed_bytes`).

`--sites` prints every site and every call it did not count.

### Injected failures

    inject `r.read_to_end(&mut v)` in src/host/forward.rs:
    vendor/pdf_oxide/src/host/forward.rs:91: reads a whole stream, file or page into one buffer: read_to_end(
        read it in bounded pieces, or allow it as src/host/forward.rs::injected_whole_read#1  # table: <kind>
    60 site(s) read a whole buffer that is not a table        (59 without it; file restored)
    stale allow line:  tool/whole_buffer_allow.txt:37: src/nowhere.rs::nothing#1 matches no flagged site
    wrong class:       tool/whole_buffer_allow.txt:37: an allowed site is `# table: <why>`; a read of anything that is not a table is bounded instead

### Sites allowed, with their kind (28)

ICC profile: `document.rs::compute_output_intent_cmyk_profile#1`, `extractors/images.rs::resolve_icc_profile_from_obj#1`, `rendering/page_renderer.rs::project_iccbased_to_rgb#1`, `rendering/resolution/color.rs::resolve_iccbased#1-3`, `rendering/sidecar.rs::try_retarget_cmyk_via_embedded_profile#1`.
Font program: `fonts/font_dict.rs::from_dict#1-3`, `extract_truetype_cmap_from_descriptor#1`, `extract_embedded_font_from_descriptor#1`, `fonts/unicode_fallback.rs` (2), `host/font_subset/mod.rs::subset#1`, `host/ops/fonts.rs::handle_register_fallback_font#1`, `writer/font_discovery.rs::resolve#1`, `writer/font_manager.rs::from_file#1`.
CMap table: `fonts/font_dict.rs::from_dict#4` (ToUnicode), `resolve_encoding_writing_mode#1`, `host/redact_scrub.rs::rewrite_font#1`.
Sampled function (type 0): `rendering/mesh_shading.rs` (2), `rendering/page_renderer.rs::evaluate_type0_multi#1`, `parse_type0_transfer_function#1`, `rendering/resolution/color.rs::evaluate_type0_sampled#1`.
DSS: `signatures/pades/dss_read.rs::stream_array#1`. Xref stream: `xref.rs::parse_xref_stream#1`.
No site is allowed as "one object-stream member", "a progressive JPEG raster" or "a JPX recompressed at full size"; the code that reads those does not match a pattern today (or sits behind `take`).

### Sites that fail the lint (59), by what they read

Not allowed, none hidden under a wrong kind. The bounded replacement is one line each.

Lane-reachable, a whole read of something that is not a table:

- `compliance/converter.rs:533, :536` (`add_pdfa_identification`): the XMP packet, decoded whole → read it as XML events, as `extractors::xmp` does.
- `document.rs:11262` (`extract_text_from_ap_stream`): a widget's AP/N stream → feed `decoded_reader` to `Operators`.
- `document.rs:25029` (`extract_embedded_files`) and `host/attachments.rs:57` (`attachment_bytes`): an embedded file → copy to the sink in pieces.
- `host/ops/editor.rs:570` (`embedFile`) and `:629` (`addImageStamp`), `host/ops/convert.rs:156` (the no-sink fallback): the caller's source read whole by `read_all_from_reader` → keep the reader and stream it into the object.
- `editor/document_editor.rs:12122, :12130` (`get_page_images`): the page's content, concatenated → `page_operators`, as the other page walks do.
- `editor/document_editor.rs:5027` (`deflate_stream`): a staged stream with no size guard → the streaming copy.
- `editor/document_editor.rs:3848` (`write_stream_copy`): read whole only when `data_len <= MEMORY_STREAM` (256 KiB); a size-guarded read, not a table, and the lint has no class for it. Plan decision needed.
- `extractors/images.rs:1280, :1283` (JPEG chain, raw samples), `:2593, :2599, :2686, :2697` (JBIG2 stream and globals), `:2768` (JPX codestream): image data. `whole_reads.txt` and "What the engine holds" document JBIG2/JPX as decoder input and `hugeJbig2` has its own ceiling, but neither is a kind on the plan's list → decide whether to add them or bound them.
- `host/image_embed.rs:157`: an interlaced PNG file, read whole (documented in `whole_reads.txt`, not a listed kind).
- `rendering/separation_renderer.rs:304, :476` (page content), `:533, :2325` (form content), `:3179` (a 1-bpc mask): the separation renderer → `page_operators` / `form_operators` / the row cursor.
- `decoders/stream.rs:243` (`decode_slice`): the decoded output of an in-memory slice, called by each `Decoder::decode` → callers move to `chain`.
- `signatures/tsa_client.rs:144`: the timestamp server's reply, no cap → `.take(MAX_TSA_REPLY)`.

Tables by nature of a kind the list does not name (so not allowed): `extractors/images.rs:1812` and `rendering/resolution/color.rs:609` (Indexed palette lookup stream, at most 256 entries by spec), `fonts/font_dict.rs:1773` (CIDToGIDMap, at most 128 KiB), and the Type 4 PostScript calculator functions `rendering/mesh_shading.rs:1117`, `rendering/page_renderer.rs:10214, :10389`, `rendering/resolution/color.rs:980`, `rendering/resolution/pipeline.rs:273` (the list says "sampled function"). Extend the kinds or bound them with a read of the spec's maximum.

Surfaces no host op reaches (bindings, command-line tools, the document builder, OCR, office import): `api/pdf_builder.rs:1775`, `bin/analyze_pdf_features.rs:127`, `converters/office/{docx.rs:57,:276; pptx.rs:82,:200; mod.rs:145,:195,:233,:285}`, `extractors/auto.rs:1083, :1161`, `ffi.rs:3613, :4554, :7646`, `python.rs:2624`, `ocr/detector.rs:40`, `ocr/recognizer.rs:60, :63`, `document.rs:1204` (`open(path)`), `editor/document_editor.rs:2018` (`merge_from(path)`), `split_bookmarks.rs:321`, `wasm.rs:2574`, `writer/image_handler.rs:199`. I did not exempt them: the plan walks all of `src`. The assistant can add a skip list like `check_cancel_polls.sh`'s.

The lint is red until these are planned. `make test-guards` and `make check` fail on it; that is the intended state.

## Gate

| step | result |
|---|---|
| Rust suite (`cargo nextest run --lib`, plan's features, debug info off) | `Summary [ 533.334s] 6720 tests run: 6720 passed (7 slow), 5 skipped` |
| `make test-fuel` | every row passes except `saveFull tagged ... 921194837 ... FAIL: total below 1000000000`, which fails the same way on the untouched code (966M in my first baseline run); not mine, not edited |
| `make test-memory` | probe tests `ok. 7 passed`; 71 rows, none FAIL (13 m 37 s) |
| `bash tool/check_cancel_polls.sh` | `every loop over a document-scale collection polls for a cancel` |
| `bash tool/check_whole_buffers.sh` | `59 site(s) read a whole buffer that is not a table` (exit 1, see above) |
| `bash tool/check_patch_markers.sh` | no output, exit 0 |
| `bash tool/check_fork_tests.sh` | no output, exit 0 |
| `bash tool/check_changelog_shape.sh` | no output, exit 0 |
| `dart analyze --fatal-infos lib hook test bin` | `No issues found!` |
| `make test-ops-native DART=dart` | `00:... +663 ~1: All tests passed!` (7 m 57 s) |

The Rust suite ran with `--lib`: the integration-test and example binaries of the plan's command exhaust the disk allowance here (25 GB of target); I cleared `target/debug/{incremental,examples}` once for that reason.

## Choices where the plan left one open

- Poll through the job's installed check in the redaction pass instead of threading a `CancelCheck` (above).
- Text extraction of forms uses an explicit check, not the thread-local (above).
- The lint skips helper bodies and `.take(` windows (above); it does not skip any directory.
- Kept the fuel size at 20,000 lines; added the save and the form case instead of growing.
- The `convert long` and `extractAll form` fixtures have sizes of their own (`CONVERT_TRIPLES` 100,000; the form reuses `WHOLE_FORM_TRIPLES`), chosen to cross the bound on the old code inside `MAX_TOTAL`.
- One Fixed bullet in `CHANGELOG.pre.md` covers redaction, form text extraction and image bounds; conversion is not named in it.
