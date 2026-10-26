<!-- Generated file — do not edit by hand; changes here are overwritten. -->

# pdf_manipulator

> Instructions for AI coding agents working in this repository — read by Codex, Cursor, Copilot, Aider, Devin, Junie, Gemini and any tool that follows the [agents.md](https://agents.md) convention. Everything an agent needs is in this repository: this file and `./docs/`.

---

## What this tool does

**pdf_manipulator** is a cross-platform PDF manipulation package for Dart & Flutter — instance-based API (`final pdf = Pdf()`) with `DataSource` in, `DataSink` out, streaming through bounded buffers (64KB read / 256KB write). Merge, split, compress, encrypt, render, extract text, search, sign, validate, convert, stamp, build from scratch. Powered by vendored forks of pdf_oxide + office_oxide (Rust) at `vendor/`. Lane architecture: one shared Dart Router routing jobs to isolated lanes — detached Rust threads on native, Web Workers + WASM on web (3 I/O modes: JSPI, Atomics, OPFS) — every operation runs off the main thread, with per-op cancellation and instant dispose. No `dart:io` in the barrel. 5-step asset resolution: cached → download → compile → submodule → error.

The architecture, design and reference docs for this code live in `./docs/`. They are the authority on how it is shaped.

---

## Build and test commands

Run these after every code change. A failing test or analyzer error means the task is not done: never silence it with `// ignore:`, `# noqa` or `--no-verify`; fix the cause.

```bash
# Setup (needs Rust — https://rustup.rs, FVM — https://fvm.app)
git clone --recursive https://github.com/whuppi/pdf_manipulator
cd pdf_manipulator
make hooks                      # activate git hooks (once after cloning)
fvm install                     # downloads SDK pinned in .fvmrc
make check                      # analyze + test-guards + tests (native + 3 web modes) + example

# Without FVM (override SDK commands)
make check DART=dart FLUTTER=flutter

# Individual targets
make analyze                    # Dart + Rust warnings (all features)
make test-ops-native            # native FFI ops tests
make test-ops-web               # web ops tests (JSPI, Atomics, OPFS)
make test-example               # integration tests (macOS + 3 web modes)
make build-wasm                 # rebuild WASM after Rust changes
```

---

## Code style

- Match the existing code in this repository first: naming, comment density, file layout, idiom.
- A comment explains only what the code cannot say: an invariant, a constraint, a surprising choice that matters, a foot-gun. Never history, dates, doc section numbers, or the code restated in English.
- A rename sweeps every reference in one pass — code, docs, comments, tests, configs — until a search for the old name finds nothing.
- No `TODO` in place of a fix, no empty methods, no placeholder comments.

---

## Tool-specific notes

**Instance-based API.** `final pdf = Pdf()` — all methods on the instance. Every engine method returns a `PdfTask<T>` (a `Future` plus `cancel()`). `pdf.dispose()` kills every lane instantly. `pdf.edit(source)` for batch editing, `pdf.build()` for creating from scratch. I/O is `DataSource` (random-access reads) and `DataSink` (sequential writes) — the transport streams through fixed 64KB/256KB buffers regardless of file size.

**5-step build hook.** cached (hash verified) → download from GitHub Releases → compile from vendor source → init submodules + compile → error. SHA-256 hashes in `lib/src/hook/asset_hashes.dart`. Constants in `build.json`.

**Vendor submodules with patches.** `vendor/pdf_oxide/` and `vendor/office_oxide/` are git submodules with named patch branches. Full inventory in `docs/UPDATING.md`.

**Conventional commits required.** PR titles must follow `feat:` / `fix:` / `chore:` etc. Enforced by CI (`pr-checks.yml`) and local hook (`.githooks/commit-msg`).

**CI/CD via the shared `whuppi/ci` repo.** Generic capabilities, the hygiene workflows (triage/auto-close/labels/retry), pr-checks, and the SDK/lockfile upgrade PRs come from `whuppi/ci` at an exact pinned version (bumped by a grouped Dependabot PR). The native machinery stays local: rust/wasm capability actions, the compile matrix in `release.yml` (gate → discover → compile → upload → publish, run through the shared `release-tool` action), web-asset hashing in `tool/ci/release_hooks.sh`, and the binaryen/pana pins in `tool/versions.env`.

---

## Data, secrets and gitignore

`.gitignore` already covers:

- `data/.env` and every other `.env` flavor (only `.env.example`, `.env.template` and `.env.sample` are committed)
- `data/auth/` (captured tokens, cookies, OAuth credentials)
- `data/db/*.sqlite*` (full app state)
- `cookies*.json`, `*.token`, `*.pem`, `*.key`
- `output/`, `debug/`, `logs/`, `cache/`

Never commit a sensitive file even if it is somehow not ignored — tell the maintainer instead. The ignore file is a second line of defense, not the only check.

---

## Working with AI agents

- **Run the test suite before claiming a task is done.** Always.
- **Fix what you find.** A `TODO` is not a fix: fix it in this pass or tell the maintainer.
- **No backwards-compatibility shims** for code that has not shipped. Code assumes the latest schema and contracts; migrations handle old data once.
- **No refactoring "for cleanliness" without a stated reason.** Suggest it before changing surrounding code.
- **No AI co-author lines in commits.** The maintainer is the author.
- **Never force-push a protected branch** (`prod`, `main`, `dev`), and never skip pre-commit hooks.
