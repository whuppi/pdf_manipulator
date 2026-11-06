# t-562: the web read path, measured

Only change: `test/ops/runners/web_io_measure_test.dart` (carried by `t-562-pkg.patch`). `lib/`, `vendor/pdf_oxide` and the rest of the tree are as they were; the window experiment was local and reverted (`git -C vendor/pdf_oxide diff` is empty, `git status` shows nothing else).

## Machine

- `nproc` = 4; Google Chrome for Testing 153.0.8010.12, headless (`--no-sandbox --disable-gpu`, from `dart_test.yaml`).
- Files: 105 MB = 110,569,021 bytes, 316 MB = 332,754,357 bytes (`-DMEASURE_MB=105` / `316`). 50 pages extracted, merge of two copies, as in t-541.
- `uptime` before each run (the raw log keeps `nproc` and the Chrome version before each run too):

| run | uptime line |
|---|---|
| 105 memory, chrome | 14:31:58 up 14 min, load 2.19 2.96 2.03 |
| 105 memory, chrome-coi | 14:33:33 up 15 min, load 1.47 2.55 1.97 |
| 105 blob, chrome | 14:34:00 up 16 min, load 1.34 2.42 1.95 |
| 105 blob, chrome-coi | 14:36:54 up 19 min, load 1.67 2.21 1.95 |
| 316 memory, chrome | 14:37:54 up 20 min, load 1.30 1.99 1.89 |
| 316 memory, chrome-coi | 14:42:54 up 25 min, load 0.97 1.42 1.68 |
| 316 blob, chrome | 14:44:10 up 26 min, load 1.19 1.38 1.64 |
| 316 blob, chrome-coi | 14:54:04 up 36 min, load 1.30 1.33 1.50 |
| window 256 KiB, constant only | 15:03:17 up 45 min, load 2.80 1.96 1.68 |
| window 256 KiB, read clamp raised | 15:09:43 up 52 min, load 1.01 1.28 1.45 |
| window 1 MiB, read clamp raised | 15:16:35 up 58 min, load 2.73 1.78 1.57 |

The load at the start is the tail of the run before (each run begins with a fresh `dart test` compile).

## Table

