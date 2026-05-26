# Contributing

Contributions are welcome.

---

## Setup

```bash
git clone --recursive https://github.com/whuppi/pdf_manipulator.git
cd pdf_manipulator
dart pub get
dart test  # build hook compiles Rust from source automatically
```

**Requires:** [Rust](https://rustup.rs). The build hook detects `vendor/pdf_oxide/Cargo.toml` and runs `cargo build`. No manual compilation step.

For web development:

```bash
make wasm                    # compile Rust → WASM
make check                   # analyze + native + web tests
```

---

## Before submitting a PR

```bash
make check                   # analyze + native + web tests
```

Must pass. Don't suppress with `// ignore:` — fix the underlying issue.

---

## PR workflow

All PRs target `dev`. That's the only branch contributors touch.

```
your fork / feature branch ──PR──► dev
                                    ↓ CI: analyze + test
                                    ↓ PR title: Conventional Commits (feat: / fix: / etc.)
                                    ↓ squash-merge when green
```

You don't write changelog entries, bump versions, or touch `prod`. The maintainer handles releases.

---

## Code style

- Match existing code in the repo
- No `dart:io` in the public API barrel — must stay web-safe
- No FFI imports in bridge files — all engine calls go through the worker
- `worker.js` is a thin pass-through — `dispatch.rs` owns the logic
- Tests in `test/` mirror the `lib/src/` structure
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
