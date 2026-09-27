#!/usr/bin/env python3
"""
Test ligature extraction on all 173 arXiv papers.

Checks for:
1. Ligature corruption (fnancial, coefcient, etc.)
2. Successful ligature expansion (financial, coefficient, etc.)
3. Math symbol extraction (ρ, α, β, etc.)
"""

import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path


# Common ligature corruption patterns to check
CORRUPTION_PATTERNS = [
    (r"\bfnanci", "fnanci"),  # financial -> fnancial
    (r"\bcoefci", "coefci"),  # coefficient -> coefcient
    (r"\binfuenc", "infuenc"),  # influence -> infuence
    (r"\befcien", "efcien"),  # efficient -> efcient
    (r"\bofcia", "ofcia"),  # official -> ofcial
    (r"\bsufcien", "sufcien"),  # sufficient -> sufcient
    (r"\bdifc", "difc"),  # difficult -> difcult
]

# Correct patterns we expect to find
SUCCESS_PATTERNS = [
    (r"\bfinanci", "financi"),  # financial
    (r"\bcoeffici", "coeffici"),  # coefficient
    (r"\binfluenc", "influenc"),  # influence
    (r"\beffici", "effici"),  # efficient
    (r"\boffici", "offici"),  # official
]

# Math symbols to check
MATH_SYMBOLS = [
    ("ρ", "rho"),
    ("α", "alpha"),
    ("β", "beta"),
    ("γ", "gamma"),
    ("δ", "delta"),
    ("ε", "epsilon"),
    ("θ", "theta"),
    ("λ", "lambda"),
    ("μ", "mu"),
    ("σ", "sigma"),
    ("π", "pi"),
]


def find_arxiv_pdfs(base_dir):
    """Find all arXiv PDFs in the test dataset."""
    base_path = Path(base_dir)
    academic_dir = base_path / "test_datasets" / "pdfs" / "academic"

    if not academic_dir.exists():
        print(f"Error: {academic_dir} does not exist")
        return []

    arxiv_pdfs = sorted(academic_dir.glob("arxiv_*.pdf"))
    return arxiv_pdfs


def extract_text(pdf_path, binary_path):
    """Extract text from PDF using debug_text binary."""
    try:
        result = subprocess.run(
            [binary_path, str(pdf_path)], capture_output=True, text=True, timeout=30
        )
        return result.stdout + result.stderr
    except subprocess.TimeoutExpired:
        return None
    except Exception as e:
        print(f"Error extracting {pdf_path.name}: {e}")
        return None


def analyze_text(text, pdf_name):
    """Analyze extracted text for ligatures and math symbols."""
    if not text:
        return {
            "pdf": pdf_name,
            "success": False,
            "error": "No text extracted",
            "corruptions": [],
            "successes": [],
            "math_symbols": [],
        }

    results = {
        "pdf": pdf_name,
        "success": True,
        "corruptions": [],
        "successes": [],
        "math_symbols": [],
        "text_length": len(text),
    }

    # Check for corruption patterns
    for pattern, name in CORRUPTION_PATTERNS:
        matches = re.findall(pattern, text, re.IGNORECASE)
        if matches:
            results["corruptions"].append(
                {"pattern": name, "count": len(matches), "examples": matches[:3]}
            )

    # Check for successful patterns
    for pattern, name in SUCCESS_PATTERNS:
        matches = re.findall(pattern, text, re.IGNORECASE)
        if matches:
            results["successes"].append(
                {
                    "pattern": name,
                    "count": len(matches),
                }
            )

    # Check for math symbols
    for symbol, name in MATH_SYMBOLS:
        count = text.count(symbol)
        if count > 0:
            results["math_symbols"].append(
                {
                    "symbol": symbol,
                    "name": name,
                    "count": count,
                }
            )

    return results


