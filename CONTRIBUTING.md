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

**Requires:** [Rust](https://rustup.rs), [FVM](https://fvm.app)
(`.fvmrc` pins the exact Flutter version). The build hook detects
`vendor/pdf_oxide/Cargo.toml` and runs `cargo build`. No manual
compilation step.

**Without FVM:** all Makefile commands accept `DART` and `FLUTTER`
overrides:

```bash
make check DART=dart FLUTTER=flutter
```

For web development:

```bash
make build-wasm              # compile Rust → WASM
make check                   # the full local gate (see Before submitting a PR)
```

---

## Before submitting a PR

```bash
make check
```

Runs `analyze` (format + Dart + Rust warnings) + `test-guards`
(mechanical test-suite rules) + `test` (unit + native + 3 web modes)
+ `test-example` (macOS + 3 web modes). Must pass. Don't suppress
with `// ignore:` — fix the underlying issue (`make analyze` fails
on any ignore comment).

---

## PR workflow

All PRs target `dev`. That's the only branch contributors touch.

```
your fork / feature branch ──PR──► dev
                                    ↓ CI: make analyze + make test-pkg-native
                                    ↓ PR title: Conventional Commits (feat: / fix: / etc.)
                                    ↓ squash-merge when green
                                    ↓ Full test suite via "ready-to-test" label
                                      (pkg + integration + verify across targets)
```

CI calls Makefile targets via the `make-target` orchestrator action.
Same commands locally and in CI. Capability actions handle runner
differences — no logic in workflows or Makefile.

You don't write changelog entries, bump versions, or touch `prod`.
The maintainer handles releases.

---

## Code style

- Match existing code in the repo.
- No `dart:io` in the public API barrel — must stay web-safe.
- **Shared brain, dumb edges.** Every decision lives in shared code —
  the Dart `Router` for orchestration, Rust `bridge_api.rs` for
  dispatch. The four platform adapter files (native + web lane
  adapters) and `lane_worker.js` only translate verbs into physics; a
  routing `if` in any of them fails `runtime/dumb_edges_test.dart`.
- Tests in `test/` mirror the `lib/src/` structure. Read the test
  invariants in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (the
  test-architecture section) before adding tests — foreign fixture
  diet, declared truths, semantic assertions, one charter per battery.
- `TestSource` returns views (not copies) — catches buffer-detach
  bugs in the runtime.
- Instance API shape: `final pdf = Pdf(); … await pdf.dispose();`

---

## Adding operations

Step-by-step checklists in [`docs/UPDATING.md`](docs/UPDATING.md).

---

## Vendored forks

Two git submodules at `vendor/`. Provenance and recipes in
[`docs/UPDATING.md`](docs/UPDATING.md).

**PRs to vendored forks**
([`whuppi/pdf_oxide`](https://github.com/whuppi/pdf_oxide),
[`whuppi/office_oxide`](https://github.com/whuppi/office_oxide)):
target the patches branch — never `main`, which is a clean mirror of
upstream. The current patch-branch names are listed in
[`docs/UPDATING.md`](docs/UPDATING.md).

After editing Rust in `vendor/`, commit AND push the submodule before
opening a PR.

---

## Releases

Handled by the maintainer. Details in [`docs/UPDATING.md`](docs/UPDATING.md).
