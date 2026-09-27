#!/usr/bin/env python3
"""
Comprehensive comparison of all PDF exports between our library and PyMuPDF4LLM.

Analyzes 103 PDFs across multiple dimensions:
- File sizes
- Content features (<br> tags, bold markers, form fields, etc.)
- Text quality (garbled text detection)
- Table structure
- Performance metrics
"""

import json
import re
import statistics
from collections import defaultdict
from pathlib import Path


class OutputComparator:
    def __init__(self, our_dir, pymupdf_dir):
        self.our_dir = Path(our_dir)
        self.pymupdf_dir = Path(pymupdf_dir)
        self.results = []
        self.summary = {"by_category": defaultdict(list)}

    def analyze_file(self, our_file, pymupdf_file):
        """Analyze a single pair of files."""
        try:
            with open(our_file, encoding="utf-8") as f:
                our_content = f.read()
        except Exception:
            our_content = ""

        try:
            with open(pymupdf_file, encoding="utf-8") as f:
                pymupdf_content = f.read()
        except Exception:
            pymupdf_content = ""

        # Basic metrics
        our_size = len(our_content)
        pymupdf_size = len(pymupdf_content)
        size_ratio = our_size / pymupdf_size if pymupdf_size > 0 else 0

        # Count features
        our_br_tags = our_content.count("<br>")
        pymupdf_br_tags = pymupdf_content.count("<br>")

        our_bold = len(re.findall(r"\*\*", our_content))
        pymupdf_bold = len(re.findall(r"\*\*", pymupdf_content))

        our_rules = our_content.count("---")
        pymupdf_rules = pymupdf_content.count("---")

        our_forms = "Form Fields" in our_content
        pymupdf_forms = "Form Fields" in pymupdf_content

        # Table metrics
        our_table_rows = our_content.count("\n|")
        pymupdf_table_rows = pymupdf_content.count("\n|")

        # Detect garbled text (heuristic: unusual character sequences)
        garbled_pattern = r"[a-z]{3}[A-Z][a-z]{4}[a-z]{2}[A-Z]"
        our_garbled = len(re.findall(garbled_pattern, our_content))
        pymupdf_garbled = len(re.findall(garbled_pattern, pymupdf_content))

        # Get category from path
        category = our_file.parent.name

        result = {
            "file": our_file.name,
            "category": category,
            "our_size": our_size,
            "pymupdf_size": pymupdf_size,
            "size_ratio": size_ratio,
            "size_diff": our_size - pymupdf_size,
            "our_br_tags": our_br_tags,
            "pymupdf_br_tags": pymupdf_br_tags,
            "our_bold": our_bold,
            "pymupdf_bold": pymupdf_bold,
            "our_rules": our_rules,
            "pymupdf_rules": pymupdf_rules,
            "our_forms": our_forms,
            "pymupdf_forms": pymupdf_forms,
            "our_table_rows": our_table_rows,
            "pymupdf_table_rows": pymupdf_table_rows,
            "our_garbled": our_garbled,
            "pymupdf_garbled": pymupdf_garbled,
        }

        return result

    def find_matching_files(self):
        """Find all matching file pairs."""
        our_files = list(self.our_dir.rglob("*.md"))
        pairs = []

        for our_file in our_files:
            # Get relative path from our_dir
            rel_path = our_file.relative_to(self.our_dir)
            pymupdf_file = self.pymupdf_dir / rel_path

            if pymupdf_file.exists():
                pairs.append((our_file, pymupdf_file))

        return pairs

    def run_comparison(self):
        """Run comparison on all file pairs."""
        pairs = self.find_matching_files()
        print(f"Found {len(pairs)} matching file pairs")

        for i, (our_file, pymupdf_file) in enumerate(pairs, 1):
            if i % 10 == 0:
                print(f"Processing {i}/{len(pairs)}...")

            result = self.analyze_file(our_file, pymupdf_file)
            self.results.append(result)

            # Aggregate by category
            cat = result["category"]
            self.summary["by_category"][cat].append(result)

        print(f"Completed analyzing {len(self.results)} files")

    def generate_statistics(self):
        """Generate statistical summary."""
        if not self.results:
            return {}

        stats = {
            "total_files": len(self.results),
            "size_ratios": [r["size_ratio"] for r in self.results],
            "avg_size_ratio": statistics.mean([r["size_ratio"] for r in self.results]),
            "median_size_ratio": statistics.median([r["size_ratio"] for r in self.results]),
            "our_total_size": sum(r["our_size"] for r in self.results),
            "pymupdf_total_size": sum(r["pymupdf_size"] for r in self.results),
            "our_avg_size": statistics.mean([r["our_size"] for r in self.results]),
            "pymupdf_avg_size": statistics.mean([r["pymupdf_size"] for r in self.results]),
            "files_with_forms_ours": sum(1 for r in self.results if r["our_forms"]),
            "files_with_forms_pymupdf": sum(1 for r in self.results if r["pymupdf_forms"]),
            "total_br_tags_ours": sum(r["our_br_tags"] for r in self.results),
            "total_br_tags_pymupdf": sum(r["pymupdf_br_tags"] for r in self.results),
            "total_bold_ours": sum(r["our_bold"] for r in self.results),
            "total_bold_pymupdf": sum(r["pymupdf_bold"] for r in self.results),
            "files_with_garbled_text_ours": sum(1 for r in self.results if r["our_garbled"] > 0),
            "files_with_garbled_text_pymupdf": sum(
                1 for r in self.results if r["pymupdf_garbled"] > 0
            ),
        }

        # Size categories
        stats["size_match_excellent"] = sum(
            1 for r in self.results if 0.95 <= r["size_ratio"] <= 1.05
        )
        stats["size_match_good"] = sum(1 for r in self.results if 0.90 <= r["size_ratio"] <= 1.10)
        stats["size_smaller"] = sum(1 for r in self.results if r["size_ratio"] < 0.90)
        stats["size_larger"] = sum(1 for r in self.results if r["size_ratio"] > 1.10)

        # Category breakdown
        stats["by_category"] = {}
        categories = {r["category"] for r in self.results}
        for cat in categories:
            cat_results = [r for r in self.results if r["category"] == cat]
            stats["by_category"][cat] = {
                "count": len(cat_results),
                "avg_size_ratio": statistics.mean([r["size_ratio"] for r in cat_results]),
                "our_avg_size": statistics.mean([r["our_size"] for r in cat_results]),
                "pymupdf_avg_size": statistics.mean([r["pymupdf_size"] for r in cat_results]),
                "with_forms": sum(1 for r in cat_results if r["our_forms"]),
            }

        return stats

    def find_outliers(self):
        """Find files with interesting characteristics."""
        outliers = {
            "largest_our": sorted(self.results, key=lambda x: x["our_size"], reverse=True)[:5],
            "largest_pymupdf": sorted(self.results, key=lambda x: x["pymupdf_size"], reverse=True)[
                :5
            ],
            "most_br_tags_ours": sorted(self.results, key=lambda x: x["our_br_tags"], reverse=True)[
                :5
            ],
            "most_br_tags_pymupdf": sorted(
                self.results, key=lambda x: x["pymupdf_br_tags"], reverse=True
            )[:5],
            "most_bold_ours": sorted(self.results, key=lambda x: x["our_bold"], reverse=True)[:5],
            "most_bold_pymupdf": sorted(
                self.results, key=lambda x: x["pymupdf_bold"], reverse=True
            )[:5],
            "most_garbled_ours": sorted(self.results, key=lambda x: x["our_garbled"], reverse=True)[
                :5
            ],
            "size_ratio_closest_to_1": sorted(
                self.results, key=lambda x: abs(1.0 - x["size_ratio"])
            )[:10],
            "size_ratio_furthest_from_1": sorted(
                self.results, key=lambda x: abs(1.0 - x["size_ratio"]), reverse=True
            )[:10],
        }
        return outliers

    def save_results(self, output_file):
        """Save detailed results to JSON."""
        data = {
            "results": self.results,
            "statistics": self.generate_statistics(),
            "outliers": self.find_outliers(),
        }

        with open(output_file, "w") as f:
            json.dump(data, f, indent=2)

        print(f"Saved results to {output_file}")

    def print_summary(self):
        """Print human-readable summary."""
        stats = self.generate_statistics()

        print("\n" + "=" * 70)
        print("COMPREHENSIVE COMPARISON SUMMARY")
        print("=" * 70)
        print(f"\nTotal files analyzed: {stats['total_files']}")
        print("\nSize Analysis:")
        print(f"  Average size ratio (ours/pymupdf): {stats['avg_size_ratio']:.3f}")
        print(f"  Median size ratio: {stats['median_size_ratio']:.3f}")
        print(f"  Files with excellent size match (95-105%): {stats['size_match_excellent']}")
        print(f"  Files with good size match (90-110%): {stats['size_match_good']}")
        print(f"  Files significantly smaller: {stats['size_smaller']}")
        print(f"  Files significantly larger: {stats['size_larger']}")

        print("\nTotal Output Sizes:")
        print(
            f"  Our library: {stats['our_total_size']:,} bytes ({stats['our_total_size'] / 1024 / 1024:.2f} MB)"
        )
        print(
            f"  PyMuPDF4LLM: {stats['pymupdf_total_size']:,} bytes ({stats['pymupdf_total_size'] / 1024 / 1024:.2f} MB)"
        )
        print(f"  Difference: {stats['our_total_size'] - stats['pymupdf_total_size']:,} bytes")

        print("\nFeature Comparison:")
        print("  <br> tags:")
        print(f"    Our library: {stats['total_br_tags_ours']:,}")
        print(f"    PyMuPDF4LLM: {stats['total_br_tags_pymupdf']:,}")
        print("  Bold markers:")
        print(f"    Our library: {stats['total_bold_ours']:,}")
        print(f"    PyMuPDF4LLM: {stats['total_bold_pymupdf']:,}")
        print(f"    Ratio: 1:{stats['total_bold_pymupdf'] / max(stats['total_bold_ours'], 1):.1f}")
        print("  Form fields:")
        print(f"    Our library: {stats['files_with_forms_ours']} files")
        print(f"    PyMuPDF4LLM: {stats['files_with_forms_pymupdf']} files")

        print("\nText Quality:")
        print("  Files with potential garbled text:")
        print(f"    Our library: {stats['files_with_garbled_text_ours']}")
        print(f"    PyMuPDF4LLM: {stats['files_with_garbled_text_pymupdf']}")

        print("\nBy Category:")
        for cat, cat_stats in sorted(stats["by_category"].items()):
            print(f"  {cat}:")
            print(f"    Files: {cat_stats['count']}")
            print(f"    Avg size ratio: {cat_stats['avg_size_ratio']:.3f}")
            print(f"    With forms: {cat_stats['with_forms']}")

        print("\n" + "=" * 70)


def main():
    our_dir = "markdown_exports/our_library_aggressive"
    pymupdf_dir = "markdown_exports/pymupdf4llm"
    output_file = "markdown_exports/comprehensive_comparison.json"

    comparator = OutputComparator(our_dir, pymupdf_dir)
    comparator.run_comparison()
    comparator.print_summary()
    comparator.save_results(output_file)

    print(f"\nDetailed results saved to: {output_file}")


if __name__ == "__main__":
    main()