def print_summary(all_results):
    """Print summary statistics."""
    total = len(all_results)
    successful = sum(1 for r in all_results if r["success"])

    print(f"\n{'=' * 80}")
    print("ARXIV LIGATURE TEST SUMMARY")
    print(f"{'=' * 80}")
    print(f"Total PDFs tested: {total}")
    print(f"Successfully extracted: {successful} ({successful / total * 100:.1f}%)")
    print(f"Failed: {total - successful}")

    # Count PDFs with corruption
    with_corruption = sum(1 for r in all_results if r.get("corruptions", []))
    print(
        f"\nPDFs with ligature corruption: {with_corruption} ({with_corruption / total * 100:.1f}%)"
    )

    # Count PDFs with successful ligatures
    with_success = sum(1 for r in all_results if r.get("successes", []))
    print(f"PDFs with correct ligatures: {with_success} ({with_success / total * 100:.1f}%)")

    # Count PDFs with math symbols
    with_math = sum(1 for r in all_results if r.get("math_symbols", []))
    print(f"PDFs with math symbols extracted: {with_math} ({with_math / total * 100:.1f}%)")

    # Most common corruption patterns
    corruption_counts = defaultdict(int)
    for r in all_results:
        for corr in r.get("corruptions", []):
            corruption_counts[corr["pattern"]] += corr["count"]

    if corruption_counts:
        print(f"\n{'=' * 80}")
        print("REMAINING CORRUPTION PATTERNS:")
        print(f"{'=' * 80}")
        for pattern, count in sorted(corruption_counts.items(), key=lambda x: -x[1]):
            print(f"  {pattern}: {count} occurrences")

    # Most common successful patterns
    success_counts = defaultdict(int)
    for r in all_results:
        for succ in r.get("successes", []):
            success_counts[succ["pattern"]] += succ["count"]

    if success_counts:
        print(f"\n{'=' * 80}")
        print("SUCCESSFUL LIGATURE PATTERNS:")
        print(f"{'=' * 80}")
        for pattern, count in sorted(success_counts.items(), key=lambda x: -x[1]):
            print(f"  {pattern}: {count} occurrences")

    # Math symbol statistics
    math_counts = defaultdict(int)
    for r in all_results:
        for symbol in r.get("math_symbols", []):
            math_counts[symbol["name"]] += symbol["count"]

    if math_counts:
        print(f"\n{'=' * 80}")
        print("MATH SYMBOLS EXTRACTED:")
        print(f"{'=' * 80}")
        for symbol, count in sorted(math_counts.items(), key=lambda x: -x[1]):
            print(f"  {symbol}: {count} occurrences")

    # Show worst offenders (most corruption)
    worst = sorted(
        [r for r in all_results if r.get("corruptions", [])],
        key=lambda r: sum(c["count"] for c in r["corruptions"]),
        reverse=True,
    )[:10]

    if worst:
        print(f"\n{'=' * 80}")
        print("TOP 10 PDFs WITH MOST CORRUPTION:")
        print(f"{'=' * 80}")
        for i, r in enumerate(worst, 1):
            total_corruption = sum(c["count"] for c in r["corruptions"])
            print(f"  {i}. {r['pdf']}: {total_corruption} corruptions")
            for corr in r["corruptions"][:3]:
                print(f"     - {corr['pattern']}: {corr['count']} occurrences")


def main():
    base_dir = Path.cwd()
    binary_path = base_dir / "target" / "release" / "debug_text"

    if not binary_path.exists():
        print(f"Error: Binary not found at {binary_path}")
        print("Please run: cargo build --release --bin debug_text")
        return 1

    print("Finding arXiv PDFs...")
    arxiv_pdfs = find_arxiv_pdfs(base_dir)
    print(f"Found {len(arxiv_pdfs)} arXiv PDFs")

    if len(arxiv_pdfs) == 0:
        print("No PDFs found!")
        return 1

    all_results = []

    print(f"\nTesting ligature extraction on {len(arxiv_pdfs)} PDFs...")
    print(f"{'=' * 80}")

    for i, pdf_path in enumerate(arxiv_pdfs, 1):
        print(f"[{i}/{len(arxiv_pdfs)}] {pdf_path.name}...", end=" ", flush=True)

        text = extract_text(pdf_path, binary_path)
        results = analyze_text(text, pdf_path.name)
        all_results.append(results)

        # Quick status
        if not results["success"]:
            print("❌ FAILED")
        elif results["corruptions"]:
            print(f"⚠️  CORRUPTION ({sum(c['count'] for c in results['corruptions'])})")
        else:
            print("✅ OK")

    # Print summary
    print_summary(all_results)

    # Save detailed results
    output_file = base_dir / "arxiv_ligature_test_results.json"
    with open(output_file, "w") as f:
        json.dump(all_results, f, indent=2, ensure_ascii=False)

    print(f"\n{'=' * 80}")
    print(f"Detailed results saved to: {output_file}")
    print(f"{'=' * 80}")

    # Return exit code based on corruption rate
    total_pdfs = len(all_results)
    pdfs_with_corruption = sum(1 for r in all_results if r.get("corruptions", []))
    corruption_rate = pdfs_with_corruption / total_pdfs if total_pdfs > 0 else 0

    if corruption_rate > 0.1:  # More than 10% corruption
        print(f"\n⚠️  WARNING: Corruption rate is {corruption_rate * 100:.1f}% (threshold: 10%)")
        return 1
    else:
        print(f"\n✅ SUCCESS: Corruption rate is {corruption_rate * 100:.1f}%")
        return 0


if __name__ == "__main__":
    sys.exit(main())
