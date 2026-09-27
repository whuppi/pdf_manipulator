#!/usr/bin/env python3
"""
Re-benchmark only pdf_oxide and merge results with existing benchmark data.

This script:
1. Backs up old pdf_oxide results
2. Cleans pdf_oxide output directory
3. Re-runs benchmark on pdf_oxide only
4. Merges new pdf_oxide results with existing other libraries' results
5. Generates comparison report
"""

import argparse
import json
import shutil
import sys
import time
from pathlib import Path
from typing import TypedDict


def backup_old_results(output_dir):
    """Backup old pdf_oxide results."""
    pdf_lib_dir = output_dir / "pdf_oxide"
    if pdf_lib_dir.exists():
        backup_dir = output_dir / f"pdf_oxide_backup_{int(time.time())}"
        print(f"Backing up old results to: {backup_dir}")
        shutil.copytree(pdf_lib_dir, backup_dir)
        return backup_dir
    return None


def clean_pdf_oxide_results(output_dir):
    """Remove only pdf_oxide results."""
    pdf_lib_dir = output_dir / "pdf_oxide"
    if pdf_lib_dir.exists():
        print("Cleaning old pdf_oxide results...")
        shutil.rmtree(pdf_lib_dir)
        pdf_lib_dir.mkdir(parents=True, exist_ok=True)
        print(f"✓ Cleaned: {pdf_lib_dir}")
    else:
        pdf_lib_dir.mkdir(parents=True, exist_ok=True)
        print(f"✓ Created: {pdf_lib_dir}")


def extract_with_pdf_oxide(pdf_path, output_path):
    """Extract text with our Rust library."""
    import pdf_oxide

    doc = pdf_oxide.PdfDocument(str(pdf_path))
    markdown = doc.to_markdown_all(detect_headings=True)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


def benchmark_pdf_oxide(pdf_files, output_dir):
    """Benchmark only pdf_oxide."""
    print(f"\n{'=' * 60}")
    print("Benchmarking: pdf_oxide (UPDATED)")
    print(f"{'=' * 60}")

    class BenchmarkResult(TypedDict, total=False):
        library: str
        total_pdfs: int
        successful: int
        failed: int
        total_time: float
        total_output_size: int
        errors: list[str]
        times: list[float]
        avg_time: float
        avg_output_size: float
        success_rate: float

    results: BenchmarkResult = {
        "library": "pdf_oxide",
        "total_pdfs": len(pdf_files),
        "successful": 0,
        "failed": 0,
        "total_time": 0.0,
        "total_output_size": 0,
        "errors": [],
        "times": [],
    }

    for i, pdf_path in enumerate(pdf_files, 1):
        output_file = output_dir / f"{pdf_path.stem}.md"

        try:
            start_time = time.time()
            output_size = extract_with_pdf_oxide(pdf_path, output_file)
            elapsed = time.time() - start_time

            results["successful"] += 1
            results["total_time"] += elapsed
            results["total_output_size"] += output_size
            results["times"].append(elapsed)

            print(
                f"  [{i}/{len(pdf_files)}] ✓ {pdf_path.name} ({elapsed:.3f}s, {output_size:,} bytes)"
            )

        except Exception as e:
            results["failed"] += 1
            error_msg = f"{pdf_path.name}: {e!s}"
            results["errors"].append(error_msg)
            print(f"  [{i}/{len(pdf_files)}] ✗ {pdf_path.name} - {str(e)[:100]}")

    # Calculate statistics
    if results["successful"] > 0:
        results["avg_time"] = results["total_time"] / results["successful"]
        results["avg_output_size"] = results["total_output_size"] / results["successful"]
        results["success_rate"] = (results["successful"] / results["total_pdfs"]) * 100
    else:
        results["avg_time"] = 0
        results["avg_output_size"] = 0
        results["success_rate"] = 0

    print("\n✅ Results for pdf_oxide (UPDATED):")
    print(
        f"  Success: {results['successful']}/{results['total_pdfs']} ({results['success_rate']:.1f}%)"
    )
    print(f"  Total time: {results['total_time']:.2f}s")
    print(f"  Avg time/PDF: {results['avg_time'] * 1000:.1f}ms")
    print(f"  Total output: {results['total_output_size']:,} bytes")

    return results


def load_existing_results(summary_file):
    """Load existing benchmark results (excluding pdf_oxide)."""
    if not summary_file.exists():
        print(f"No existing results found at {summary_file}")
        return []

    with open(summary_file) as f:
        all_results = json.load(f)

    # Filter out old pdf_oxide results
    other_results = [r for r in all_results if r["library"] != "pdf_oxide"]

    print(f"\n📊 Loaded existing results for {len(other_results)} libraries:")
    for r in other_results:
        print(
            f"  - {r['library']}: {r['successful']}/{r['total_pdfs']} PDFs, {r['avg_time'] * 1000:.1f}ms avg"
        )

    return other_results


