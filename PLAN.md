You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-563: a backing per web reader. Approved for 6.0.0 (design d-131, ruled; its text is below).

Read first: docs/ARCHITECTURE.md (the web runtime, the four modes, "Memory"), web_assets/executor_worker.js (`hostReadAt` near :188: it already checks a reader's `opfsHandle` before the mode switch; the per-mode reads; `dropReader`; `resetLane`; the held-reader map near :703-735), lib/src/runtime/web/lease.dart (the OPFS pre-copy near :777-860, read answers that copy at :777-781, `cancelWake`, `_heldSources` near :248), lib/src/runtime/web/protocol.dart, lib/src/types/data_source.dart (the `DataSource` contract), lib/src/io/ (memory_source.dart: `readAt` returns a view; file_source.dart:27 tells web users to write their own Blob source), lib/src/runtime/pool.dart (`peak`, the keep line `KEEP_BYTES`), the barrel export guard test/guards/barrel_exports_test.dart (the approved public names).

## The design (d-131, settled)

A reader in the worker carries one of three backings; the mode's read path is only the fallback for a reader with none:
- **Blob**: a new public `BlobSource` (web only; lib/src/io/, exported where the barrel guard allows web-only types, with its name added to the approved list) wraps a browser `Blob` (a `File` from a picker is one). Its Blob is handed to the worker with the job (structured clone, by reference, no copy) and read there with `FileReaderSync` on a slice: O(1) memory, no pre-copy, any size, no main-thread round trip, in every mode. `readAt` on the main thread still works (for native-style use), through `Blob.slice().arrayBuffer()`.
- **Bytes**: any other source whose `length` fits in what is left of the lane's transfer budget is read whole on the main thread once and transferred to the worker as an ArrayBuffer with the job (a `MemorySource` is copied first: its `readAt` returns a view, and transferring it would detach the user's bytes). Reads are a synchronous copy from it.
- **OPFS handle**: the pre-copy, as today, in OPFS mode, only for a source past the budget.
A user's own `DataSource` without a Blob takes the bytes backing under the budget, else the mode's own path.

The budget: `kTransferBytes` = 16 MiB (provisional), the most source bytes one lane may hold as copies in its worker, own and held sources together; at most `KEEP_BYTES / 2`. A source joins when it fits in what is left: a merge of 100 small inputs transfers until the budget fills and the rest take the fallback. Never an error.

Memory: the worker reports `held` (the sum of bytes-backed readers) on `result` and `resetDone` beside `peak`; the pool reads it as a diagnostic. Blob backings hold nothing. docs/ARCHITECTURE.md "Memory" gains one sentence: on web, a non-Blob source under the budget is copied once into the worker, as the pre-copy copies it to disk.

The contract: lib/src/types/data_source.dart's lifetime sentence becomes "while a handle opened from it is live" (today it says "during an op", but the editor already reads the source at save time and holds merged sources for its life). A copied source is a snapshot at job start.

Edges: a short read of the whole-source read fails the job, and the reader's `length` is the buffer's byteLength; a custom source's awaited `readAt(0, length)` races `cancelWake` as the pre-copy does; a File changed on disk makes `FileReaderSync` throw, which becomes the host's I/O error (a failure, never corruption); held groups mixing backings index through the existing reader map, and `_heldSources` stays only for readers without a backing.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown
    dart pub get --no-example && dart tool/generate_fixtures.dart
    install google-chrome or chromium, set CHROME_EXECUTABLE
    make build-wasm DART=dart && make build-wasm-asyncify DART=dart
    the runners: make test-ops-native / test-ops-opfs / test-ops-jspi / test-ops-atomics / test-ops-asyncify DART=dart

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code and the glossary in docs/ARCHITECTURE.md; a comment says only what the code cannot; no TODO; never silence a check. The engine fork is not touched by this task.

- Write each test before its fix, and see it fail on the current code.
- Show each new test can fail: break the code on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour, stop and write that in REPORT.md instead.

## Tests (a battery registered in all four web runners)

- A `BlobSource` opens and reads in every mode with zero main-thread `readAt` calls during the op (count them) and no OPFS pre-copy file.
- A `MemorySource` under the budget: no OPFS file, no main-thread read during the op after the one whole read, and the caller's bytes still readable after the op (not detached).
- A merge of more inputs than the budget holds: the first ones transferred, the rest by the mode's path; the output equals the same merge today.
- `held` equals the transferred sizes during a held document's life and zero after release and after a reset.
- Cancel during the whole read of a slow custom source completes the job cancelled without posting it.
- A `BlobSource` whose underlying Blob fails to read (patch `FileReaderSync.prototype.readAsArrayBuffer` in a blob-bootstrapped worker to throw once) fails the op with the I/O error.
- The native runner still passes (BlobSource is web only; the barrel guard says where it is exported).

## Docs

README: `BlobSource` in the web setup and the sources table, one paragraph on the three backings. docs/ARCHITECTURE.md: the reader backings in the web section, the Memory sentence. CHANGELOG.pre.md: one Added bullet (BlobSource) and one Changed bullet (small sources are copied into the worker instead of pre-copied to disk), in the shape tool/check_changelog_shape.sh checks. If the DataSource lifetime sentence is a breaking change for implementers, the migration guide gets an entry in the shape tool/check_migration.sh checks.

## Gate

`dart analyze --fatal-infos lib hook test bin`; `dart format --set-exit-if-changed lib hook tool test bin`; `dart test test/guards test/runtime test/bridge test/keep test/hook`; all five runners; `bash tool/check_changelog_shape.sh`; `bash tool/check_migration.sh`; `bash tool/check_migration_snippets.sh`. Then measure, and put the table in REPORT.md (the test test/ops/runners/web_io_measure_test.dart has the harness; add the source kind `blobsource`): open+extract and merge on 105 MB, every mode, `BlobSource` against today's Blob-over-main-thread source; and the main-thread copy time per MiB for a MemorySource at 1, 4 and 16 MiB. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: what you changed (file:line), each test and its injected failure with the message, the measurements, the summary lines, and every choice you made where the plan left one open.
- `git add -N` new files, then `git diff HEAD -- . ':!vendor' ':!REPORT.md' ':!PLAN.md' > t-563-pkg.patch`. Undo your working changes so only the patch carries them.
- Commit only `t-563-pkg.patch` and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-563`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-563`, and do not clone it.
