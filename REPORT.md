# t-576 — deflate cost of a page save

**Outcome: stopped before the fix.** The cause is in files outside my share (`vendor/pdf_oxide/src/editor/document_editor.rs`, and the per-operator writers it drives). No file of the tree is changed, so there are no `t-576-*.patch` files. This report is the deliverable.

The plan's baseline also did not reproduce (see "What did not reproduce"). Please read that section before acting on the fix.

## What did not reproduce

The plan says one page of 150,000 `q 1 0 0 1 0 0 cm Q` triples (2.85 MB) costs 3.4e9 wasm instructions without deflate and 1.96e10 with it (~5,000 instr/byte). I measured the same page in the fuel probe's wasm:

| save path (fixture: 150,000 triples, 2.85 MB) | no deflate | engine level 9 | extra for deflate |
|---|---|---|---|
| page edited (`setImageBounds`, goes through `write_pass_stream`) | 3.366e9 | 4.901e9 | 1.53e9 (~537 instr/B) |
| page copied (`recode`, no edit) | 3.5e6 | 7.22e8 | 7.2e8 (~253 instr/B) |
| `compress` options (merge duplicates + prune resources), copied page | 1.820e9 | 2.539e9 | 7.2e8 |
| same, source page Flate-encoded | 2.698e9 | 3.092e9 | 3.9e8 |

The "without deflate" figure matches the plan (3.37e9). The "with deflate" figure does not: the worst case I found is 4.90e9, four times lower than 1.96e10. The existing row `setImageBounds long` (100,000 triples) measures 3.265e9 total on the unchanged code, which is the same curve (4.90e9 × 100/150). I do not know what the plan's 1.96e10 was measured on; I could not find a save path or setting that gets near it.

## Step 1 — measurements

zlib-rs 0.6.7 (the version in `vendor/pdf_oxide/Cargo.lock`) through flate2 1.1.10 `write::ZlibEncoder`, inputs of about 3 MB: the repetitive page (2,850,000 bytes), page text (`gen::text_lines`, wasm; synthetic words, 3,101,079 bytes, native) and noise standing in for already-compressed data (3,000,000 bytes, incompressible, so every level leaves it stored-size).

**Wasm, fuel (wasmtime, `tool/fuelprobe` guest build), zlib levels, 64 KB writes / 6-byte writes:**

| input | zlib 6 | zlib 8 | zlib 9 |
|---|---|---|---|
| repetitive | 2.13e8 / 6.09e8 | 3.57e8 / 7.57e8 | 3.64e8 / 7.59e8 |
| text | 9.68e8 / 1.91e9 | 3.05e9 / 3.48e9 | 1.90e9 / 2.33e9 |
| noise | 1.36e9 / 1.79e9 | 1.19e9 / 1.64e9 | 1.47e9 / 1.91e9 |

Per byte on the repetitive page: 75, 125 and 128 instr/B at 64 KB writes. Nowhere near 5,000.

**Native (x86-64, release), ns per input byte, 64 KB writes / 6-byte writes; output bytes in brackets for the 64 KB case:**

| input | zlib 6 | zlib 8 | zlib 9 |
|---|---|---|---|
| repetitive | 1.0 / 35.2 (16,618) | 1.8 / 37.1 (6,945) | 1.9 / 34.5 (6,948) |
| text | 20.0 / 90.8 (368,173) | 106.5 / 164.3 (346,403) | 52.1 / 97.0 (346,400) |
| noise | 21.2 / 56.4 | 18.2 / 51.0 | 23.2 / 57.1 |

The engine's save level 9 runs zlib 8 (`host/deflate.rs:121`), 8 runs 8, 6 runs 6.

Inputs for the native and wasm rows are not byte-identical (different generators), so compare columns within a table, not across.

## Step 2 — cause

