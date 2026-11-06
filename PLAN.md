You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-569: a wall that proves the incremental save's refusal list complete. Approved for 6.0.0.

Why. `PdfSaveOptions` with incremental appends the edits to the original bytes, and throws `PdfIncrementalRefused` when an append cannot carry them. The decision is `DocumentEditor::incremental_refusal` (vendor/pdf_oxide/src/editor/document_editor.rs near :2617-2664), read by vendor/pdf_oxide/src/host/incremental.rs:205. The only guard that a new kind of edit is either refused or carried by the append is prose (document_editor.rs near :2615-2616 and :3522). An outside review checked every staged field of the editor (near :794-901) against the list and found none dropped today; the field added after release is the one that would be dropped silently: an incremental save would report success without the edit.

Read first: docs/ARCHITECTURE.md (the editor, incremental save), document_editor.rs: the `DocumentEditor` struct, `incremental_refusal`, what the append writes (near :2690-2740: modified objects, page props, info, form fields through the flush, `acroform_modified`), incremental.rs, the existing incremental tests (grep `incremental` under vendor/pdf_oxide/src/host/engine_proofs and test/ops for the Dart batteries), tool/check_fork_tests.sh (where a fork test may live).

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    cargo install cargo-nextest --locked (if missing)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    Rust: CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts" [-E 'test(<name>)'] (debug info off: the default fills the disk)

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code; a comment says only what the code cannot; no TODO; never silence a check. vendor/pdf_oxide is a fork: upstream files change only between patch markers (tool/check_patch_markers.sh); src/host/ is the package's own; fork tests go where tool/check_fork_tests.sh expects.

- Show each new test can fail: break the code on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. If the wall finds a kind that is dropped today, stop and report it in REPORT.md with a reproducer instead of fixing it.

## The work

1. **The field census, at compile time.** A test-only function that destructures `DocumentEditor` exhaustively (`let DocumentEditor { a, b, c } = editor;` with no `..`), so a new field fails to compile until it is classified. Each field gets one class in a table beside it: `source` (what was opened; not an edit), `cache` (derived, never written), `carried` (an edit the append writes: say which part of the append), or `refused` (an edit `incremental_refusal` names). A field that is an edit and is neither carried nor refused is a failure of this task: report it.
2. **The behaviour, per kind.** For every `carried` and `refused` class, a test stages one edit of that kind on a small fixture and saves incrementally:
   - `refused`: the save fails with the refusal, naming the kind.
   - `carried`: the save succeeds, and the incremental output, opened again, has the same result as the full save of the same edit: page count, each page's content operators (through `Operators`), the edited objects (the annotation, the field value, the info entry, the page property). Compare what the edit changed, not raw bytes.
   One test per kind, named for it, so a failure names the kind.
3. **Show both walls failing:** add a field to `DocumentEditor` (compile error from the census: paste it), and remove one entry from `incremental_refusal` (the matching kind's test fails: paste it). Restore both.

## Gate

The Rust suite (debug info off as above); `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `make test-ops-native DART=dart`. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the census table (field, class, why), each kind's test and what it compares, each injected failure and its message, anything dropped today with its reproducer, the summary lines, and every choice you made where the plan left one open.
- `git -C vendor/pdf_oxide add -N .` then `git -C vendor/pdf_oxide diff HEAD > t-569-fork.patch` at the repository root. Do not commit inside vendor/pdf_oxide.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-569-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-569-pkg.patch`. Undo your working changes in both trees so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-569`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-569`, and do not clone it.
