#!/usr/bin/env python3
"""
Benchmark our PDF library on 356 PDFs to measure real quality.

Measures:
- Success rate
- Processing time per PDF
- Output size
- Errors and failure modes
- Quality assessment (where possible)
"""

import argparse
import json
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import TypedDict


def benchmark_pdf_oxide(pdf_files, output_dir):
    """Benchmark our Rust PDF library."""
    import pdf_oxide

    class BenchmarkResult(TypedDict, total=False):
        library: str
        version: str
        timestamp: str
        total_pdfs: int
        successful: int
        failed: int
        total_time: float
        total_output_size: int
        total_chars: int
        total_pages: int
        errors: list[str]
        times: list[float]
        per_pdf_stats: list[dict]
        avg_time: float
        avg_time_per_page: float
        avg_output_size: float
        avg_chars: float
        success_rate: float
        time_percentiles: dict[str, float]

    results: BenchmarkResult = {
        "library": "pdf_oxide",
        "version": "Phase 1 + Phase 2 (Adaptive Heuristics)",
        "timestamp": datetime.now().isoformat(),
        "total_pdfs": len(pdf_files),
        "successful": 0,
        "failed": 0,
        "total_time": 0.0,
        "total_output_size": 0,
        "total_chars": 0,
        "total_pages": 0,
        "errors": [],
        "times": [],
        "per_pdf_stats": [],
    }

    print(f"\n{'=' * 70}")
    print(f"Benchmarking pdf_oxide on {len(pdf_files)} PDFs")
    print(f"{'=' * 70}\n")

    for i, pdf_path in enumerate(pdf_files, 1):
        pdf_stat = {
            "filename": pdf_path.name,
            "category": pdf_path.parent.name,
            "path": str(pdf_path.relative_to("test_datasets/pdfs")),
            "success": False,
            "time": 0.0,
            "output_size": 0,
            "page_count": 0,
            "char_count": 0,
            "error": None,
        }

        try:
            # Open PDF and get page count
            doc = pdf_oxide.PdfDocument(str(pdf_path))
            page_count = doc.page_count()
            pdf_stat["page_count"] = page_count

            # Extract text with timing
            start_time = time.time()
            markdown = doc.to_markdown_all(
                detect_headings=False
            )  # Disable heading detection for consistency
            elapsed = time.time() - start_time

            output_size = len(markdown)
            char_count = len(markdown.strip())

            pdf_stat["success"] = True
            pdf_stat["time"] = elapsed
            pdf_stat["output_size"] = output_size
            pdf_stat["char_count"] = char_count

            results["successful"] += 1
            results["total_time"] += elapsed
            results["total_output_size"] += output_size
            results["total_chars"] += char_count
            results["total_pages"] += page_count
            results["times"].append(elapsed)

            # Save output
            output_file = output_dir / f"{pdf_path.stem}.md"
            output_file.parent.mkdir(parents=True, exist_ok=True)
            with open(output_file, "w", encoding="utf-8") as f:
                f.write(f"# {pdf_path.name}\n\n")
                f.write(f"**Category**: {pdf_path.parent.name}\n")
                f.write(f"**Pages**: {page_count}\n")
                f.write(f"**Processing Time**: {elapsed:.3f}s\n")
                f.write(f"**Characters**: {char_count:,}\n\n")
                f.write("---\n\n")
                f.write(markdown)

            # Progress indicator
            progress = (i / len(pdf_files)) * 100
            time_per_pdf = elapsed * 1000  # ms
            print(
                f"[{i:3d}/{len(pdf_files)}] ({progress:5.1f}%) ✓ {pdf_path.parent.name:20s}/{pdf_path.name:40s} | {page_count:3d}p | {time_per_pdf:6.1f}ms | {char_count:7,}c"
            )

        except Exception as e:
            pdf_stat["success"] = False
            pdf_stat["error"] = str(e)

            results["failed"] += 1
            error_msg = f"{pdf_path.relative_to('test_datasets/pdfs')}: {e!s}"
            results["errors"].append(error_msg)

            print(
                f"[{i:3d}/{len(pdf_files)}] ({(i / len(pdf_files)) * 100:5.1f}%) ✗ {pdf_path.parent.name:20s}/{pdf_path.name:40s} | ERROR: {str(e)[:60]}"
            )

        results["per_pdf_stats"].append(pdf_stat)

    # Calculate statistics
    if results["successful"] > 0:
        results["avg_time"] = results["total_time"] / results["successful"]
        results["avg_time_per_page"] = (
            results["total_time"] / results["total_pages"] if results["total_pages"] > 0 else 0
        )
        results["avg_output_size"] = results["total_output_size"] / results["successful"]
        results["avg_chars"] = results["total_chars"] / results["successful"]
        results["success_rate"] = (results["successful"] / results["total_pdfs"]) * 100

        # Time percentiles
        sorted_times = sorted(results["times"])
        results["time_percentiles"] = {
            "p50": sorted_times[len(sorted_times) // 2],
            "p90": sorted_times[int(len(sorted_times) * 0.9)],
            "p95": sorted_times[int(len(sorted_times) * 0.95)],
            "p99": sorted_times[int(len(sorted_times) * 0.99)],
            "min": sorted_times[0],
            "max": sorted_times[-1],
        }
    else:
        results["avg_time"] = 0
        results["avg_time_per_page"] = 0
        results["avg_output_size"] = 0
        results["avg_chars"] = 0
        results["success_rate"] = 0
        results["time_percentiles"] = {}

    return results


def print_summary(results):
    """Print benchmark summary."""
    print(f"\n{'=' * 70}")
    print("Benchmark Results Summary")
    print(f"{'=' * 70}\n")

    print(f"Library: {results['library']}")
    print(f"Version: {results['version']}")
    print(f"Timestamp: {results['timestamp']}")
    print()

    print("📊 Success Rate:")
    print(
        f"  Successful: {results['successful']}/{results['total_pdfs']} ({results['success_rate']:.1f}%)"
    )
    print(
        f"  Failed: {results['failed']}/{results['total_pdfs']} ({100 - results['success_rate']:.1f}%)"
    )
    print()

    print("⏱️  Performance:")
    print(f"  Total time: {results['total_time']:.2f}s")
    print(f"  Avg time/PDF: {results['avg_time'] * 1000:.1f}ms")
    print(f"  Avg time/page: {results['avg_time_per_page'] * 1000:.1f}ms")

    if results["time_percentiles"]:
        print("  Time percentiles:")
        print(f"    P50 (median): {results['time_percentiles']['p50'] * 1000:.1f}ms")
        print(f"    P90: {results['time_percentiles']['p90'] * 1000:.1f}ms")
        print(f"    P95: {results['time_percentiles']['p95'] * 1000:.1f}ms")
        print(f"    P99: {results['time_percentiles']['p99'] * 1000:.1f}ms")
        print(f"    Min: {results['time_percentiles']['min'] * 1000:.1f}ms")
        print(f"    Max: {results['time_percentiles']['max'] * 1000:.1f}ms")
    print()

    print("📄 Output:")
    print(f"  Total pages: {results['total_pages']:,}")
    print(f"  Total output: {results['total_output_size']:,} bytes")
    print(f"  Total chars: {results['total_chars']:,}")
    print(f"  Avg output/PDF: {results['avg_output_size']:,.0f} bytes")
    print(f"  Avg chars/PDF: {results['avg_chars']:,.0f}")
    print()

    if results["errors"]:
        print(f"❌ Errors ({len(results['errors'])}):")
        for error in results["errors"][:10]:  # Show first 10
            print(f"  - {error}")
        if len(results["errors"]) > 10:
            print(f"  ... and {len(results['errors']) - 10} more")
        print()

    print(f"{'=' * 70}\n")


def analyze_failures(results):
    """Analyze failure patterns."""
    print(f"\n{'=' * 70}")
    print("Failure Analysis")
    print(f"{'=' * 70}\n")

    if results["failed"] == 0:
        print("✅ No failures!")
        return

    # Group errors by category
    error_categories = {}
    for stat in results["per_pdf_stats"]:
        if not stat["success"] and stat["error"]:
            category = stat["category"]
            if category not in error_categories:
                error_categories[category] = []
            error_categories[category].append({"file": stat["filename"], "error": stat["error"]})

    print("Failures by category:")
    for category, errors in sorted(error_categories.items()):
        print(f"  {category}: {len(errors)} failures")
        for err in errors[:3]:  # Show first 3
            print(f"    - {err['file']}: {err['error'][:80]}")
        if len(errors) > 3:
            print(f"    ... and {len(errors) - 3} more")
    print()


def main():
    parser = argparse.ArgumentParser(description="Benchmark pdf_oxide on PDF corpus")
    parser.add_argument(
        "--pdfs",
        default="test_datasets/pdfs",
        help="Directory containing PDFs to test (default: test_datasets/pdfs)",
    )
    parser.add_argument(
        "--output",
        default="benchmark_results",
        help="Output directory for results (default: benchmark_results)",
    )
    parser.add_argument("--limit", type=int, help="Limit number of PDFs to test (default: all)")
    parser.add_argument("--save-json", action="store_true", help="Save detailed JSON results")

    args = parser.parse_args()

    # Check if library is available
    try:
        import pdf_oxide  # noqa: F401

        print("✓ pdf_oxide available")
    except ImportError:
        print("✗ pdf_oxide NOT installed")
        print("\nPlease install with: maturin develop --release")
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
        print(f"Limiting to first {args.limit} PDFs")

    print(f"Found {len(pdf_files)} PDFs to test")

    # Create output directory
    output_dir = Path(args.output) / "pdf_oxide"
    output_dir.mkdir(parents=True, exist_ok=True)

    # Run benchmark
    results = benchmark_pdf_oxide(pdf_files, output_dir)

    # Print summary
    print_summary(results)

    # Analyze failures
    analyze_failures(results)

    # Save JSON results
    if args.save_json:
        json_file = Path(args.output) / "pdf_oxide_benchmark.json"
        with open(json_file, "w") as f:
            json.dump(results, f, indent=2)
        print(f"✓ Detailed results saved to: {json_file}")

    # Save summary
    summary_file = Path(args.output) / "pdf_oxide_summary.txt"
    with open(summary_file, "w") as f:
        f.write("PDF Library Benchmark Results\n")
        f.write(f"{'=' * 70}\n\n")
        f.write(f"Timestamp: {results['timestamp']}\n")
        f.write(f"Version: {results['version']}\n\n")
        f.write(
            f"Success: {results['successful']}/{results['total_pdfs']} ({results['success_rate']:.1f}%)\n"
        )
        f.write(f"Failed: {results['failed']}\n")
        f.write(f"Total time: {results['total_time']:.2f}s\n")
        f.write(f"Avg time/PDF: {results['avg_time'] * 1000:.1f}ms\n")
        f.write(f"Avg time/page: {results['avg_time_per_page'] * 1000:.1f}ms\n")
        f.write(f"Total pages: {results['total_pages']:,}\n")
        f.write(f"Total output: {results['total_chars']:,} chars\n")
    print(f"✓ Summary saved to: {summary_file}\n")


if __name__ == "__main__":
    main()
