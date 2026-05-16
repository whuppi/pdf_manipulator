# Contributing

Contributions are welcome. Here's how to get started.

## Setup

```bash
git clone --recursive https://github.com/whuppi/pdf_manipulator.git
cd pdf_manipulator
dart pub get
dart test  # build hook compiles Rust from source automatically
```

**Requires:** [Rust](https://rustup.rs) (the build hook runs `cargo build` on `vendor/pdf_oxide/`).

## Workflow

1. Fork the repo, create a feature branch
2. Make changes, add tests
3. Run `dart test` and `dart analyze .` — both must pass
4. Open a PR to `dev`
5. CI runs automatically (analyze + macOS test)
6. Repo owner may trigger a full 5-platform test

## Code style

- Match existing code style
- No `dart:io` in `lib/` (the barrel must stay web-safe)
- Tests go in `test/` mirroring the `lib/src/` structure
- Instance API only: `final pdf = Pdf(); pdf.method(); pdf.kill();`

## Adding a new PDF operation

See [`docs/UPDATING.md` → S3](docs/UPDATING.md) for the end-to-end checklist (Rust → C header → ffigen → Dart wrapper → platform interface → public API → tests).

## Rust patches

The vendored fork at `vendor/pdf_oxide/` carries patches for functions not yet in upstream. Every patch has a `LOCAL PATCH` comment with a removal trigger. See [`docs/UPDATING.md`](docs/UPDATING.md) for the full inventory.