Times are ms, run 1 / run 2 (the two runs alternate with the other modes). `reads` is the count of source `readAt` calls for open+extract, then for the merge (two inputs, so both inputs' reads). Window rows are Blob source, 105 MB, plain chrome.

### Base window (64 KiB)

| mode | source | size | open+extract ms | merge ms | reads (open+extract / merge) |
|---|---|---|---|---|---|
| jspi | memory | 105 | 10035 / 9652 | 2675 / 2629 | 7405 / 12333 |
| atomics | memory | 105 | 8439 / 8237 | 1785 / 1727 | 7405 / 12333 |
| opfs | memory | 105 | 9288 / 8715 | 2115 / 2155 | 1688 / 3376 |
| asyncify | memory | 105 | 16532 / 15462 | 3915 / 3411 | 7405 / 12333 |
| jspi | blob | 105 | 16578 / 16143 | 14443 / 13649 | 7405 / 12333 |
| atomics | blob | 105 | 15065 / 14546 | 12013 / 10991 | 7405 / 12333 |
| opfs | blob | 105 | 10440 / 10235 | 5048 / 5089 | 1688 / 3376 |
| asyncify | blob | 105 | 22460 / 22392 | 14878 / 14728 | 7405 / 12333 |
| jspi | memory | 316 | 33229 / 33578 | 9964 / 9683 | 30412 / 47493 |
| atomics | memory | 316 | 28045 / 27216 | 6522 / 6240 | 30412 / 47493 |
| opfs | memory | 316 | 29562 / 30166 | 7303 / 7294 | 5078 / 10156 |
| asyncify | memory | 316 | 53414 / 52060 | 12735 / 12482 | 30412 / 47493 |
| jspi | blob | 316 | 58886 / 59898 | 49796 / 52567 | 30412 / 47493 |
| atomics | blob | 316 | 52818 / 52472 | 42619 / 42524 | 30412 / 47493 |
| opfs | blob | 316 | 33634 / 34585 | 15247 / 15373 | 5078 / 10156 |
| asyncify | blob | 316 | 79862 / 79236 | 54124 / 53887 | 30412 / 47493 |

OPFS "reads" are the pre-copy's reads (64 KiB each, `lease.dart`); the engine then reads the OPFS file, not the source. OPFS's ms include the pre-copy.

### Window experiment (105 MB, blob)

| window | mode | open+extract ms | merge ms | reads (open+extract / merge) |
|---|---|---|---|---|
| 64 KiB (base) | jspi | 16578 / 16143 | 14443 / 13649 | 7405 / 12333 |
| 64 KiB (base) | asyncify | 22460 / 22392 | 14878 / 14728 | 7405 / 12333 |
| 256 KiB, constant only | jspi | 16034 / 16293 | 13329 / 14133 | 7405 / 12333 |
| 256 KiB, constant only | asyncify | 22412 / 21932 | 14208 / 14391 | 7405 / 12333 |
| 256 KiB, constant + read clamp | jspi | 11491 / 10875 | 5134 / 4784 | 1746 / 2922 |
| 256 KiB, constant + read clamp | asyncify | 16678 / 16598 | 5511 / 5462 | 1746 / 2922 |
| 1 MiB, constant + read clamp | jspi | 9783 / 9365 | 3196 / 3240 | 452 / 740 |
| 1 MiB, constant + read clamp | asyncify | 16017 / 15069 | 3939 / 3840 | 452 / 740 |

## What the numbers say

(a) Order. With the in-memory source the open+extract order is Atomics, OPFS, JSPI, Asyncify (105 MB: 8.3, 9.0, 9.8, 16.0 s; 316 MB: 27.6, 29.9, 33.4, 52.7 s), and the same on merge (105 MB: 1.8, 2.1, 2.7, 3.7 s). With the Blob source OPFS is first, then Atomics, JSPI, Asyncify (105 MB open+extract: 10.3, 14.8, 16.4, 22.4 s; merge: 5.1, 11.5, 14.0, 14.8 s; 316 MB open+extract: 34.1, 52.6, 59.4, 79.5 s; merge: 15.3, 42.6, 51.2, 54.0 s). JSPI and Asyncify are close on Blob merge (14.0 and 14.8 s at 105 MB; 51.2 and 54.0 s at 316 MB). Asyncify is slower than JSPI by 5-6 s at 105 MB and 20-26 s at 316 MB on open+extract in both sources. The read count is the same for JSPI, Atomics and Asyncify (7,405 + 12,333 at 105 MB), and OPFS makes a quarter of it (1,688 + 3,376, the pre-copy). Moving from memory to Blob adds about 6.5 s to JSPI's open+extract for 7,405 reads (about 0.9 ms a read) and 11.8 s to its merge for 12,333 reads; OPFS pays the Blob once in its pre-copy and so gains least. Caveat: Atomics ran on `chrome-coi`, the others on plain `chrome`, as the plan sets.

(b) Window. Raising only `READ_BUF_CAPACITY` changed nothing: the read counts and times equal the base (the worker's `hostReadAt` and `SabLayout.maxChunk` in `lib/src/runtime/web/protocol.dart` cap every read at 64 KiB). With that cap raised as well (local, reverted), 256 KiB cuts reads to 1,746 + 2,922 and 1 MiB to 452 + 740. JSPI: open+extract 16.4 s to 11.2 s to 9.6 s, merge 14.0 s to 5.0 s to 3.2 s. Asyncify: open+extract 22.4 s to 16.6 s to 15.5 s, merge 14.8 s to 5.5 s to 3.9 s. The step from 256 KiB to 1 MiB gains JSPI 1.6 s on open+extract and 1.8 s on merge; Asyncify 1.1 s and 1.6 s. Asyncify's open+extract stays 5-6 s over JSPI's at every window. The memory cost is below.

## What a larger window costs in memory

Every user of `READ_BUF_CAPACITY` (grep over `vendor/pdf_oxide` and `lib`):

- `host/wasm/js_reader.rs:74`: the web reader's window, a `vec![0; READ_BUF_CAPACITY]` allocated at an open source's first miss and held for the source's life. This is the per-open-source cost on web, in the executor's wasm heap (counted by the heap peak that `KEEP_BYTES` reads).
- `host/native/shared_buffer.rs:76` (`read_channel::DATA_CAPACITY`, `TOTAL_SIZE` = 160 + that) and `native/callback_reader.rs:105`: the native read channel, one per open source, fixed at the capacity.
- `host/incremental.rs:221` and `host/sign.rs:218`: one `vec![0u8; READ_BUF_CAPACITY]` chunk while that operation runs (transient).
- `host/engine_proofs/font_subset_walk.rs:21,46,162`: test-only.
- Outside the constant, but tied to it: `SabLayout.maxChunk` sizes the Atomics read SAB (`readSabBytes`, per source) and the OPFS pre-copy's chunk in `lease.dart`; the `DataSource` contract and `TestSource` say "64KB max per call"; the worker clamps to `sabMaxChunk`. The experiment needed `maxChunk` raised too, which is why the constant alone did nothing.

Cost per open source: +192 KiB at 256 KiB (4x the 64 KiB window), +960 KiB at 1 MiB (16x). A merge of N inputs, or a document held open with its sources, holds N windows, so a 100-input merge goes from 6.25 MiB of windows to 25 MiB at 256 KiB and 100 MiB at 1 MiB. The d-130 promise (ARCHITECTURE.md "Memory") is a fixed ceiling over the memory the process already holds, whatever the file size; a larger window raises that ceiling by (N sources x the increase), and the `limit` rows of `tool/memprobe` would have to be re-set. It does not make memory grow with file size. I did not run `make test-memory`.

## Choices the plan left open

- Blob source: the package has none (`FileSource` is `dart:io`, native only; `DataSource` docs only mention `Blob.slice`), so the test has a private `_BlobSource`, the narrowest `DataSource` over `Blob.slice().arrayBuffer()`, built from a `web.Blob([bytes])`. It has no 64 KiB guard (the in-memory `TestSource` throws above 64 KiB, so the window rows use Blob only).
- `MEASURE_MODES` is split at `,` or `+`: `dart test` splits `--dart2js-args` at a comma, so the comma form fails in dart2js ("Extra arguments"). All runs used `+`.
- A mode the platform cannot run is skipped via `markTestSkipped` when `open` throws `PdfUnsupported`. No mode was unsupported in these runs (JSPI ran on plain `chrome` with Chrome 153; the plan's note that plain chrome has no JSPI did not hold here), so the skip path itself was not exercised. I did not force `webIoMode` away from what the plan listed.
- Each platform run takes the modes together (`chrome`: jspi, opfs, asyncify alternating; `chrome-coi`: atomics alone), so Atomics's two runs are back to back.
- The window experiment: the plan said to change `READ_BUF_CAPACITY` only. That changes nothing (shown in the table), so I took the narrowest further step: also raised `SabLayout.maxChunk` in `lib/src/runtime/web/protocol.dart` to the same value, rebuilt, measured, and reverted both. Rebuilt the normal and Asyncify wasm after the revert.
- `dart analyze` needed `dart pub get` in `tool/bench` (a separate package) to stop reporting 12 unresolved-import errors there; none were in code I touched.
- `make build-wasm` / `build-wasm-asyncify` ran on Rust 1.97 (the build warns the tested version is 1.92).

## Gate

```
$ dart analyze --fatal-infos lib hook tool test bin
Analyzing lib, hook, tool, test, bin...
No issues found!
$ dart format --set-exit-if-changed test
Formatted 160 files (0 changed) in 0.50 seconds.
$ dart test test/ops/runners/web_io_measure_test.dart -p chrome      (no MEASURE_MB)
00:00 +0 ~8: All tests skipped.
```

Summary lines of the runs: `02:48 +6: All tests passed!` (105 blob chrome), `09:49 +6: All tests passed!` (316 blob chrome), `03:12 +2: All tests passed!` (316 blob chrome-coi), and every other run ended `All tests passed!`.
