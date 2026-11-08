#!/usr/bin/env python3
"""
Benchmark all PDF libraries against our Rust library.

Tests the top 10 Python PDF libraries + our Rust library on the PDF corpus:
1. pdf_oxide (our Rust library)
2. PyMuPDF (pymupdf)
3. pymupdf4llm
4. pdfplumber
5. pypdf
6. pdfminer.six
7. Camelot
8. tabula-py
9. pikepdf
10. borb
11. pypdfium2
12. playa-pdf

Outputs markdown files to separate directories for comparison.
"""

import argparse
import json
import sys
import time
from pathlib import Path
from typing import TypedDict


# Test if libraries are available
AVAILABLE_LIBRARIES = {}


def check_library_availability():
    """Check which libraries are installed."""
    libraries = {
        "pymupdf": "fitz",
        "pymupdf4llm": "pymupdf4llm",
        "pdfplumber": "pdfplumber",
        "pypdf": "pypdf",
        "pdfminer.six": "pdfminer",
        "pikepdf": "pikepdf",
        "borb": "borb",
        "pypdfium2": "pypdfium2",
        "playa-pdf": "playa",
    }

    for name, import_name in libraries.items():
        try:
            __import__(import_name)
            AVAILABLE_LIBRARIES[name] = True
            print(f"✓ {name} available")
        except ImportError:
            AVAILABLE_LIBRARIES[name] = False
            print(f"✗ {name} NOT installed")

    # Check for our Rust library
    try:
        import pdf_oxide  # noqa: F401

        AVAILABLE_LIBRARIES["pdf_oxide"] = True
        print("✓ pdf_oxide (Rust) available")
    except ImportError:
        AVAILABLE_LIBRARIES["pdf_oxide"] = False
        print("✗ pdf_oxide (Rust) NOT installed")

    print()


def extract_with_pdf_oxide(pdf_path, output_path):
    """Extract text with our Rust library."""
    import pdf_oxide

    doc = pdf_oxide.PdfDocument(str(pdf_path))
    markdown = doc.to_markdown_all(detect_headings=True)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


def extract_with_pymupdf(pdf_path, output_path):
    """Extract text with PyMuPDF."""
    import fitz

    doc = fitz.open(str(pdf_path))
    text_parts = []

    for page in doc:
        text = page.get_text()
        text_parts.append(text)

    markdown = "\n\n".join(text_parts)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    doc.close()
    return len(markdown)


def extract_with_pymupdf4llm(pdf_path, output_path):
    """Extract text with pymupdf4llm."""
    import pymupdf4llm

    markdown = pymupdf4llm.to_markdown(str(pdf_path))

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


def extract_with_pdfplumber(pdf_path, output_path):
    """Extract text with pdfplumber."""
    import pdfplumber

    text_parts = []
    with pdfplumber.open(str(pdf_path)) as pdf:
        for page in pdf.pages:
            text = page.extract_text()
            if text:
                text_parts.append(text)

    markdown = "\n\n".join(text_parts)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


def extract_with_pypdf(pdf_path, output_path):
    """Extract text with pypdf."""
    from pypdf import PdfReader

    reader = PdfReader(str(pdf_path))
    text_parts = []

    for page in reader.pages:
        text = page.extract_text()
        if text:
            text_parts.append(text)

    markdown = "\n\n".join(text_parts)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


def extract_with_pdfminer(pdf_path, output_path):
    """Extract text with pdfminer.six."""
    from pdfminer.high_level import extract_text

    text = extract_text(str(pdf_path))

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(text)

    return len(text)


def extract_with_playa(pdf_path, output_path):
    """Extract text with PLAYA-PDF."""
    import playa

    # PLAYA-PDF is more for PDF exploration than text extraction
    # Basic text extraction only
    with playa.open(pdf_path) as pdf:
        text = "\n\n".join(pdf.pages.map(playa.Page.extract_text))

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(text)

    return len(text)


def extract_with_pikepdf(pdf_path, output_path):
    """Extract text with pikepdf (basic extraction)."""
    import pikepdf

    # pikepdf is more for PDF manipulation than text extraction
    # Basic text extraction only
    text_parts = []

    with pikepdf.open(str(pdf_path)) as pdf:
        for _ in pdf.pages:
            # Very basic extraction - pikepdf doesn't have built-in text extraction
            text_parts.append(f"[Page {len(text_parts) + 1}]")

    markdown = "\n\n".join(text_parts)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


def extract_with_borb(pdf_path, output_path):
    """Extract text with borb."""
    from borb.pdf import PDF
    from borb.toolkit.text.simple_text_extraction import SimpleTextExtraction

    text_parts = []

    with open(pdf_path, "rb") as pdf_file:
        doc = PDF.loads(pdf_file)

        for page_num in range(len(doc.get_document_info().get_number_of_pages())):
            extractor = SimpleTextExtraction()
            doc.get_page(page_num).render_to_device(extractor)
            text = extractor.get_text()
            if text:
                text_parts.append(text)

    markdown = "\n\n".join(text_parts)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


