#!/usr/bin/env python3
"""
Analyze PDF layout to understand column structure and gaps.

This helps us tune XY-Cut parameters for untagged PDFs.
"""

import sys


sys.path.insert(0, "/home/yfedoseev/projects/pdf_oxide")

import pdf_oxide


def analyze_layout(pdf_path):
    """Analyze the physical layout of a PDF page."""
    print(f"\n{'=' * 70}")
    print(f"Layout Analysis: {pdf_path}")
    print(f"{'=' * 70}\n")

    doc = pdf_oxide.PdfDocument(pdf_path)

    # Analyze first page
    page_num = 0
    chars = doc.extract_chars(page_num)

    if not chars:
        print("No text found!")
        return

    # Collect statistics
    xs = [c.bbox.x for c in chars]
    ys = [c.bbox.y for c in chars]
    widths = [c.bbox.width for c in chars]
    heights = [c.bbox.height for c in chars]
    font_sizes = [c.font_size for c in chars]

    print(f"Total characters: {len(chars)}")
    print("\nPage Dimensions:")
    print(f"  X range: {min(xs):.1f} to {max(xs):.1f} (width: {max(xs) - min(xs):.1f})")
    print(f"  Y range: {min(ys):.1f} to {max(ys):.1f} (height: {max(ys) - min(ys):.1f})")

    print("\nFont Statistics:")
    print(
        f"  Font sizes: min={min(font_sizes):.1f}, median={sorted(font_sizes)[len(font_sizes) // 2]:.1f}, max={max(font_sizes):.1f}"
    )
    print(
        f"  Char widths: min={min(widths):.1f}, median={sorted(widths)[len(widths) // 2]:.1f}, max={max(widths):.1f}"
    )
    print(
        f"  Char heights: min={min(heights):.1f}, median={sorted(heights)[len(heights) // 2]:.1f}, max={max(heights):.1f}"
    )

    # Analyze X distribution (for column detection)
    print(f"\n{'=' * 70}")
    print("COLUMN DETECTION ANALYSIS")
    print(f"{'=' * 70}\n")

    # Create X histogram with 100 bins
    page_width = max(xs) - min(xs)
    bin_width = page_width / 100
    bins = [0] * 100

    for x in xs:
        bin_idx = int((x - min(xs)) / bin_width)
        if bin_idx >= 100:
            bin_idx = 99
        bins[bin_idx] += 1

    # Find valleys (potential column gaps)
    avg_density = sum(bins) / len(bins)
    threshold_10pct = avg_density * 0.1
    threshold_15pct = avg_density * 0.15
    threshold_20pct = avg_density * 0.2

    print(f"Average density per bin: {avg_density:.1f} chars")
    print(f"Threshold (10%): {threshold_10pct:.1f}")
    print(f"Threshold (15%): {threshold_15pct:.1f}")
    print(f"Threshold (20%): {threshold_20pct:.1f}")

    # Find valleys
    valleys_10 = []
    valleys_15 = []
    valleys_20 = []

    for i, density in enumerate(bins):
        x_pos = min(xs) + (i + 0.5) * bin_width
        if density < threshold_10pct:
            valleys_10.append((x_pos, density))
        if density < threshold_15pct:
            valleys_15.append((x_pos, density))
        if density < threshold_20pct:
            valleys_20.append((x_pos, density))

    print("\nValleys found:")
    print(f"  With 10% threshold: {len(valleys_10)} valleys")
    print(f"  With 15% threshold: {len(valleys_15)} valleys")
    print(f"  With 20% threshold: {len(valleys_20)} valleys")

    # Find largest continuous valley (potential column gap)
    if valleys_15:
        print("\n  Likely column gaps (15% threshold):")
        # Group consecutive valleys
        gap_start = None
        gap_densities = []

        for x_pos, density in valleys_15:
            if gap_start is None:
                gap_start = x_pos
                gap_densities = [density]
            elif x_pos - valleys_15[valleys_15.index((x_pos, density)) - 1][0] < bin_width * 2:
                gap_densities.append(density)
            else:
                # End of gap
                gap_width = x_pos - gap_start
                avg_gap_density = sum(gap_densities) / len(gap_densities)
                print(
                    f"    Gap at X={gap_start:.1f}-{x_pos:.1f} (width: {gap_width:.1f}, avg density: {avg_gap_density:.1f})"
                )
                gap_start = x_pos
                gap_densities = [density]

    # Visualize X distribution
    print("\nX Distribution (simplified histogram):")
    print(
        "  "
        + "0" * 10
        + "1" * 10
        + "2" * 10
        + "3" * 10
        + "4" * 10
        + "5" * 10
        + "6" * 10
        + "7" * 10
        + "8" * 10
        + "9" * 10
    )

    # Create ASCII histogram
    max_density = max(bins) if bins else 1
    histogram_height = 20
    for row in range(histogram_height, 0, -1):
        threshold_val = (row / histogram_height) * max_density
        line = "  "
        for density in bins:
            if density >= threshold_val:
                line += "█"
            else:
                line += " "
        print(line)

    print("  " + "^" * 100)
    print(f"  0{' ' * 48}{page_width / 2:.0f}{' ' * 47}{page_width:.0f}")

    # Recommendations
    print(f"\n{'=' * 70}")
    print("RECOMMENDATIONS FOR XY-CUT")
    print(f"{'=' * 70}\n")

    median_font_size = sorted(font_sizes)[len(font_sizes) // 2]
    median_char_height = sorted(heights)[len(heights) // 2]

    print("Document properties:")
    print(f"  Median font size: {median_font_size:.1f}pt")
    print(f"  Median char height: {median_char_height:.1f}pt")
    print(f"  Page width: {page_width:.1f}pt")

    print("\nRecommended XY-Cut parameters:")
    print(f"  min_region_size: {3 * median_char_height:.1f}  (3× char height)")
    print("  valley_threshold: Use 15-20% of average (not 10%)")
    print(f"    → Absolute threshold: {threshold_15pct:.1f} chars per bin")
    print(f"  projection_bins: {int(page_width):.0f}  (1pt per bin, not width/2)")

    print("\nWhy current parameters fail:")
    print(f"  Current valley threshold (10%): {threshold_10pct:.1f}")
    print(f"  → Finds {len(valleys_10)} valleys (too few if < 1)")
    print(f"  Better threshold (15%): {threshold_15pct:.1f}")
    print(f"  → Finds {len(valleys_15)} valleys")


def main():
    pdf_path = "test_datasets/pdfs/academic/arxiv_2510.21165v1.pdf"
    analyze_layout(pdf_path)


if __name__ == "__main__":
    main()
