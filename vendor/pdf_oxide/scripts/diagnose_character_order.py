#!/usr/bin/env python3
"""
Diagnostic script to examine character extraction order from PDF.

This script extracts characters from a PDF page and shows:
1. The order they appear in the content stream (extraction order)
2. Their spatial positions (X, Y coordinates)
3. Their correct left-to-right, top-to-bottom order

This helps diagnose column mixing issues.
"""

import pdf_oxide


def diagnose_character_order(pdf_path, page_num=0, max_chars=100):
    """
    Extract characters and analyze their order.

    Args:
        pdf_path: Path to PDF file
        page_num: Page number to analyze (0-indexed)
        max_chars: Maximum characters to display
    """
    print("=== Diagnosing Character Order ===")
    print(f"PDF: {pdf_path}")
    print(f"Page: {page_num}")
    print()

    # Extract characters
    doc = pdf_oxide.PdfDocument(pdf_path)
    chars = doc.extract_chars(page_num)

    print(f"Total characters extracted: {len(chars)}")
    print()

    # Show first N characters in EXTRACTION ORDER
    print("=" * 80)
    print("EXTRACTION ORDER (as they appear in PDF content stream):")
    print("=" * 80)
    for i, char in enumerate(chars[:max_chars]):
        # Get character properties (TextChar: .char, .bbox as (x, y, w, h), .font_size)
        c = char.char
        x, y = char.bbox[0], char.bbox[1]
        font_size = char.font_size

        # Display character with position
        display_char = c if c.isprintable() and c != " " else f"[{c!r}]"
        print(f"{i:3d}: '{display_char}' at X={x:6.1f} Y={y:6.1f} size={font_size:.1f}")

    print()

    # Sort by spatial position (Y descending, then X ascending)
    # In PDF coordinates, larger Y = higher on page
    sorted_chars = sorted(chars[:max_chars], key=lambda ch: (-ch.bbox[1], ch.bbox[0]))

    print("=" * 80)
    print("SPATIAL ORDER (top-to-bottom, left-to-right):")
    print("=" * 80)
    for i, char in enumerate(sorted_chars):
        c = char.char
        x, y = char.bbox[0], char.bbox[1]
        font_size = char.font_size

        display_char = c if c.isprintable() and c != " " else f"[{c!r}]"
        print(f"{i:3d}: '{display_char}' at X={x:6.1f} Y={y:6.1f} size={font_size:.1f}")

    print()

    # Reconstruct text in both orders
    extraction_order_text = "".join(ch.char for ch in chars[:max_chars])
    spatial_order_text = "".join(ch.char for ch in sorted_chars)

    print("=" * 80)
    print("TEXT COMPARISON:")
    print("=" * 80)
    print(f"\nExtraction order text (first {max_chars} chars):")
    print(f"{extraction_order_text[:200]}")
    print()
    print(f"Spatial order text (first {max_chars} chars):")
    print(f"{spatial_order_text[:200]}")
    print()

    # Check if they differ
    if extraction_order_text != spatial_order_text:
        print("❌ ORDER MISMATCH DETECTED!")
        print("Characters in content stream are NOT in spatial left-to-right order.")
        print("This is the ROOT CAUSE of column mixing.")

        # Find first difference
        for i, (c1, c2) in enumerate(zip(extraction_order_text, spatial_order_text, strict=True)):
            if c1 != c2:
                print(f"\nFirst difference at position {i}:")
                print(f"  Extraction order: '{c1}'")
                print(f"  Spatial order:    '{c2}'")
                break
    else:
        print("✅ Orders match - content stream is already in spatial order")

    print()

    # Analyze Y-coordinate distribution to detect columns
    print("=" * 80)
    print("Y-COORDINATE ANALYSIS:")
    print("=" * 80)

    # Group characters by Y coordinate (within 5 units tolerance)
    y_groups = {}
    for char in chars[:max_chars]:
        y = char.bbox[1]
        # Find existing group within tolerance
        found_group = False
        for y_key in y_groups:
            if abs(y - y_key) < 5.0:
                y_groups[y_key].append(char)
                found_group = True
                break
        if not found_group:
            y_groups[y] = [char]

    # Show lines with multiple X ranges (potential columns)
    print(f"\nFound {len(y_groups)} distinct Y positions (±5 units)")
    print("\nLines with wide X range (potential multi-column):")

    for y, group in sorted(y_groups.items(), key=lambda x: -x[0])[:10]:
        x_values = [ch.bbox[0] for ch in group]
        x_min, x_max = min(x_values), max(x_values)
        x_range = x_max - x_min

        # If X range is very large, likely multi-column
        if x_range > 200:
            text = "".join(ch.char for ch in sorted(group, key=lambda c: c.bbox[0]))
            print(f"  Y={y:6.1f}: X range {x_min:6.1f} - {x_max:6.1f} (width: {x_range:6.1f})")
            print(f"            Text: {text[:80]}")


if __name__ == "__main__":
    import sys

    # Default: analyze arXiv paper that has column mixing
    pdf_path = "test_datasets/pdfs/academic/arxiv_2510.21165v1.pdf"

    if len(sys.argv) > 1:
        pdf_path = sys.argv[1]

    page_num = 0
    if len(sys.argv) > 2:
        page_num = int(sys.argv[2])

    max_chars = 200
    if len(sys.argv) > 3:
        max_chars = int(sys.argv[3])

    diagnose_character_order(pdf_path, page_num, max_chars)
