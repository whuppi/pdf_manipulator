You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-442: two holes in the memory and cancel promises 6.0.0 publishes. Approved for 6.0.0.

Read first: docs/ARCHITECTURE.md ("Memory": the promise, the per-op ceiling, the piece budget; the cancel contract and "Stopping"), tool/check_cancel_polls.sh with tool/cancel_poll_patterns.txt and tool/cancel_poll_allow.txt (the model for the new lint), tool/fuelprobe/ (guest/src/main.rs: the op cases, `size::REDACT_LINES`; host/src/main.rs: the bound), vendor/pdf_oxide/src/host/content/operators.rs (`with_cancel`, `poll_cancel`), vendor/pdf_oxide/src/redaction/ (transform.rs:538 `run_then`, the `content` providers it calls: vendor/pdf_oxide/src/document.rs near :20359 and vendor/pdf_oxide/src/editor/document_editor.rs near :777), vendor/pdf_oxide/src/cancel.rs.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown wasm32-wasip1; cargo install cargo-nextest --locked (if missing)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    Rust: CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts" (debug info off: the default fills the disk)
    make test-fuel DART=dart ; make test-memory DART=dart ; bash tool/check_cancel_polls.sh

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code; a comment says only what the code cannot; no TODO; never silence a check. vendor/pdf_oxide is a fork: upstream files change only between patch markers (tool/check_patch_markers.sh); src/host/ is the package's own; fork tests go where tool/check_fork_tests.sh expects.

- Write each test before its fix, and see it fail on the current code.
- Show each new test and wall can fail: break the fix on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour, stop and write that in REPORT.md instead.

## Part 1: redaction is cancellable

Redaction reads page content through `Operators` built without `with_cancel` (transform.rs:538 and the content providers), and src/redaction/ has no cancel poll, so one page of millions of operators cannot be cancelled until the page ends. The fuel wall's ("redact", "text") and ("applyRedactions", "regions") cases pass only because `REDACT_LINES` (20,000) is too small to exceed the bound.

1. Grow the redaction fuel cases until, on the current code, the work between two polls exceeds the bound (find the size; keep the probe's run time sane, say what it costs). Paste the failure.
2. Wire the op's `CancelCheck` into every `Operators` the redaction paths read (apply and the save's rewrite), the way other ops do (grep `with_cancel(` for the pattern), and any other loop in src/redaction/ that scales with one page. The fuel cases pass.
3. Check every other `Operators::new(` in vendor/pdf_oxide/src that has no `with_cancel`/`with_cancel_every` on the same chain: for each, show it is polled another way or bounded (one line each in REPORT.md), or wire the cancel and add a fuel case if none covers it.

## Part 2: the whole-buffer lint

d-122 (the memory design) promised a wall: an engine site that reads a whole stream, file or page into memory fails the build unless it is one of the kinds that are tables by nature. Build it as `tool/check_whole_buffers.sh`, modelled on check_cancel_polls.sh:

- Walks the tracked and new .rs files of vendor/pdf_oxide/src (not src/host/engine_proofs/, not `#[cfg(test)]`/`#[test]` code).
- Flags each call matching a pattern of `tool/whole_buffer_patterns.txt` (one regex per line, a `#` comment naming what it matches). Start with: `read_to_end(`, `read_to_string(`, `read_whole_raw(`, `decode_stream_data(`, `decoded_data(`, `std::fs::read(`, `fs::read(`, and any other whole-read helper you find in the engine (grep for functions returning `Vec<u8>` that read a stream body or a source whole; list them in REPORT.md and add each).
- A flagged site passes only when it is a line of `tool/whole_buffer_allow.txt`: `<path under vendor/pdf_oxide>::<fn>#<n>  # table: why`, where `table:` is the only class allowed, and the why names one of these kinds (the d-122 allow-list): font program, sampled function, ICC profile, the CMap table, DER certificate or the DSS, the xref stream, one object-stream member, a progressive JPEG raster, a JPX recompressed at full size. An allow line that matches no flagged site fails (no stale entries).
- Every current site is either allowed with its true kind, or it is a real whole read of something that is not a table: for each of those, do not allow it; list it in REPORT.md with file:line, what it reads whole, and the bounded replacement in one line. If there are any, the lint fails on them, and you stop after Part 2's tooling and report: those are bugs the assistant will plan. Do not hide one under a wrong kind.
- Wire it into `make test-guards` (or the target that runs check_cancel_polls.sh) and show it failing once on an injected `read_to_end`.
- One sentence in docs/ARCHITECTURE.md "Memory" naming the lint and its allow-list.

## Gate

The Rust suite (debug info off as above); `make test-fuel DART=dart`; `make test-memory DART=dart`; `bash tool/check_cancel_polls.sh`; `bash tool/check_whole_buffers.sh`; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `dart analyze --fatal-infos lib hook test bin`; `make test-ops-native DART=dart`; one Fixed bullet in CHANGELOG.pre.md's 6.0.0-dev.0 entry (a cancel reaches a redaction on a huge page) in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: Part 1's fuel size and failures, every `Operators::new` and how it is polled, Part 2's patterns, every flagged site with its kind or its bug line, each injected failure and its message, the summary lines, and every choice you made where the plan left one open.
- `git -C vendor/pdf_oxide add -N .` then `git -C vendor/pdf_oxide diff HEAD > t-442-fork.patch` at the repository root. Do not commit inside vendor/pdf_oxide.
- `git add -N` new package files, then `git diff HEAD -- . ':!vendor' ':!t-442-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-442-pkg.patch`. Undo your working changes in both trees so only the patches carry them.
- Commit only `t-442-fork.patch`, `t-442-pkg.patch` and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-442`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-442`, and do not clone it.