1. **Many small writes into the encoder (the one I can show a change removes).**
   - `ImageModStage::emit` writes every operator on its own into the encoder: `vendor/pdf_oxide/src/host/image_mods.rs:105` (and `:127`). The redaction pass does the same at `src/redaction/transform.rs:706, 723, 895`.
   - `write_pass_stream` creates the `ZlibEncoder` and hands it to the body unbuffered: `src/editor/document_editor.rs:4272`. For the repetitive page that is 450,000 writes of 2, 15 and 2 bytes.
   - Cost of the small writes alone, standalone zlib-rs: 2.1× the fuel on the repetitive page (7.57e8 vs 3.57e8), and natively 18× the time (37.1 vs 1.8 ns/B at zlib 8).
   - Change that removes it (done in a scratch copy of the tree, not in the repo): wrap the encoder in `BufWriter::with_capacity(64 * 1024, …)` in `write_pass_stream` and `into_inner()?.finish()` at the end. Result in the probe, edited-page path, 150,000 triples:

     | | before | with buffering |
     |---|---|---|
     | engine level 9 | 4.901e9 | 4.570e9 |
     | engine level 6 | 4.811e9 | 4.100e9 |
     | no deflate | 3.366e9 | 3.381e9 |
     | existing row `setImageBounds long` | 3.265e9 | 3.028e9 |

     So buffering removes 0.33e9 of the 1.53e9 deflate share in fuel (22%). It is much larger in time on native (18×), which fuel does not show.
   - The stack sample (`FUELPROBE_SAMPLE`) puts the extra time under `write_image_mods_content → ImageModStage::push → flush_held → serialize_operator` plus `zlib_rs::deflate::algorithm::slow::deflate_slow` and `slide_hash::rust::slide_hash_chain`. The pass's own cost is mainly `f32` formatting (`float_to_decimal_common_exact`) and operator parsing, not deflate.

2. **Not found: a cost I can attribute.** About 0.8e9 of the 1.19e9 deflate share that remains after buffering is not zlib-rs's own cost (standalone: 3.6e8). Level 1 (zlib-rs's quick path, nearly free) already adds 1.05e9 on the edited-page path (4.421e9 vs 3.366e9). I could not find what that is without profiling inside the engine with frames from `flate2` and `std::io`, which I did not do. Treat the fix below as removing a measured 22%, not as the whole of the plan's reported cost.

3. **Copy path deflates a stream over 256 KB twice.** `document_editor.rs:4017` counts with `recode` into a `Counter`, then `:4081` recodes for real. For the repetitive page that is 2 × 3.6e8 = 7.2e8 (measured 7.22e8). For 3 MB of text at zlib 8 it is 6.11e9, twice the standalone 3.05e9. That is the design of the memory bound (`MEMORY_STREAM`), not a bug; I left it.

4. **Level choice is not the cause for the repetitive page.** Levels 8 and 9 cost the same there (3.57e8 vs 3.64e8) and produce the same size. On text, zlib 8 (what engine level 9 runs) is 1.6× slower than zlib 9 for the same output size (3.05e9 vs 1.90e9 fuel; 106 vs 52 ns/B) and 3.2× slower than zlib 6, which is 6% larger (368,173 vs 346,403). zlib-rs 0.6.7 switches level 9 to a rolling hash and `longest_match_slow` when `max_chain_length > 1024` (`zlib-rs/src/deflate/hash_calc.rs:14`, `algorithm/slow.rs`), so level 8 walks 1,024-deep 4-byte-hash chains on text. The comment at `host/deflate.rs:115` says level 9 is worse on content streams, and I did not rerun that on the #268 corpus (see "Not run"), so I did not touch the level.

Chunking also changes the encoder's output below zlib 8: text at zlib 6 is 460,090 bytes with 6-byte writes and 368,173 with 64 KB writes. At zlib 8 and 9 the outputs are the same (346,403 / 346,400 in both). The default save level runs zlib 8, so buffering leaves its bytes unchanged and can only make levels below 9 smaller.

## Step 3 — the change I would make, and why I did not

The fix is in `vendor/pdf_oxide/src/editor/document_editor.rs`, which the plan does not name. In `write_pass_stream` (`:4271–4278`):

```rust
Some(level) => {
    let mut zlib = std::io::BufWriter::with_capacity(
        64 * 1024,
        flate2::write::ZlibEncoder::new(
            out,
            flate2::Compression::new(crate::host::deflate::zlib_level(level)),
        ),
    );
    body(&mut zlib)?;
    zlib.into_inner().map_err(|e| e.into_error())?.finish()?;
    Ok(())
},
```

