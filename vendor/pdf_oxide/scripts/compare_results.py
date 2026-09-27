#!/usr/bin/env python3
"""Compare pdf_oxide vs pymupdf4llm results - detailed human-readable analysis."""

import json
import re
from difflib import SequenceMatcher
from pathlib import Path


def extract_metadata(md_content):
    """Extract metadata from markdown header."""
    lines = md_content.split("\n")
    metadata = {}

    for line in lines[:10]:
        if line.startswith("**Category**:"):
            metadata["category"] = line.split(":", 1)[1].strip()
        elif line.startswith("**Pages**:"):
            metadata["pages"] = line.split(":", 1)[1].strip()
        elif line.startswith("**Processing Time**:"):
            metadata["time"] = line.split(":", 1)[1].strip()
        elif line.startswith("**Characters**:"):
            metadata["chars"] = line.split(":", 1)[1].strip().replace(",", "")

    return metadata


def extract_content(md_content):
    """Extract main content after metadata header."""
    parts = md_content.split("---\n\n", 1)
    if len(parts) == 2:
        return parts[1].strip()
    return md_content


def similarity_ratio(text1, text2):
    """Calculate similarity ratio between two texts."""
    return SequenceMatcher(None, text1, text2).ratio()


def analyze_structure(content):
    """Analyze markdown structure."""
    return {
        "headings": len(re.findall(r"^#+\s", content, re.MULTILINE)),
        "lists": len(re.findall(r"^\s*[-*+]\s", content, re.MULTILINE)),
        "bold": len(re.findall(r"\*\*[^*]+\*\*", content)),
        "italic": len(re.findall(r"\*[^*]+\*", content)),
        "links": len(re.findall(r"\[([^\]]+)\]\(([^)]+)\)", content)),
        "code_blocks": len(re.findall(r"```", content)) // 2,
        "tables": len(re.findall(r"^\|", content, re.MULTILINE)),
        "paragraphs": len([p for p in content.split("\n\n") if p.strip()]),
    }


def compare_files(pdf_lib_path, pymupdf_path):
    """Compare two markdown files."""
    result = {
        "filename": pdf_lib_path.name,
        "exists_both": pymupdf_path.exists(),
        "pdf_lib_size": pdf_lib_path.stat().st_size if pdf_lib_path.exists() else 0,
        "pymupdf_size": pymupdf_path.stat().st_size if pymupdf_path.exists() else 0,
    }

    if not result["exists_both"]:
        result["status"] = "missing_pymupdf"
        return result

    # Read contents
    pdf_lib_content = pdf_lib_path.read_text(encoding="utf-8", errors="ignore")
    pymupdf_content = pymupdf_path.read_text(encoding="utf-8", errors="ignore")

    # Extract metadata
    result["pdf_lib_meta"] = extract_metadata(pdf_lib_content)
    result["pymupdf_meta"] = extract_metadata(pymupdf_content)

    # Extract main content
    pdf_lib_main = extract_content(pdf_lib_content)
    pymupdf_main = extract_content(pymupdf_content)

    result["pdf_lib_chars"] = len(pdf_lib_main)
    result["pymupdf_chars"] = len(pymupdf_main)
    result["char_ratio"] = result["pdf_lib_chars"] / max(result["pymupdf_chars"], 1)

    # Similarity
    result["similarity"] = similarity_ratio(pdf_lib_main[:10000], pymupdf_main[:10000])

    # Structure analysis
    result["pdf_lib_structure"] = analyze_structure(pdf_lib_main)
    result["pymupdf_structure"] = analyze_structure(pymupdf_main)

    # Quality assessment
    if result["char_ratio"] < 0.5:
        result["quality"] = "much_less_content"
    elif result["char_ratio"] > 1.5:
        result["quality"] = "much_more_content"
    elif result["similarity"] > 0.8:
        result["quality"] = "very_similar"
    elif result["similarity"] > 0.6:
        result["quality"] = "similar"
    else:
        result["quality"] = "different"

    return result


def main():
    pdf_lib_dir = Path("benchmark_results/pdf_oxide")
    pymupdf_dir = Path("test_datasets/benchmark_outputs/pymupdf4llm")

    print("Comparing pdf_oxide vs pymupdf4llm results...")
    print("=" * 80)

    # Get all pdf_oxide files
    pdf_lib_files = sorted(pdf_lib_dir.glob("*.md"))

    print(f"\nTotal files to compare: {len(pdf_lib_files)}\n")

    results = []
    categories = {}

    for i, pdf_lib_file in enumerate(pdf_lib_files, 1):
        pymupdf_file = pymupdf_dir / pdf_lib_file.name

        result = compare_files(pdf_lib_file, pymupdf_file)
        results.append(result)

        # Categorize
        quality = result.get("quality", "unknown")
        if quality not in categories:
            categories[quality] = []
        categories[quality].append(result)

        # Progress
        if i % 50 == 0:
            print(f"Processed {i}/{len(pdf_lib_files)}...")

    print(f"\n{'=' * 80}")
    print("COMPARISON SUMMARY")
    print("=" * 80)

    # Summary by quality
    print("\n📊 Quality Distribution:\n")
    for quality, items in sorted(categories.items(), key=lambda x: len(x[1]), reverse=True):
        count = len(items)
        pct = (count / len(results)) * 100
        print(f"  {quality:25s}: {count:3d} files ({pct:5.1f}%)")

    # Detailed categories
    print(f"\n{'=' * 80}")
    print("DETAILED FINDINGS")
    print("=" * 80)

    # Files with much less content
    if "much_less_content" in categories:
        print(
            f"\n⚠️  PDF_LIBRARY HAS MUCH LESS CONTENT ({len(categories['much_less_content'])} files):\n"
        )
        for item in categories["much_less_content"][:10]:
            print(
                f"  {item['filename']:50s} | pdf_lib: {item['pdf_lib_chars']:7,}c | pymupdf: {item['pymupdf_chars']:7,}c | ratio: {item['char_ratio']:.2f}"
            )

    # Files with much more content
    if "much_more_content" in categories:
        print(
            f"\n✅ PDF_LIBRARY HAS MUCH MORE CONTENT ({len(categories['much_more_content'])} files):\n"
        )
        for item in categories["much_more_content"][:10]:
            print(
                f"  {item['filename']:50s} | pdf_lib: {item['pdf_lib_chars']:7,}c | pymupdf: {item['pymupdf_chars']:7,}c | ratio: {item['char_ratio']:.2f}"
            )

    # Very similar
    if "very_similar" in categories:
        print(f"\n✅ VERY SIMILAR OUTPUTS ({len(categories['very_similar'])} files):\n")
        for item in categories["very_similar"][:5]:
            print(
                f"  {item['filename']:50s} | similarity: {item['similarity']:.2%} | ratio: {item['char_ratio']:.2f}"
            )

    # Different outputs
    if "different" in categories:
        print(f"\n⚠️  SIGNIFICANTLY DIFFERENT ({len(categories['different'])} files):\n")
        for item in categories["different"][:10]:
            print(
                f"  {item['filename']:50s} | similarity: {item['similarity']:.2%} | ratio: {item['char_ratio']:.2f}"
            )

    # Save detailed JSON
    output_file = Path("benchmark_results/comparison_detailed.json")
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2)

    print(f"\n{'=' * 80}")
    print(f"✅ Detailed results saved to: {output_file}")
    print("=" * 80)


if __name__ == "__main__":
    main()
