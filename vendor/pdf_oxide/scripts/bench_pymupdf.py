#!/usr/bin/env python3
"""
Bulk text extraction with pymupdf — walks corpus directories and extracts text from every PDF.

Outputs:
  <output_dir>/pymupdf/<corpus>__<filename>.txt  — extracted text
  <output_dir>/pymupdf/results.csv               — path, chars, ms, pages, error

Usage:
  python scripts/bench_pymupdf.py [--output /tmp/text_comparison]
"""

import csv
import os
import sys
import time
from pathlib import Path


CORPORA = {
    "veraPDF": Path(os.path.expanduser("~/projects/veraPDF-corpus")),
    "pdfjs": Path(os.path.expanduser("~/projects/pdf_oxide_tests/pdfs_pdfjs")),
    "safedocs": Path(os.path.expanduser("~/projects/pdf_oxide_tests/pdfs_safedocs")),
}

PASSWORDS = ["", "owner", "user", "asdfasdf", "password", "test", "123456", "ownerpass", "userpass"]
SKIP_FILES = {"bomb_giant.pdf", "bomb.pdf"}


def find_all_pdfs():
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


def extract_pymupdf(pdf_path):
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


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Bulk text extraction with pymupdf")
    parser.add_argument("--output", default="/tmp/text_comparison", help="Output directory")
    args = parser.parse_args()

    output_dir = Path(args.output)
    text_dir = output_dir / "pymupdf"
    text_dir.mkdir(parents=True, exist_ok=True)

    try:
        import pymupdf

        print(f"pymupdf: {pymupdf.VersionBind}")
    except ImportError:
        print("ERROR: pymupdf not available", file=sys.stderr)
        sys.exit(1)

    pdfs = find_all_pdfs()
    total = len(pdfs)
    print(f"Found {total} PDFs across {len(CORPORA)} corpora\n")

    csv_path = text_dir / "results.csv"
    processed = 0
    global_start = time.perf_counter()

    with open(csv_path, "w", newline="") as csv_file:
        writer = csv.writer(csv_file)
        writer.writerow(["pdf_path", "pdf_filename", "corpus", "pages", "chars", "ms", "error"])

        for idx, (pdf_path, corpus) in enumerate(pdfs, 1):
            filename = os.path.basename(pdf_path)
            if filename in SKIP_FILES:
                continue

            start = time.perf_counter()
            try:
                text, pages = extract_pymupdf(pdf_path)
                error = ""
            except Exception as e:
                text = ""
                pages = -1
                error = str(e)[:200]
            ms = (time.perf_counter() - start) * 1000
            chars = len(text)

            # Save text file
            safe_name = f"{corpus}__{filename}".replace(" ", "_")
            txt_name = safe_name.rsplit(".", 1)[0] + ".txt"
            if text:
                (text_dir / txt_name).write_text(text, encoding="utf-8")

            writer.writerow([pdf_path, filename, corpus, pages, chars, f"{ms:.1f}", error])
            csv_file.flush()

            processed += 1
            if processed % 100 == 0 or processed == total or error:
                tag = f" [err: {error[:40]}]" if error else ""
                print(f"  [{idx}/{total}] chars={chars:>7} {ms:.0f}ms {filename[:50]}{tag}")
    total_secs = time.perf_counter() - global_start
    print(f"\nDone: {processed} PDFs in {total_secs:.1f}s")
    print(f"Output: {text_dir}")
    print(f"CSV:    {csv_path}")


if __name__ == "__main__":
    main()
