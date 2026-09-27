# Cross-language performance benchmark

A tiny harness that runs the same four operations — `Open`, `ExtractText`
(page 0), `ExtractText` (all pages), `SearchAll` — against each language
binding and emits one NDJSON line per fixture, so the numbers are directly
comparable.

All four runners report **UTF-8 byte count** for `textLen`, regardless of the
host language's native string encoding, so a consistency check is trivial:

```bash
target/release/rust_bench bench_fixtures/*.pdf | jq -r '[.fixture,.textLen]|@tsv'
target/go_bench               bench_fixtures/*.pdf | jq -r '[.fixture,.textLen]|@tsv'
csharp/PdfOxide.Bench/bin/Release/net8.0/csharp_bench bench_fixtures/*.pdf | jq -r '[.fixture,.textLen]|@tsv'
node js/tests/bench.mjs       bench_fixtures/*.pdf | jq -r '[.fixture,.textLen]|@tsv'
```

All four columns should match; divergence means an extraction bug.

## Setup

```bash
# 1. Build the Rust cdylib — all bindings load it at runtime
cargo build --release --lib -p pdf_oxide

# 2. Stage fixture PDFs (not in git — they're large)
mkdir -p bench_fixtures
cp ~/projects/pdf_oxide_tests/pdfs/mixed/HPXULDFI3DAZ3V2NZOHYUGUY5SLS4AHU.pdf bench_fixtures/tiny.pdf
cp ~/projects/pdf_oxide_tests/pdfs/academic/arxiv_2510.24054v1.pdf            bench_fixtures/small.pdf
cp ~/projects/pdf_oxide_tests/pdfs/academic/arxiv_2510.25591v1.pdf            bench_fixtures/medium.pdf
cp ~/projects/pdf_oxide_tests/pdfs/academic/arxiv_2510.25507v1.pdf            bench_fixtures/large.pdf

# 3. Stage the cdylib where each binding expects it
mkdir -p go/lib/linux_amd64 lib
cp target/release/libpdf_oxide.so go/lib/linux_amd64/
cp target/release/libpdf_oxide.so lib/
```

## Build each runner

```bash
# Rust
cargo build --release --bin rust_bench -p pdf_oxide

# Go
cd go && go build -o ../target/go_bench ./cmd/bench && cd ..

# C#
dotnet build csharp/PdfOxide.Bench/PdfOxide.Bench.csproj -c Release

# JS (requires node-gyp + the compiled cdylib in ./lib)
cd js && npm install --ignore-scripts && npx tsc && node scripts/fix-esm-imports.js && npx node-gyp rebuild && cd ..
```

## Run

```bash
FIXTURES="bench_fixtures/tiny.pdf bench_fixtures/small.pdf bench_fixtures/medium.pdf bench_fixtures/large.pdf"
export LD_LIBRARY_PATH=$(pwd)/target/release

target/release/rust_bench                                                             $FIXTURES
target/go_bench                                                                       $FIXTURES
csharp/PdfOxide.Bench/bin/Release/net8.0/csharp_bench                                 $FIXTURES
node js/tests/bench.mjs                                                               $FIXTURES
```

Each command prints one NDJSON line per fixture with the schema:

```json
{
  "language":       "rust" | "go" | "csharp" | "js",
  "fixture":        "tiny.pdf",
  "sizeBytes":      1659,
  "openNs":         152926,
  "extractPage0Ns": 1516958,
  "extractAllNs":   1636347,
  "searchNs":       287197,
  "pageCount":      1,
  "textLen":        648
}
```

## Interpretation

`textLen` is always the UTF-8 byte count of page 0's extracted text. The
four bindings should report identical values — any mismatch is a real
extraction divergence.

On the maintainer's machine (Linux x64, 2026-04), the average `extractAll`
ratio vs the Rust baseline across `small.pdf`, `medium.pdf`, and `large.pdf`
is:

| Binding | Ratio vs Rust | Notes |
|---------|---------------|-------|
| Go      | ~1.15x        | CGo per-call overhead |
| C#      | ~0.77x        | Often faster — the native-path `Mutex<PdfDocument>` overhead doesn't apply at the FFI boundary where the handle is already exclusive |
| JS/TS   | ~1.23x        | N-API marshaling |

Re-run after any Rust FFI change to confirm no regression. A ratio worse
than ~2x on `extractAll` for a real-size fixture warrants profiling.
