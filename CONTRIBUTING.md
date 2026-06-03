# Contributing

Contributions are welcome.

---

## Setup

```bash
git clone --recursive https://github.com/whuppi/pdf_manipulator.git
cd pdf_manipulator
fvm install              # downloads the SDK version pinned in .fvmrc
fvm dart pub get
fvm dart test            # build hook compiles Rust from source automatically
```

**Requires:** [Rust](https://rustup.rs), [FVM](https://fvm.app) (`.fvmrc` pins the exact Flutter version). The build hook detects `vendor/pdf_oxide/Cargo.toml` and runs `cargo build`. No manual compilation step.

**Without FVM:** all Makefile commands accept `DART` and `FLUTTER` overrides:

```bash
make check DART=dart FLUTTER=flutter
```

For web development:

```bash
make build-wasm              # compile Rust → WASM
make check                   # analyze + native + web tests + example (all 3 web modes)
```

---

## Before submitting a PR

```bash
make check
```

Runs `analyze` + `test` (native + 3 web modes) + `test-example` (native + 3 web modes). Must pass. Don't suppress with `// ignore:` — fix the underlying issue.

---

## PR workflow

All PRs target `dev`. That's the only branch contributors touch.

```
your fork / feature branch ──PR──► dev
                                    ↓ CI: make analyze + make test-unit + make test-ops-native
                                    ↓ PR title: Conventional Commits (feat: / fix: / etc.)
                                    ↓ squash-merge when green
                                    ↓ Full 10-job test via "ready-to-test" label
                                      (4 pkg: macOS/Linux/Windows/web
                                       6 integration: macOS/Linux/Windows/Android/iOS/web)
```

CI calls Makefile targets — same commands locally and in CI. No logic lives in the CI YAML.

You don't write changelog entries, bump versions, or touch `prod`. The maintainer handles releases.

---

## Code style

- Match existing code in the repo
- No `dart:io` in the public API barrel — must stay web-safe
- No FFI imports in bridge files — all engine calls go through the worker
- `worker.js` is a thin pass-through — `bridge_api.rs` owns the dispatch logic
- Tests in `test/` mirror the `lib/src/` structure
- `TestSource` returns views (not copies) — catches buffer-detach bugs in transport
- Instance API: `final pdf = Pdf(); pdf.method(); pdf.dispose();`

---

## Adding operations

Step-by-step checklists in [`docs/UPDATING.md`](docs/UPDATING.md).

---

## Vendored forks

Two git submodules at `vendor/`. Provenance and recipes in [`docs/UPDATING.md`](docs/UPDATING.md).

After editing Rust in `vendor/`, commit AND push the submodule before opening a PR.

---

## Releases

Handled by the maintainer. Details in [`docs/UPDATING.md`](docs/UPDATING.md).
