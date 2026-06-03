#!/usr/bin/env python3
"""
Compare pdf_oxide (Rust) vs pymupdf text extraction results.

Reads results.csv from both sides, classifies differences, and generates
a comparison report + issues.csv for investigation.

Usage:
  python scripts/bench_compare.py [--output /tmp/text_comparison]

Expects:
  <output_dir>/pdf_oxide/results.csv
  <output_dir>/pymupdf/results.csv
"""

import csv
import sys
from pathlib import Path


def load_results(csv_path):
    """Load results CSV into a dict keyed by pdf_path."""
    rows = {}
    with open(csv_path) as f:
        reader = csv.DictReader(f)
        for row in reader:
            key = row["pdf_path"]
            row["chars"] = int(row["chars"])
            row["pages"] = int(row["pages"])
            row["ms"] = float(row["ms"])
            rows[key] = row
    return rows


def read_text_file(text_dir, corpus, filename):
    """Try to read extracted text file. Returns None if not found."""
    safe_name = f"{corpus}__{filename}".replace(" ", "_")
    txt_name = safe_name.rsplit(".", 1)[0] + ".txt"
    txt_path = text_dir / txt_name
    if txt_path.exists():
        try:
            return txt_path.read_text(encoding="utf-8", errors="replace")
        except Exception:
            pass
    return None


def has_garbage(text):
    """Check if text contains significant garbage (replacement chars, control chars)."""
    if not text or len(text) == 0:
        return False
    replacement = text.count("\ufffd")
    control = sum(1 for c in text if ord(c) < 32 and c not in "\n\r\t")
    total = len(text)
    return (replacement + control) / total > 0.15


def strip_control_chars(text):
    """Remove control characters (except newline/tab) that shouldn't affect comparison."""
    return "".join(c for c in text if c in "\n\r\t" or ord(c) >= 32)


def classify(oxide_row, mupdf_row, oxide_text=None, mupdf_text=None):
    """Classify the comparison result for a single PDF."""
    oxide_err = oxide_row.get("error", "")
    mupdf_err = mupdf_row.get("error", "") if mupdf_row else ""
    oxide_chars = oxide_row["chars"]
    mupdf_chars = mupdf_row["chars"] if mupdf_row else 0

    if oxide_err and mupdf_err:
        return "both_error"
    if oxide_err:
        return "oxide_error"
    if mupdf_err:
        return "mupdf_error"
    if oxide_chars == 0 and mupdf_chars == 0:
        return "both_empty"
    if oxide_chars > 0 and mupdf_chars == 0:
        return "oxide_only"
    if oxide_chars == 0 and mupdf_chars > 0:
        # If pymupdf text is mostly garbage (control chars), treat as both_empty
        if mupdf_text is not None and has_garbage(mupdf_text):
            return "both_empty"
        # Very short pymupdf text (<= 10 chars) that looks like synthesized
        # annotation text (e.g. "DRAFT", "SIGN") — treat as both_empty
        if mupdf_text is not None:
            mt_stripped = strip_control_chars(mupdf_text).strip()
            if len(mt_stripped) <= 10 and mt_stripped.isupper():
                return "both_empty"
        return "oxide_empty"

    # Text-level comparison (if text files available)
    if oxide_text is not None and mupdf_text is not None:
        ot = strip_control_chars(oxide_text).strip()
        mt = strip_control_chars(mupdf_text).strip()

        # Identical after stripping whitespace and control chars → clean
        if ot == mt:
            return "clean"

        # Same words in same order → just formatting difference → clean
        if ot.split() == mt.split():
            return "clean"

        # Same characters after removing all whitespace → clean
        # (Handles CID fonts where pymupdf outputs one-char-per-line)
        ot_nows = "".join(ot.split())
        mt_nows = "".join(mt.split())
        if ot_nows == mt_nows:
            return "clean"

        # Check if oxide contains all of mupdf's non-whitespace characters (superset)
        # This handles cases where oxide text has same content but different spacing
        ot_chars = set(ot_nows)
        mt_chars = set(mt_nows)
        if mt_chars and ot_chars >= mt_chars:
            return "clean"

        # Garbage detection
        if has_garbage(oxide_text) and has_garbage(mupdf_text):
            return "both_garbage"
        if has_garbage(oxide_text) and not has_garbage(mupdf_text):
            return "oxide_garbage"

    # Use stripped lengths for ratio comparison
    if oxide_text is not None and mupdf_text is not None:
        ot_len = len(strip_control_chars(oxide_text).strip())
        mt_len = len(strip_control_chars(mupdf_text).strip())
    else:
        ot_len = oxide_chars
        mt_len = mupdf_chars

    if mt_len == 0:
        return "clean" if ot_len == 0 else "oxide_only"

    ratio = ot_len / max(mt_len, 1)
    if ratio < 0.5:
        return "oxide_much_less"
    elif ratio < 0.8:
        return "oxide_less"
    elif ratio > 2.0:
        return "oxide_much_more"
    elif ratio > 1.2:
        return "oxide_more"
    else:
        return "clean"


CLEAN_CATS = {
    "clean",
    "both_empty",
    "oxide_only",
    "oxide_more",
    "oxide_much_more",
    "both_garbage",
    "mupdf_error",
}