def merge_and_save_results(new_pdf_oxide_results, other_results, output_base):
    """Merge new pdf_oxide results with existing results and save."""
    all_results = [new_pdf_oxide_results] + other_results

    # Save merged summary
    summary_file = output_base / "benchmark_summary.json"
    with open(summary_file, "w") as f:
        json.dump(all_results, f, indent=2)

    print(f"\n✅ Saved merged results to: {summary_file}")

    # Generate comparison report
    print(f"\n{'=' * 60}")
    print("UPDATED BENCHMARK COMPARISON")
    print(f"{'=' * 60}\n")

    # Sort by average time (fastest first)
    all_results.sort(key=lambda x: x["avg_time"])

    print(f"{'Library':<20} {'Success':<12} {'Total Time':<12} {'Avg/PDF':<12} {'Output Size':<15}")
    print(f"{'-' * 20} {'-' * 12} {'-' * 12} {'-' * 12} {'-' * 15}")

    for result in all_results:
        lib_name = result["library"]
        if lib_name == "pdf_oxide":
            lib_name = "pdf_oxide ⭐NEW"

        print(
            f"{lib_name:<20} "
            f"{result['successful']}/{result['total_pdfs']:<9} "
            f"{result['total_time']:>10.2f}s "
            f"{result['avg_time'] * 1000:>10.1f}ms "
            f"{result['total_output_size']:>13,} bytes"
        )

    # Show relative performance
    if len(all_results) > 1:
        baseline = all_results[0]
        print(f"\n{'=' * 60}")
        print(f"RELATIVE PERFORMANCE (vs {baseline['library']})")
        print(f"{'=' * 60}\n")

        for result in all_results[1:]:
            if result["avg_time"] > 0:
                speedup = result["avg_time"] / baseline["avg_time"]
                lib_name = result["library"]
                if lib_name == "pdf_oxide":
                    lib_name = "pdf_oxide ⭐"
                print(f"{lib_name:<20} {speedup:>6.2f}× slower")


def main():
    parser = argparse.ArgumentParser(
        description="Re-benchmark only pdf_oxide and merge with existing results"
    )
    parser.add_argument(
        "--pdfs", default="test_datasets/pdfs", help="Directory containing PDFs to test"
    )
    parser.add_argument(
        "--output", default="test_datasets/benchmark_outputs", help="Output directory for results"
    )
    parser.add_argument("--limit", type=int, help="Limit number of PDFs to test")
    parser.add_argument("--no-backup", action="store_true", help="Skip backing up old results")

    args = parser.parse_args()

    # Check if pdf_oxide is available
    try:
        import pdf_oxide  # noqa: F401

        print("✓ pdf_oxide (Rust) available\n")
    except ImportError:
        print("✗ ERROR: pdf_oxide not installed!")
        print("Please build and install: maturin develop --release")
        sys.exit(1)

    # Find all PDFs
    pdf_dir = Path(args.pdfs)
    if not pdf_dir.exists():
        print(f"Error: PDF directory not found: {pdf_dir}")
        sys.exit(1)

    pdf_files = sorted(pdf_dir.rglob("*.pdf"))
    if not pdf_files:
        print(f"Error: No PDFs found in {pdf_dir}")
        sys.exit(1)

    if args.limit:
        pdf_files = pdf_files[: args.limit]

    print(f"Found {len(pdf_files)} PDFs to test\n")

    # Setup output directory
    output_base = Path(args.output)
    output_base.mkdir(parents=True, exist_ok=True)

    # Load existing results for other libraries
    summary_file = output_base / "benchmark_summary.json"
    other_results = load_existing_results(summary_file)

    if not other_results:
        print("\n⚠️  Warning: No existing results found for other libraries.")
        print("Running benchmark on pdf_oxide only.\n")

    # Backup old pdf_oxide results (optional)
    if not args.no_backup:
        backup_dir = backup_old_results(output_base)
        if backup_dir:
            print()

    # Clean pdf_oxide results
    clean_pdf_oxide_results(output_base)

    # Benchmark pdf_oxide
    pdf_oxide_output = output_base / "pdf_oxide"
    new_results = benchmark_pdf_oxide(pdf_files, pdf_oxide_output)

    # Merge and save results
    merge_and_save_results(new_results, other_results, output_base)

    print(f"\n{'=' * 60}")
    print("✅ RE-BENCHMARK COMPLETE")
    print(f"{'=' * 60}")
    print(f"\nNew pdf_oxide results: {pdf_oxide_output}")
    print(f"Merged summary: {summary_file}")
    print("\nOther libraries were NOT re-tested (using cached results)")


if __name__ == "__main__":
    main()
