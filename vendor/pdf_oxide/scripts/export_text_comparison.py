#!/usr/bin/env python3
"""
Export text from pdf_oxide and pymupdf for all PDFs in the benchmark corpora.

Walks veraPDF, pdfjs, and safedocs corpora, extracts text with both libraries,
saves .txt files side-by-side, and generates a comparison CSV.

Usage:
    python scripts/export_text_comparison.py [--limit N] [--output DIR]
"""

import contextlib
import csv
import gc
import multiprocessing
import os
import sys
import time
from pathlib import Path
from typing import Protocol, cast


class _MPContext(Protocol):
    """Protocol for multiprocessing context (has Process, Queue). ForkContext not in typeshed on all platforms."""

    def Process(self, *args, **kwargs): ...  # noqa: N802 (match multiprocessing API)
    def Queue(self): ...  # noqa: N802 (match multiprocessing API)


CORPORA = {
    "veraPDF": Path(os.path.expanduser("~/projects/veraPDF-corpus")),
    "pdfjs": Path(os.path.expanduser("~/projects/pdf_oxide_tests/pdfs_pdfjs")),
    "safedocs": Path(os.path.expanduser("~/projects/pdf_oxide_tests/pdfs_safedocs")),
}

PASSWORDS = ["", "owner", "user", "asdfasdf", "password", "test", "123456", "ownerpass", "userpass"]
TIMEOUT_SEC = 30

# PDFs known to be decompression bombs or cause hangs
SKIP_FILES = {"bomb_giant.pdf", "bomb.pdf"}


def find_all_pdfs():
    """Find all PDFs across all corpora."""
    results = []
    for corpus_name, corpus_path in CORPORA.items():
        if not corpus_path.exists():
            print(f"WARNING: corpus '{corpus_name}' not found at {corpus_path}", file=sys.stderr)
            continue
        for root, _dirs, files in os.walk(corpus_path):
            for fname in sorted(files):
                if fname.lower().endswith(".pdf"):
                    results.append((os.path.join(root, fname), corpus_name))
    results.sort(key=lambda x: x[0])
    return results


def _extract_pdf_oxide(pdf_path):
    """Extract text with pdf_oxide (runs in subprocess)."""
    from pdf_oxide import PdfDocument

    doc = PdfDocument(pdf_path)
    for pw in PASSWORDS:
        if pw:
            with contextlib.suppress(Exception):
                doc.authenticate(pw)
    count = doc.page_count()
    texts = []
    for i in range(count):
        texts.append(doc.extract_text(i))
    return "\n".join(texts), count


def _extract_pymupdf(pdf_path):
    """Extract text with pymupdf (runs in subprocess)."""
    import pymupdf

    doc = pymupdf.open(pdf_path)
    if doc.needs_pass:
        for pw in PASSWORDS:
            if doc.authenticate(pw):
                break
    texts = []
    count = doc.page_count
    for page in doc:
        texts.append(page.get_text())
    doc.close()
    return "\n".join(texts), count


