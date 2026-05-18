# Contributing

Contributions are welcome.

---

## Setup

```bash
git clone --recursive https://github.com/whuppi/pdf_manipulator.git
cd pdf_manipulator
git config core.hooksPath .githooks
dart pub get
dart test  # build hook compiles Rust from source automatically
```

The `core.hooksPath` step activates git hooks that enforce conventional commits
and block accidental commits of gitignored files.

**Requires:** [Rust](https://rustup.rs). The build hook detects `vendor/pdf_oxide/Cargo.toml` and runs `cargo build`. No manual compilation step.

For web tests:

```bash
dart test test/web/web_smoke_test.dart -p chrome
```

---

## Before submitting a PR

```bash
dart analyze .   # zero errors, zero warnings
dart test
```

Both must pass. Don't suppress with `// ignore:` — fix the underlying issue.

---

## PR workflow

```
feature branch ──PR──► dev
                        ↓ CI runs automatically (analyze + macOS test)
                        ↓ PR title must follow Conventional Commits (feat: / fix: / etc.)
                        ↓ repo owner reviews
                        ↓ merge when green (squash-merge, PR title = commit message)
```

CI runs on every PR — no manual trigger needed. PR titles are validated by `pr-lint.yml` (required check). Use conventional commit prefixes: `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `test:`, `refactor:`, `perf:`, `build:`, `deps:`.

---

## Code style

- Match existing code in the repo
- No `dart:io` in `lib/` — the barrel must stay web-safe
- Tests in `test/` mirror the `lib/src/` structure
- Instance API: `final pdf = Pdf(); pdf.method(); pdf.dispose();`

---

## Adding a new PDF operation

End-to-end checklist:

1. **Rust** — add `#[no_mangle] pub extern "C" fn` in `vendor/pdf_oxide/src/ffi.rs`
2. **C header** — add declaration in `vendor/pdf_oxide/include/pdf_oxide_c/pdf_oxide.h`
3. **ffigen** — `dart run ffigen --config ffigen.yaml`
4. **Dart FFI wrapper** — safe wrapper in `lib/src/ffi/bindings.dart`
5. **Op enum** — add value to `lib/src/platform/_op.dart`
6. **Native dispatch** — add case in `lib/src/platform/_native.dart`
7. **Web dispatch** — add method in `lib/src/platform/_web.dart` + case in `web_assets/worker.js`
8. **Platform interface** — add method to `lib/src/platform/pdf_platform.dart`
9. **Public API** — expose via `Pdf`, `PdfEditor`, or `PdfBuilder`
10. **Tests** — add test in `test/`
11. **WASM** — if Rust changed, rebuild: `./tool/build_wasm.sh`

Full maintenance recipes (bumping upstream, editing patches, rebuilding) are in [`docs/UPDATING.md`](docs/UPDATING.md).

---

## Rust patches

The vendored fork at `vendor/pdf_oxide/` carries patches for functions not yet in upstream. Every patch has a `LOCAL PATCH` comment with a removal trigger. The full inventory is in [`docs/UPDATING.md`](docs/UPDATING.md).

---

## Releases

Contributors don't need to worry about releases. Versioning, changelog, tagging, and publishing are fully automated via release-please. When conventional commits land on `dev` or `prod`, release-please opens a Release PR that bumps the version and writes the changelog. Merging that PR creates a tag, which triggers the full pipeline: 6-platform test → compile → GitHub Release → pub.dev publish. Details in [`docs/UPDATING.md`](docs/UPDATING.md).