CATEGORY_ORDER = [
    ("clean", "Equivalent text"),
    ("both_empty", "Both empty (image/test PDFs)"),
    ("oxide_only", "Oxide has text, mupdf empty"),
    ("oxide_more", "Oxide has more text (>1.2x)"),
    ("oxide_much_more", "Oxide has much more text (>2x)"),
    ("mupdf_error", "pymupdf error/crash"),
    ("both_garbage", "Both produce garbage"),
    ("oxide_less", "Oxide less text (0.5-0.8x)"),
    ("oxide_much_less", "Oxide much less text (<0.5x)"),
    ("oxide_empty", "Oxide empty, mupdf has text"),
    ("oxide_garbage", "Oxide garbage, mupdf ok"),
    ("oxide_error", "Oxide error/crash"),
    ("both_error", "Both error"),
    ("oxide_missing", "Missing from oxide run"),
    ("mupdf_missing", "Missing from mupdf run"),
]


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Compare pdf_oxide vs pymupdf results")
    parser.add_argument("--output", default="/tmp/text_comparison", help="Output directory")
    args = parser.parse_args()

    output_dir = Path(args.output)
    oxide_dir = output_dir / "pdf_oxide"
    mupdf_dir = output_dir / "pymupdf"

    oxide_csv = oxide_dir / "results.csv"
    mupdf_csv = mupdf_dir / "results.csv"

    if not oxide_csv.exists():
        print(f"ERROR: {oxide_csv} not found. Run bench_extract_all first.", file=sys.stderr)
        sys.exit(1)
    if not mupdf_csv.exists():
        print(f"ERROR: {mupdf_csv} not found. Run bench_pymupdf.py first.", file=sys.stderr)
        sys.exit(1)

    oxide_results = load_results(oxide_csv)
    mupdf_results = load_results(mupdf_csv)

    all_keys = sorted(set(oxide_results.keys()) | set(mupdf_results.keys()))
    print(f"Total PDFs: {len(all_keys)}")
    print(f"  pdf_oxide results: {len(oxide_results)}")
    print(f"  pymupdf results:   {len(mupdf_results)}")

    # Classify each PDF
    categories = {}
    issues = []

    for pdf_path in all_keys:
        oxide_row = oxide_results.get(pdf_path)
        mupdf_row = mupdf_results.get(pdf_path)

        row = oxide_row or mupdf_row
        corpus = row["corpus"]
        filename = row["pdf_filename"]

        if not oxide_row:
            cat = "oxide_missing"
            issues.append((corpus, filename, cat, 0, 0, "", pdf_path))
        elif not mupdf_row:
            cat = "mupdf_missing"
            issues.append((corpus, filename, cat, oxide_row["chars"], 0, "", pdf_path))
        else:
            # Try to read text files for garbage detection
            oxide_text = read_text_file(oxide_dir, corpus, filename)
            mupdf_text = read_text_file(mupdf_dir, corpus, filename)
            cat = classify(oxide_row, mupdf_row, oxide_text, mupdf_text)
            if cat not in CLEAN_CATS:
                issues.append(
                    (
                        corpus,
                        filename,
                        cat,
                        oxide_row["chars"],
                        mupdf_row["chars"],
                        oxide_row.get("error", ""),
                        pdf_path,
                    )
                )

        categories[cat] = categories.get(cat, 0) + 1

    # Summary
    total = len(all_keys)
    clean_count = sum(categories.get(c, 0) for c in CLEAN_CATS)

    print(f"\n{'=' * 70}")
    print("COMPARISON SUMMARY")
    print(f"{'=' * 70}")
    print(f"  Total PDFs:    {total}")
    print(f"  Clean:         {clean_count} ({100 * clean_count / total:.1f}%)")
    print(f"  Issues:        {total - clean_count} ({100 * (total - clean_count) / total:.1f}%)")
    print()

    for cat, label in CATEGORY_ORDER:
        count = categories.get(cat, 0)
        if count > 0:
            marker = "  " if cat in CLEAN_CATS else ">>"
            print(f"  {marker} {label:40s} {count:>5}  ({100 * count / total:.1f}%)")

    # Write issues CSV (with full pdf_path for easy investigation)
    issues_csv = output_dir / "issues.csv"
    with open(issues_csv, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(
            ["corpus", "filename", "category", "oxide_chars", "mupdf_chars", "error", "pdf_path"]
        )
        for row in sorted(issues, key=lambda r: (r[2], r[0], r[1])):
            writer.writerow(row)

    print(f"\nIssues CSV: {issues_csv}")
    print(f"  {len(issues)} PDFs with issues — review and group by category")

    # Per-category details
    print(f"\n{'=' * 70}")
    print("ISSUE DETAILS (first 5 per category)")
    print(f"{'=' * 70}")
    by_cat = {}
    for corpus, filename, cat, oc, mc, err, pp in issues:
        by_cat.setdefault(cat, []).append((corpus, filename, oc, mc, err, pp))

    for cat, label in CATEGORY_ORDER:
        if cat in CLEAN_CATS:
            continue
        entries = by_cat.get(cat, [])
        if not entries:
            continue
        print(f"\n  {label} ({len(entries)}):")
        for corpus, filename, oc, mc, err, _pp in entries[:5]:
            err_tag = f"  [{err[:40]}]" if err else ""
            print(f"    {corpus}/{filename}  oxide={oc} mupdf={mc}{err_tag}")
        if len(entries) > 5:
            print(f"    ... and {len(entries) - 5} more")


if __name__ == "__main__":
    main()