def run_with_timeout(fn, pdf_path, timeout_sec):
    """Run extraction in a forked subprocess with timeout."""
    ctx = cast("_MPContext", multiprocessing.get_context("fork"))
    q = ctx.Queue()

    def worker():
        try:
            text, pages = fn(pdf_path)
            q.put(("ok", text, pages))
        except Exception as e:
            q.put(("error", str(e)[:200], -1))

    proc = ctx.Process(target=worker)
    proc.start()
    proc.join(timeout=timeout_sec)

    if proc.is_alive():
        proc.kill()
        proc.join(timeout=5)
        return None, f"timeout after {timeout_sec}s", -1

    try:
        status, text_or_err, pages = q.get_nowait()
        if status == "ok":
            return text_or_err, "", pages
        else:
            return None, text_or_err, pages
    except Exception:
        return None, f"worker died (exit code {proc.exitcode})", -1


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Export text from pdf_oxide and pymupdf")
    parser.add_argument("--limit", type=int, default=0, help="Max PDFs to process (0=all)")
    parser.add_argument("--output", default="/tmp/text_comparison", help="Output directory")
    parser.add_argument("--resume", action="store_true", help="Skip PDFs already in CSV")
    args = parser.parse_args()

    output_dir = Path(args.output)
    oxide_dir = output_dir / "pdf_oxide"
    mupdf_dir = output_dir / "pymupdf"
    oxide_dir.mkdir(parents=True, exist_ok=True)
    mupdf_dir.mkdir(parents=True, exist_ok=True)

    # Verify libraries
    try:
        import pdf_oxide  # noqa: F401

        print("pdf_oxide: ok")
    except ImportError:
        print("ERROR: pdf_oxide not available", file=sys.stderr)
        sys.exit(1)
    try:
        import pymupdf

        print(f"pymupdf: {pymupdf.VersionBind}")
    except ImportError:
        print("ERROR: pymupdf not available", file=sys.stderr)
        sys.exit(1)

    pdfs = find_all_pdfs()
    if args.limit > 0:
        pdfs = pdfs[: args.limit]
    total = len(pdfs)
    print(f"\nFound {total} PDFs across {len(CORPORA)} corpora\n")

    # Resume support: load already-processed paths
    csv_path = output_dir / "comparison.csv"
    done_paths = set()
    if args.resume and csv_path.exists():
        with open(csv_path) as f:
            reader = csv.reader(f)
            next(reader, None)  # skip header
            for row in reader:
                if row:
                    done_paths.add(row[0])
        print(f"Resuming: {len(done_paths)} already processed, skipping\n")

    # CSV writer
    write_header = not args.resume or not csv_path.exists() or len(done_paths) == 0
    stats = {
        "pass": 0,
        "oxide_better": 0,
        "mupdf_better": 0,
        "both_empty": 0,
        "oxide_fail": 0,
        "mupdf_fail": 0,
        "skipped": 0,
    }

    with open(csv_path, "a" if args.resume and done_paths else "w", newline="") as csv_file:
        writer = csv.writer(csv_file)
        if write_header:
            writer.writerow(
                [
                    "pdf_path",
                    "pdf_filename",
                    "corpus",
                    "pages",
                    "oxide_chars",
                    "oxide_ms",
                    "oxide_error",
                    "mupdf_chars",
                    "mupdf_ms",
                    "mupdf_error",
                    "diff_chars",
                    "ratio",
                ]
            )
            csv_file.flush()

        for idx, (pdf_path, corpus) in enumerate(pdfs, 1):
            if pdf_path in done_paths:
                stats["skipped"] += 1
                continue

            filename = os.path.basename(pdf_path)

            if filename in SKIP_FILES:
                stats["skipped"] += 1
                continue
            safe_name = f"{corpus}__{filename}".replace(" ", "_")
            txt_name = safe_name.rsplit(".", 1)[0] + ".txt"

            # Extract with pdf_oxide (with timeout)
            t0 = time.perf_counter()
            oxide_text, oxide_err, pages = run_with_timeout(
                _extract_pdf_oxide, pdf_path, TIMEOUT_SEC
            )
            oxide_ms = (time.perf_counter() - t0) * 1000
            oxide_chars = len(oxide_text) if oxide_text else 0
            if oxide_err:
                stats["oxide_fail"] += 1

            # Extract with pymupdf (with timeout)
            t0 = time.perf_counter()
            mupdf_text, mupdf_err, _ = run_with_timeout(_extract_pymupdf, pdf_path, TIMEOUT_SEC)
            mupdf_ms = (time.perf_counter() - t0) * 1000
            mupdf_chars = len(mupdf_text) if mupdf_text else 0
            if mupdf_err:
                stats["mupdf_fail"] += 1

            # Save text files
            if oxide_text:
                (oxide_dir / txt_name).write_text(oxide_text, encoding="utf-8")
            if mupdf_text:
                (mupdf_dir / txt_name).write_text(mupdf_text, encoding="utf-8")

            # Classify
            diff = oxide_chars - mupdf_chars
            ratio = (
                oxide_chars / max(mupdf_chars, 1)
                if mupdf_chars > 0
                else (999 if oxide_chars > 0 else 0)
            )

            if oxide_chars == 0 and mupdf_chars == 0:
                stats["both_empty"] += 1
            elif not oxide_err and not mupdf_err:
                stats["pass"] += 1
                if oxide_chars < mupdf_chars * 0.5:
                    stats["mupdf_better"] += 1
                elif oxide_chars > mupdf_chars * 1.5:
                    stats["oxide_better"] += 1

            writer.writerow(
                [
                    pdf_path,
                    filename,
                    corpus,
                    pages,
                    oxide_chars,
                    f"{oxide_ms:.1f}",
                    oxide_err,
                    mupdf_chars,
                    f"{mupdf_ms:.1f}",
                    mupdf_err,
                    diff,
                    f"{ratio:.3f}",
                ]
            )
            csv_file.flush()

            # Free large text strings to reduce memory for fork()
            del oxide_text, mupdf_text
            gc.collect()

            # Progress every 100 or on errors
            actual_idx = idx - stats["skipped"]
            actual_total = total - len(done_paths)
            if actual_idx % 100 == 0 or actual_idx == actual_total or oxide_err or mupdf_err:
                tag = ""
                if oxide_err:
                    tag = f" [oxide err: {oxide_err[:40]}]"
                elif mupdf_err:
                    tag = f" [mupdf err: {mupdf_err[:40]}]"
                print(
                    f"  [{actual_idx}/{actual_total}] oxide={oxide_chars:>7} mupdf={mupdf_chars:>7} {filename[:50]}{tag}"
                )

    # Summary
    print(f"\n{'=' * 70}")
    print("EXPORT COMPLETE")
    print(f"{'=' * 70}")
    print(f"  Total PDFs:        {total}")
    print(f"  Skipped (resume):  {stats['skipped']}")
    print(f"  Both extracted:    {stats['pass']}")
    print(f"  Both empty:        {stats['both_empty']}")
    print(f"  Oxide better:      {stats['oxide_better']}")
    print(f"  MuPDF better:      {stats['mupdf_better']}")
    print(f"  Oxide failures:    {stats['oxide_fail']}")
    print(f"  MuPDF failures:    {stats['mupdf_fail']}")
    print(f"\n  Output:            {output_dir}")
    print(f"  Comparison CSV:    {csv_path}")
    print(f"  pdf_oxide texts:   {oxide_dir}")
    print(f"  pymupdf texts:     {mupdf_dir}")


if __name__ == "__main__":
    main()
