# t-564 — a web source read past 4 GiB returned the wrong bytes

## Cause

`vendor/pdf_oxide/src/host/wasm/js_reader.rs` declared the import
`host_read_at(source_index: u32, offset: u32, count: u32, buf_ptr: u32)` and
`JsCallbackReader::read` passed `self.position as u32`. A position past 4 GiB
wrapped, the host read at `position mod 2^32`, and the engine got other bytes
with no error. (wasm-bindgen's glue also did `arg1 >>> 0`.)

## Narrowings found and fixed

1. `js_reader.rs`: the `host_read_at` offset is now `f64` (a JS number, exact
   to 2^53), like the source lengths. The call passes `self.position as f64`.
   `count` and `buf_ptr` stay `u32` (a window and a wasm address). The
   regenerated glue (`web_assets/pdf_oxide.js`) no longer applies `>>> 0` to
   the offset.
2. `js_reader.rs`, same function: `to_read = (remaining as usize).min(window)`.
   On wasm32 `usize` is 32 bits, so `remaining >= 4 GiB` wrapped and a window
   read was asked for fewer bytes than it could take (a source of 4 GiB + 4 KiB
   asked for 4096 bytes at offset 0). Now `remaining.min(window as u64) as usize`.
   This was not in the plan; it is the same narrowing on the same read.

## Paths checked, no change needed (the offset is a JS number on all of them)

- `executor_worker.js` `hostReadAt`: `Math.min(count, reader.length - offset, …)`
  on numbers; `readViaAtomics` and `readViaJspi` post `offset` in a message;
  the Asyncify path is `readViaJspi`; `readFromOpfs` uses
  `handle.read(buf, {at: offset})`. No typed array holds an offset (the
  `Int32Array`s hold status words and the answer length, at most 64 KiB).
- `lease.dart` `_serveReadAt`: `_int(obj, 'offset')` is `JSNumber.toDartInt`,
  exact on dart2js up to 2^53; `source.readAt(offset, count)`.
- Request wire: `sourceLength` travels as an i64 (two 32-bit halves, exact to
  2^53); `sourceLengths` as packed f64 (`bridge_api.rs` `execute`).
- Step 3, write side: `host_write_chunk(sink_index, buf_ptr, len)` carries no
  file offset; `len` is at most the 256 KiB write buffer and `buf_ptr` is a
  wasm address. The sink is sequential, no output position crosses the
  boundary. The only OPFS offset is the pre-copy of a *source*
  (`opfsWrite`, Dart `offset.toJS` → `sh.write(…, {at: offset})`), a number
  end to end. `executor_alloc(size)`, `executor_dealloc`, `heap_peak` are
  wasm32 memory sizes, below 4 GiB by construction. So nothing on the write
  side can overflow and nothing was changed there.

## Tests

`test/ops/platform/web/web_large_offset_battery.dart`, registered in the four
web runners (`registerWebLargeOffsetTests`):

- `PastFourGiBSource`: length 4 GiB + 64 MiB, no such buffer; `readAt` makes
  its bytes: `%PDF-1.7` header, whitespace filler, the objects of a one-page
  PDF at 4 GiB + 16, filler, then the xref (true offsets past 4 GiB), trailer,
  `startxref` and `%%EOF` at the end of the source.
- Test 1, "a source read past 4 GiB returns the bytes at that offset": open it,
  extract the page text, expect the marker `PASTFOURGIB 4294967296` and that a
  read began at an offset >= 4 GiB.
- Test 2, "a read with a full window left asks for a full window when the
  length passes 4 GiB by less than one": a source of 4 GiB + 4 KiB; every read
  with 64 KiB left must ask for 64 KiB.
- JSPI, Atomics, Asyncify run both. OPFS skips both with the reason "OPFS
  pre-copies its sources: a 4 GiB source would be copied to disk".

Before the fix (current code): test 1 failed in JSPI with
`TimeoutException after 0:01:00.000000: Test timed out after 1 minutes.` and
`PdfCancelled: Operation cancelled` (the wrapped xref offsets send recovery
scanning the 4 GiB source).

Injected failures (each rebuilt with `make build-wasm`, JSPI runner, then the
fix was restored and `git -C vendor/pdf_oxide diff` showed only the fix):

- offset narrowed again (`self.position as u32 as f64`): test 1 failed with the
  same `TimeoutException after 0:01:00` / `PdfCancelled: Operation cancelled`;
  test 2 passed (it does not exercise that line).
- `to_read` narrowed again (`(remaining as usize).min(...)`): test 2 failed with
  `Expected: empty  Actual: [(int, int):(0, 4096)]`; test 1 passed.

## OPFS: no smaller check written

`readFromOpfs` passes the number straight to `SyncAccessHandle.read({at})`, so
it has no narrowing to test. A check that exercises it past 4 GiB without the
copy would need a sparse OPFS file written at an offset past 4 GiB through a raw
worker. I tried the sparse write in a bare headless Chromium (Playwright script
in the scratchpad); it did not finish within the time I gave it and I killed it,
so I do not know that Chrome keeps such a file sparse, and I did not build the
raw-worker test on it. The OPFS path is therefore covered by reading the code,
not by a test.

## Gate (summary lines)

```
analyze: No issues found!                      (dart analyze --fatal-infos lib hook tool test bin)
format:  Formatted 274 files (0 changed)       (rc 0)
dart test test/guards test/runtime test/bridge test/keep test/hook:
         00:56 +447 -1: Some tests failed.
rust:    Summary [724.605s] 10273 tests run: 10273 passed (10 slow), 65 skipped
check_patch_markers.sh: rc 0
check_fork_tests.sh:    rc 0
check_changelog_shape.sh: rc 0
native runner:   01:17 +663 ~1: All tests passed!
web jspi:        02:11 +678 ~1: All tests passed!
web atomics:     01:29 +675 ~2: All tests passed!
web asyncify:    02:42 +678 ~1: All tests passed!
web opfs:        03:39 +684 ~8: All tests passed!
web auto mode:   00:06 +3: All tests passed!   (run by test-ops-asyncify)
```

### The one failing test is not from this change

`test/guards/wasm_size_test.dart` "no wasm in web_assets/ reaches the 25 MiB
static-asset limit" fails: the Asyncify wasm built here is 28,229,268 bytes
against 26,214,400. It was 28,229,263 bytes in my first build, before any edit,
so it fails on the unchanged code in this environment (Rust 1.97 rather than
the tested 1.92; the build warns of it). I did not change the guard or the size
budget. The maintainer should look at it with the pinned toolchain.

Setup notes: `dart analyze` over `tool` needs `dart pub get` in `tool/bench`
(its own package); without it 12 unresolved-import errors come from there. The
Rust test build filled the session disk once; I deleted `target/debug` and
re-ran with `CARGO_INCREMENTAL=0` and debug info off, as the Makefile does.
`web_assets/pdf_oxide_asyncify_bg.wasm.list` is a build artifact and is not in
the patch.

## Choices where the plan left one open

- The second narrowing (`remaining as usize`) was fixed and tested although
  the plan named only the offset; it is the same wrap on the same read.
- The patch for `web_assets/pdf_oxide.js` is included in `t-564-pkg.patch`: it
  is tracked, and `make build-wasm` regenerates it with the f64 import. The
  `.wasm` files are gitignored.
- Test 2 uses a source 4 KiB past 4 GiB so a wrapped `remaining` is below the
  window; with 64 MiB past, `min` hides the wrap.
- The fixture's filler is spaces with a newline every 64 bytes, not a comment
  line, so a parser skips it as whitespace.
- OPFS: skipped with a reason and not covered by a smaller test (above).
- Changelog: one Fixed bullet, last in the 6.0.0-dev.0 entry.