That is the diff I ran in the scratch tree. It needs a fork test under the marker rules (`tool/check_fork_tests.sh`) and a patch marker around the changed lines. `ImageModStage::finish` calls `out.flush()` (`image_mods.rs`), which on a `BufWriter` flushes the buffer into the encoder and on the encoder is a sync flush; confirm that stays once per stream. I did not add a fork test or the marker, since I did not edit the file.

I took the narrower choice: no edit outside the files named. No level change either, because the repetitive page does not depend on the level.

## Step 4 — fuel row: not added

The plan asks for a row that fails with the slow path and passes with the fix. I did not add one:

- The slow and fixed paths differ by 7–10% of a row's total in fuel (4.90e9 vs 4.57e9 at 150,000 triples; 3.27e9 vs 3.03e9 for the existing `setImageBounds long`). The host has one bound for all rows (`MAX_TOTAL` 2e10, `MAX_GAP` 1.1e9), and a row can only fail the slow path by outgrowing it: 150,000 triples would need about 600,000 (11 MB), far past what a probe run should cost.
- A per-row bound would be a new host mechanism (`tool/fuelprobe/host/src/main.rs`) tuned to a 7% difference between two measurements, which would fail on unrelated codegen changes.
- The cost it should hold is native time (18×), which fuel cannot see. A native fork test that counts the write calls the encoder receives (assert a few large ones for the repetitive page) would hold it, but it needs the fix in the other file to pass.

## Tests and injected failures

None written or injected: no fix was applied, so there was nothing to put a test in front of. The temporary measurement cases (`zlib`, `copy`, `pass`, `comp`, `compf` in the fuel guest) were removed again; `git status` is clean in the repo and in both submodules.

## Summary lines

Run on the unchanged tree:

- `bash tool/check_patch_markers.sh`: exit 0, no output.
- `bash tool/check_fork_tests.sh`: exit 0, no output.
- `bash tool/check_cancel_polls.sh`: `every loop over a document-scale collection polls for a cancel`
- `bash tool/check_changelog_shape.sh`: exit 0, no output (nothing was added to CHANGELOG.pre.md, since no user-visible change was made).
- `dart analyze --fatal-infos lib hook test bin` (after `dart pub get --no-example` and `dart tool/generate_fixtures.dart`): `No issues found!`
- `make test-fuel DART=dart`: **fails on the unchanged tree.** 39 of 40 rows pass; `saveFull tagged  61764014  967292744  16890  FAIL: total below 1000000000`. That row's fixture (`TAGGED_SAVE`, `tool/fuelprobe/guest/src/main.rs`) is under the 1e9 floor; it is not related to deflate, and I did not change it, since the plan does not ask for it. The other 39 rows are within the bounds. The `setImageBounds long` row in that run: `45802681  3246206667  99` (an earlier run of the same code gave 3265008207; the total varies by about 0.5% between runs).
- Make exit: `make: *** [Makefile:354: test-fuel] Error 1`.

## Not run

- The Rust suite (`cargo nextest run … --lib`) and `make test-ops-native DART=dart`, `make test-memory`. Nothing in the tree changed, and each takes a long full build; I did not take a baseline of them.
- The #268 size table before and after: no change that affects sizes was made, and I did not download or run the `tool/bench` corpus. For the shipped default (zlib 8), buffering leaves output bytes unchanged on the three inputs above (6-byte and 64 KB writes give the same size: 6,945; 346,403; 3,000,926).

## Choices where the plan left one open

- The plan says to stop when the fix needs another file; I stopped, and did not edit `document_editor.rs` even temporarily. The buffering experiment ran in a scratch copy of `vendor/` and the guest outside the repo.
- Did not change the level: the plan's step 3 changes the level only when the cause is zlib-rs's level 9 path, and for the repetitive page it is not.
- Did not fix `saveFull tagged` or the double deflate of streams over 256 KB.
- Commit contains only this file; no co-author line, per `AGENTS.md`.
