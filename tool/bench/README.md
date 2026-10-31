# Compression benchmark

Compares `pdf_manipulator`'s `compress` with `pdf_document` 12.0.0's `PdfCompressor.optimize` on the public files in `corpus.toml` plus three generated cases (self-merge x5, our own output re-compressed, a file saved incrementally twice). Not part of `make check`.

Run it from the package root: `make bench` (narrow it with `BENCH_ARGS="--only irs-fw9,plos-0000308 --max-pages 5"`). The first run downloads the corpus into `.corpus/` and checks each file against its SHA-256.

For each file and each of lossless, screen, ebook and printer it measures output size, wall time, page count, text-extract equality and render fidelity: every page at 100 dpi through `pdf_manipulator`'s renderer, RMSE and SSIM against the source, worst page reported. Each config's compress runs 5 times on the same input; `ms` is the fastest run and the CSV's `ms_median` the median. The row is scored on the first output, and `deterministic` says whether all 5 outputs were byte-identical. Configs that produce identical output share one candidate document and one fidelity result, and each source page is rendered once for all candidates; equal render bytes score RMSE 0 and SSIM 1 without a decode. It prints one progress line per case and the total wall time at the end.

A lossless row must show RMSE 0. It prints a markdown table and writes `out/bench.csv` and `out/bench.md`.
