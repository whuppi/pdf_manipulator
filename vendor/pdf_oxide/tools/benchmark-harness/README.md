# pdf_oxide benchmark-harness

External PDF corpus used by `cargo bench`, the `benches/*.rs`
regression suite, and the A/B rendering + extraction scripts during
release validation.

## Layout

```
tools/benchmark-harness/
├── .fixture-src/kreuzberg/     git-submodule pointing at the upstream
│                               Goldziher/kreuzberg test corpus
├── fixtures/kreuzberg/         symlinks into .fixture-src + a .skip file
│                               for known-bad fixtures
└── validate_fixtures.sh        header-check + skip-list enforcement
```

Adding a new corpus goes under `fixtures/<name>/`; convention is a
symlink tree into a read-only source (submodule, download, etc.) so
the real bytes never live in this repo.

## Fixture hygiene

Some upstream PDFs are not actually PDFs:

- AT&T ISP DNS hijacking can save an HTML "search suggestion" page
  under a `.pdf` filename if DNS fails during download.
- Joomla sites with the `akeeba` component can wrap a PDF in an HTML
  `<div>` before serving.
- Intentional polyglots (PoC‖GTFO zine) place `%PDF-` deep in the byte
  stream after a PostScript header. Valid PDF structurally, but any
  "does-the-file-start-with-%PDF-" tool trips.

The `validate_fixtures.sh` script scans every `.pdf` under `fixtures/`
and flags anything that doesn't start with `%PDF-`. Known-bad files
are listed per-directory in `.skip` (one filename per line). Filing
new bad fixtures goes there, with a comment explaining why.

### Usage

```bash
# Informational scan — always exits 0
./tools/benchmark-harness/validate_fixtures.sh

# CI-gating mode — exits 1 if any unexpected bad fixture appears
./tools/benchmark-harness/validate_fixtures.sh --strict
```

Run `--strict` in CI to catch new corruption as it enters the corpus.

## Consumers

- `benches/streaming_table_scaling.rs` — the #393 release-gate
  benchmark; synthesises its own content, does not depend on
  `fixtures/`.
- A/B regression scripts at `/tmp/regress/ab_*.sh` (unpacked from
  release-validation sessions; not committed).

## Upstream sources

- Kreuzberg: https://github.com/Goldziher/kreuzberg (MIT). 154 PDFs
  covering scanned, nougat-style, pdfa, and stress-test corpora.
  Tracked in `.fixture-src/kreuzberg/` as a read-only git submodule.
