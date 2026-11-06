# t-561 — an op's last output bytes are written by an explicit flush, never by a writer's Drop

## Cause

`Drop` of both host writers flushed the buffered tail and ignored the result:
`vendor/pdf_oxide/src/host/native/callback_writer.rs:141-145` (`CallbackWriter`,
`let _ = self.flush_pending()`) and `vendor/pdf_oxide/src/host/wasm/js_writer.rs:90-94`
(`JsCallbackWriter`, `let _ = self.flush_buffer()`). Nothing under `src/host/ops` calls
`flush()`, so the last write of an op (anything under the 256 KB buffer is *only* a last write)
happened in a drop, and a failure or cancel there could not reach the response.

The plan says `handle_request` owns the sinks and drops them after the response is built. That
is not so for the sinks the ops use: every sink op calls `ctx.take_sink(0)`
(`src/host/ops/mod.rs:60`), which **removes** the sink from the vec, and the op drops it at its own
end. A flush placed after the op in `handle_request` would have found an empty `sinks` vec. See
"Choices" (1).

Reproduced before the fix, on native, through the Dart API with a sink whose first write throws
(`CountingSink(failOnWrite: 1)`): `builder.save` and `doc.extractAttachment` resolved OK with
the sink holding nothing; `pdf.merge`, `editor.save`, `extractText`, `render`, `extractImages`
already failed (their engine paths flush, or the Dart side captures the sink's error).

## What changed

Fork (`vendor/pdf_oxide`, `t-561-fork.patch`):

- `src/host/bridge_api.rs` `handle_request`: before the op runs it keeps a shared handle to every
  sink. After the op: status ok -> flush every sink in order; a flush error becomes
  `ResponseWriter::cancelled()` when the error is the host's `"operation cancelled"`, else
  `ResponseWriter::error_of(&Error::Io(e))` (the response an op gives for a failed write,
  `ErrorKind::Io`). Status not ok (originally or after a flush error) -> no flush; every sink is
  discarded.
- `BoxedWriter` is now a handle on a shared core (`Arc<Mutex<..>>`), with `buffered()`
  (a `BufferedSink` writer: `Write + Send + discard()`), a private `share()` and `discard()`.
  `BoxedWriter::new` is unchanged for every plain writer, so no existing call site or test changed.
- `callback_writer.rs`, `js_writer.rs`: `Drop` no longer writes. It does
  `debug_assert!(nothing buffered || thread::panicking())`; `discard()` empties the buffer.
  The "finished" state of the plan is the empty buffer: a flush leaves it empty, a discard
  empties it, and a drop with bytes in it after an ok op is the debug-build panic.
- `native/executor.rs` and `wasm_entry::execute` build their sinks with `BoxedWriter::buffered`.
- `binary_codec.rs`: `ResponseWriter::is_ok(&[u8])`.

Package (`t-561-pkg.patch`):

- `lib/src/bridge/shared_bridge_doc.dart`: `extractText` aborts the caller's sink through the
  handle's `scope` (as `_exec` does) — see "Existing test" below; and two comments that said the
  engine's last chunk is flushed as it finishes so a failed sink does not fail the op, now false.
- `CHANGELOG.pre.md`: one Fixed bullet in 6.0.0-dev.0.
- `test/ops/core/pdf_abort_battery.dart`: the new Dart test.

## Tests

Rust (all under `src/host/`, plain `#[cfg(test)]`, no document types):

| test | file | what it proves |
|---|---|---|
| `an_ok_op_has_its_sinks_flushed` | bridge_api.rs | ok op: one flush by the door, output delivered, no discard |
| `a_failed_last_flush_fails_the_op` | bridge_api.rs | flush error -> status 0, kind `Io`, bytes discarded |
| `a_cancel_at_the_last_flush_cancels_the_op` | bridge_api.rs | `"operation cancelled"` at flush -> response `[2]` |
| `a_failed_op_does_not_flush_its_sinks` | bridge_api.rs | non-ok op: 0 flushes, 1 discard |
| `a_flush_sends_the_buffered_bytes_once` | callback_writer.rs | the real `CallbackWriter` with an acknowledging host sends 1 chunk on flush |
| `a_discarded_writer_sends_nothing_when_dropped` | callback_writer.rs | after `discard`, drop sends 0 chunks |
| `dropping_buffered_bytes_after_an_ok_op_is_a_bug` | callback_writer.rs | debug build: drop with bytes buffered panics (`should_panic`) |

The `bridge_api` tests use a mock sink that buffers like the host writers (write succeeds,
`flush` delivers or refuses) driving `builderCreate` + `builderSave` through `handle_request`.

Dart (`test/ops/core/pdf_abort_battery.dart`, group "an op whose last write fails fails and
aborts its sink once", registered by all five runners): `builder save`, `editor save`,
`extractAttachment`, each with `CountingSink(failOnWrite: 1)` on an output smaller than the
write buffer, so the one write is the final flush: the op throws a non-cancel error, the sink saw
1 write, was aborted once, holds nothing. `editor save` already passed before the fix and is kept
as a guard. The cancel-at-the-final-flush case is covered in Rust only (the Dart sink cannot make
a cancel land between the last write's acknowledgement and the response).

### Each test failing on the current code

Before the fix (writer tests with a behaviour-neutral `discard` stub so they compile):

- `an_ok_op_has_its_sinks_flushed`: `assertion left == right failed: flushed once, by the door  left: 0  right: 1`
- `a_failed_last_flush_fails_the_op`: `left: 1 right: 0` (response ok where an error is expected)
- `a_cancel_at_the_last_flush_cancels_the_op`: answer was an ok frame `[1, 1, 0, 8, 115, 116, ...]`, not `[2]`
- `dropping_buffered_bytes_after_an_ok_op_is_a_bug`: failed (no panic: the old Drop wrote)
- `a_discarded_writer_sends_nothing_when_dropped` (with the stub made a no-op): `a drop after a failed or cancelled op writes nothing  left: 1  right: 0`
- Dart native: `builder save` and `extractAttachment`: `Expected: throws not <Instance of 'PdfCancelled'>  Actual: <Instance of 'Future<void>'>  Which: emitted <null>`

### Each injected failure (fix broken on purpose, run, restored; `diff` against a saved copy and `git diff` show nothing injected left)

1. door never flushes (`if false && is_ok`): FAIL `an_ok_op_has_its_sinks_flushed` (`flushed once, by the door  left: 0 right: 1`), `a_failed_last_flush_fails_the_op` (`the response is an error, not ok`), `a_cancel_at_the_last_flush_cancels_the_op` (`left: [1,1,0,...] right: [2]`).
2. door never discards: FAIL `a_failed_op_does_not_flush_its_sinks` (`a failed op's sink is discarded  left: 0 right: 1`), `a_failed_last_flush_fails_the_op` (`the refused bytes are dropped`).
3. Drop writes again: FAIL `dropping_buffered_bytes_after_an_ok_op_is_a_bug` (`test did not panic as expected`). `a_discarded_writer_...` still passes here, because `discard` has already emptied the buffer: it guards the discard, the assert test guards the drop.
4. door flushes after a failed op too (`if true`): FAIL `a_failed_op_does_not_flush_its_sinks` (`assertion left == right failed`).
5. cancel mapped to a generic error (`if false` in `write_failure`): FAIL `a_cancel_at_the_last_flush_cancels_the_op` (`assertion left == right failed`).
6. Dart, door never flushes, native runner: FAIL `builder save` and `extractAttachment` with the message above.

## Measurement

`make build-wasm DART=dart` then `dart --packages=.dart_tool/package_config.json tool/asyncify_list.dart web_assets/pdf_oxide_bg.wasm`:

Before (wasm 20877699 bytes):
```
functions: 8740 (12 imports), address-taken: 2354
pausing imports: (./pdf_oxide_bg.js.__wbg_host_read_at_9846ac94fd847750, ./pdf_oxide_bg.js.__wbg_host_write_chunk_d8ab01a752df09ac)
callers by direct calls only: 20
callers with call_indirect resolved by signature: 7446 (85.2% of the module)
signature resolution, in the order it grows the set:
  (i32,i32,i32,i32)->()  address-taken ancestors 7, call-site functions 167, adds 1059
  (i32)->()  address-taken ancestors 6, call-site functions 321, adds 922
  (i32,i32)->()  address-taken ancestors 85, call-site functions 78, adds 4665
  (i32,i32,i32)->(i32)  address-taken ancestors 45, call-site functions 114, adds 724
```
After (wasm 20879432 bytes):
```
functions: 8744 (12 imports), address-taken: 2355
pausing imports: (./pdf_oxide_bg.js.__wbg_host_read_at_9846ac94fd847750, ./pdf_oxide_bg.js.__wbg_host_write_chunk_d8ab01a752df09ac)
callers by direct calls only: 19
callers with call_indirect resolved by signature: 7449 (85.2% of the module)
signature resolution, in the order it grows the set:
  (i32,i32,i32,i32)->()  address-taken ancestors 7, call-site functions 163, adds 1059
  (i32,i32)->()  address-taken ancestors 53, call-site functions 75, adds 5586
  (i32,i32,i32)->(i32)  address-taken ancestors 45, call-site functions 114, adds 724
```
Reading it: the `(i32)->()` signature (the drop glue's, the t-541 reason) no longer adds a
group, and one function fewer is a direct caller of a pausing import (20 -> 19), but the list does
**not** shrink: 7446 -> 7449 callers (85.2% of the module both times), because the
`(i32,i32)->()` signature now reaches 5586 functions where it reached 4665 and the
`(i32)->()` group's 922 are covered anyway. The drop glue stopped calling the pausing import; the
list tool still has to assume the other `(i32,i32)->()` call sites can.

`make build-wasm-asyncify DART=dart` (Asyncify wasm):

- before: 28229263 bytes (gzip 10159632)
- after: 28235650 bytes (gzip 10159276), +6387 bytes

## Gate (summary lines)

- `dart analyze --fatal-infos lib hook tool test bin`: `No issues found!` (exit 0). `tool/bench`
  (a separate package) needed its own `dart pub get` first; until then its 12 missing-package
  errors were the only ones, none in a file touched here.
- `dart test test/guards test/runtime test/bridge test/keep test/hook`: `+447 -1`. The one failure
  is the wasm size wall: `no wasm in web_assets/ reaches the 25 MiB static-asset limit`,
  `limit is 26214400 bytes; built: {pdf_oxide_asyncify_bg.wasm: 28235650, pdf_oxide_bg.wasm: 20879432}`.
  It already failed on the baseline (28229263); not this task's.
- Rust, the plan's feature list, `cargo nextest run --no-fail-fast`: `Summary [ 760.037s] 10280 tests run: 10280 passed (10 slow), 65 skipped`.
  Built with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`: the default debug build of
  every integration test filled the disk (`ld` died with SIGBUS at 0 bytes free).
- `bash tool/check_patch_markers.sh`: exit 0. `bash tool/check_fork_tests.sh`: exit 0.
  `bash tool/check_cancel_polls.sh`: exit 0 (`every loop over a document-scale collection polls for a cancel`).
  `bash tool/check_changelog_shape.sh`: exit 0.
- Native runner: `01:08 +666 ~1: All tests passed!`
- OPFS: `02:50 +684 ~9: All tests passed!`
- JSPI: `01:49 +679 ~1: All tests passed!`
- Asyncify: `02:12 +679 ~1: All tests passed!`; auto-mode: `00:05 +3: All tests passed!`
- Atomics (`chrome-coi`): four full runs on the final code: `+675 ~2 -1` and `+675 ~2 -1`
  (both the same stress test, `build and open a 15,-page image PDF inline`, 18.22x and 16.98x
  against a 15x budget, op 415 ms and 350 ms), then `01:20 +676 ~2: All tests passed!` and
  `01:19 +676 ~2: All tests passed!` (op 62 ms and 77 ms; in isolation 165 ms, ratio 4-6x).
  A timing budget, not an assertion; I take it as load noise on this box but did not prove it
  against a baseline wasm.

### An existing test failed, and what I did

After the fix, native, JSPI, Atomics and Asyncify each failed one existing test:
`abort a combinator that ends cancelled aborts its output reads, one extractText`
(`_expectAbortedOnce`: `Expected: <1> Actual: <2>  aborted once`). It cancels the task from the
sink's first write. Before, the cancel landed in the Drop flush whose result was ignored, so the
engine answered ok. Now the final flush sees the cancel and the engine answers cancelled, which
reaches `extractText`'s catch inside `readOnce`: it aborted the caller's sink through its own
group hook and `_own` aborted it again through the combinator's hook, and the dedupe is per hook.
The test pins the abort-once contract, not the old behaviour, so I did not touch it; I made
`extractText` abort through `scope ?? hook`, as `_exec` already does. This is a package change the
plan did not list.

## Choices where the plan left one open

1. Sinks: `handle_request` keeps shared handles to every sink (the ops take theirs), rather than
   changing `take_sink` or every op. This is why `BoxedWriter` changed although the plan said to keep it.
2. `BoxedWriter` grew a `buffered()` constructor and `discard()`; `new()` and plain writers are
   untouched. The mutex is uncontended (one op thread); it is there for `Send`.
3. "finished" is the empty buffer, not a separate flag (equivalent; one less state to keep in sync).
4. The drop assert is skipped while panicking: an op that panics unwinds with bytes buffered.
5. The host's cancel is recognised by its message, `"operation cancelled"`, as `engine_proofs` do.
6. A flush failure that is not a cancel answers `error_of(Error::Io)` (kind `Io`), what an op
   answers for a failed write. No new kind.
7. Dart test: the three ops above, not "the battery of throwing sinks" generally, because the
   others already fail through Dart-side capture; skipped in OPFS (next section).
8. Rust tests live in `bridge_api.rs` / `callback_writer.rs` `#[cfg(test)]` modules (the wall allows
   them there: no document types), not `engine_proofs`, whose tests must explain why no Dart battery
   sees them — here one does.
9. Two stale comments in `shared_bridge_doc.dart` reworded; the code under them is unchanged.

## For the maintainer (not fixed)

OPFS mode never tells the engine that a sink refused a chunk: `_serveChunk` records the error and,
for OPFS, discards it (`// OPFS mode: fire-and-forget writes, no ack channel`,
`lib/src/runtime/web/lease.dart`). So on OPFS an op with a throwing sink succeeds unless the Dart
side captures the error itself (`extractText`, `render`, `extractImages` do; `builder.save`,
`editor.save`, `extractAttachment` do not). `editor.save` passed on the old code on every other
mode and fails on OPFS, so it is not this change. The new Dart test skips OPFS with that reason
(`markTestSkipped`); fixing it is a redesign of the OPFS transport.