def extract_with_pypdfium2(pdf_path, output_path):
    """Extract text with pypdfium2."""
    import pypdfium2 as pdfium

    pdf = pdfium.PdfDocument(str(pdf_path))
    text_parts = []

    for page in pdf:
        textpage = page.get_textpage()
        text = textpage.get_text_range()
        if text:
            text_parts.append(text)
        textpage.close()
        page.close()

    pdf.close()

    markdown = "\n\n".join(text_parts)

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(markdown)

    return len(markdown)


# Library extractors mapping
EXTRACTORS = {
    "pdf_oxide": extract_with_pdf_oxide,
    "pymupdf": extract_with_pymupdf,
    "pymupdf4llm": extract_with_pymupdf4llm,
    "pdfplumber": extract_with_pdfplumber,
    "pypdf": extract_with_pypdf,
    "pdfminer.six": extract_with_pdfminer,
    "pikepdf": extract_with_pikepdf,
    "borb": extract_with_borb,
    "pypdfium2": extract_with_pypdfium2,
    "playa-pdf": extract_with_playa,
}


def benchmark_library(library_name, pdf_files, output_dir):
    """Benchmark a single library on all PDFs."""
    if not AVAILABLE_LIBRARIES.get(library_name, False):
        print(f"Skipping {library_name} (not installed)")
        return None

    extractor = EXTRACTORS.get(library_name)
    if not extractor:
        print(f"No extractor for {library_name}")
        return None

    print(f"\n{'=' * 60}")
    print(f"Benchmarking: {library_name}")
    print(f"{'=' * 60}")

    class BenchmarkResults(TypedDict, total=False):
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

    results: BenchmarkResults = {
        "library": library_name,
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
            output_size = extractor(pdf_path, output_file)
            elapsed = time.time() - start_time

            results["successful"] += 1
            results["total_time"] += elapsed
            results["total_output_size"] += output_size
            results["times"].append(elapsed)

            print(
                f"  [{i}/{len(pdf_files)}] ✓ {pdf_path.name} ({elapsed:.3f}s, {output_size} bytes)"
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

    print(f"\nResults for {library_name}:")
    print(
        f"  Success: {results['successful']}/{results['total_pdfs']} ({results['success_rate']:.1f}%)"
    )
    print(f"  Total time: {results['total_time']:.2f}s")
    print(f"  Avg time/PDF: {results['avg_time'] * 1000:.1f}ms")
    print(f"  Total output: {results['total_output_size']:,} bytes")

    return results


def main():
    parser = argparse.ArgumentParser(description="Benchmark all PDF libraries")
    parser.add_argument(
        "--pdfs", default="test_datasets/pdfs", help="Directory containing PDFs to test"
    )
    parser.add_argument(
        "--output", default="test_datasets/benchmark_outputs", help="Output directory for results"
    )
    parser.add_argument(
        "--libraries", nargs="+", help="Specific libraries to test (default: all available)"
    )
    parser.add_argument("--limit", type=int, help="Limit number of PDFs to test")

    args = parser.parse_args()

    # Check library availability
    print("Checking library availability...\n")
    check_library_availability()

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

    # Create output directory
    output_base = Path(args.output)
    output_base.mkdir(parents=True, exist_ok=True)

    # Determine which libraries to test
    if args.libraries:
        libraries_to_test = args.libraries
    else:
        libraries_to_test = [lib for lib, available in AVAILABLE_LIBRARIES.items() if available]

    print(f"Testing libraries: {', '.join(libraries_to_test)}\n")

    # Benchmark each library
    all_results = []

    for library_name in libraries_to_test:
        library_output_dir = output_base / library_name
        library_output_dir.mkdir(parents=True, exist_ok=True)

        results = benchmark_library(library_name, pdf_files, library_output_dir)
        if results:
            all_results.append(results)

    # Save summary report
    summary_file = output_base / "benchmark_summary.json"
    with open(summary_file, "w") as f:
        json.dump(all_results, f, indent=2)

    print(f"\n{'=' * 60}")
    print("BENCHMARK SUMMARY")
    print(f"{'=' * 60}\n")

    # Sort by average time (fastest first)
    all_results.sort(key=lambda x: x["avg_time"])

    print(f"{'Library':<20} {'Success':<12} {'Total Time':<12} {'Avg/PDF':<12} {'Output Size':<15}")
    print(f"{'-' * 20} {'-' * 12} {'-' * 12} {'-' * 12} {'-' * 15}")

    for result in all_results:
        print(
            f"{result['library']:<20} "
            f"{result['successful']}/{result['total_pdfs']:<9} "
            f"{result['total_time']:>10.2f}s "
            f"{result['avg_time'] * 1000:>10.1f}ms "
            f"{result['total_output_size']:>13,} bytes"
        )

    print(f"\nResults saved to: {output_base}")
    print(f"Summary: {summary_file}")

    # Show relative performance
    if len(all_results) > 1:
        baseline = all_results[0]
        print(f"\n{'=' * 60}")
        print(f"RELATIVE PERFORMANCE (vs {baseline['library']})")
        print(f"{'=' * 60}\n")

        for result in all_results[1:]:
            speedup = result["avg_time"] / baseline["avg_time"]
            print(f"{result['library']:<20} {speedup:>6.1f}× slower")


if __name__ == "__main__":
    main()
