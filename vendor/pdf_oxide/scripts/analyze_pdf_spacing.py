#!/usr/bin/env python3
"""
Analyze spacing in problematic PDF to understand what's really happening.

This script will:
1. Extract the raw content stream
2. Look for Tc/Tw operators
3. Analyze TJ arrays
4. Measure actual character positions
5. Calculate real gaps between characters

This will tell us WHY we're inserting spaces incorrectly.
"""

import fitz  # PyMuPDF


PDF_PATH = "test_datasets/pdfs/mixed/5PFVA6CO2FP66IJYJJ4YMWOLK5EHRCCD.pdf"


def analyze_content_stream(pdf_path):
    """Analyze the raw PDF content stream."""
    doc = fitz.open(pdf_path)
    page = doc[0]

    # Get raw content stream
    print("=" * 80)
    print("RAW CONTENT STREAM ANALYSIS")
    print("=" * 80)

    # Get text blocks with detailed info
    blocks = page.get_text("dict")

    print(f"\nFound {len(blocks['blocks'])} blocks")

    for block_idx, block in enumerate(blocks["blocks"][:3]):  # First 3 blocks
        if block["type"] == 0:  # Text block
            print(f"\n{'=' * 80}")
            print(f"BLOCK {block_idx}")
            print(f"{'=' * 80}")
            print(f"Bbox: {block['bbox']}")

            for line_idx, line in enumerate(block["lines"][:5]):  # First 5 lines
                print(f"\n  LINE {line_idx}:")
                print(f"  Bbox: {line['bbox']}")
                print(f"  Direction: {line['dir']}")

                for span_idx, span in enumerate(line["spans"][:10]):  # First 10 spans
                    print(f"\n    SPAN {span_idx}:")
                    print(f"    Text: '{span['text']}'")
                    print(f"    Bbox: {span['bbox']}")
                    print(f"    Font: {span['font']}")
                    print(f"    Size: {span['size']}")
                    print(f"    Origin: {span['origin']}")

                    # Calculate span width and character width
                    bbox = span["bbox"]
                    span_width = bbox[2] - bbox[0]
                    char_count = len(span["text"])

                    if char_count > 0:
                        avg_char_width = span_width / char_count
                        print(f"    Span width: {span_width:.2f}pt")
                        print(f"    Chars: {char_count}")
                        print(f"    Avg char width: {avg_char_width:.2f}pt")

                    # Calculate gap to next span
                    if span_idx < len(line["spans"]) - 1:
                        next_span = line["spans"][span_idx + 1]
                        gap = next_span["bbox"][0] - span["bbox"][2]
                        print(f"    Gap to next: {gap:.2f}pt")

                        if char_count > 0:
                            gap_ratio = gap / avg_char_width
                            print(f"    Gap ratio: {gap_ratio:.2f}x char width")

                            if gap < avg_char_width * 0.5:
                                print("    → Should MERGE (gap < 0.5x char width)")
                            else:
                                print("    → Should INSERT SPACE (gap >= 0.5x char width)")

    doc.close()


def analyze_with_rawdict(pdf_path):
    """Analyze using rawdict to get more details."""
    doc = fitz.open(pdf_path)
    page = doc[0]

    print("\n" + "=" * 80)
    print("RAWDICT ANALYSIS (Low-level)")
    print("=" * 80)

    # Get raw dictionary with detailed positioning
    raw = page.get_text("rawdict")

    for block in raw["blocks"][:2]:  # First 2 blocks
        if block["type"] == 0:
            print(f"\nBlock bbox: {block['bbox']}")
            for line in block["lines"][:3]:  # First 3 lines
                print(f"  Line: {line['bbox']}")

                spans = line["spans"]
                print(f"  {len(spans)} spans:")

                for i, span in enumerate(spans[:15]):  # First 15 spans
                    text = "".join(chr(c) if c < 128 else f"\\x{c:02x}" for c in span["chars"])
                    bbox = span["bbox"]
                    width = bbox[2] - bbox[0]
                    char_count = len(span["chars"])

                    print(f"    [{i}] '{text}' @ x={bbox[0]:.1f} w={width:.1f} chars={char_count}")

                    if i < len(spans) - 1:
                        next_span = spans[i + 1]
                        gap = next_span["bbox"][0] - bbox[2]
                        print(f"         gap={gap:.1f}pt", end="")

                        if char_count > 0:
                            avg_cw = width / char_count
                            print(f" ({gap / avg_cw:.2f}x)", end="")
                        print()

    doc.close()


def main():
    print(f"Analyzing: {PDF_PATH}")
    print()

    try:
        analyze_content_stream(PDF_PATH)
        print("\n\n")
        analyze_with_rawdict(PDF_PATH)

        print("\n" + "=" * 80)
        print("SUMMARY")
        print("=" * 80)
        print("""
Key findings to look for:
1. Are spans single characters or multiple characters?
2. What are the actual gaps between spans in points?
3. What is the average character width?
4. What ratio of gap/char_width separates words vs characters?

If we see:
- Single-char spans with gaps of ~0.1-0.3x char width → NO SPACE
- Single-char spans with gaps of ~0.5-1.0x char width → SPACE
- Multi-char spans with gaps > 0.2x char width → SPACE

This will tell us what threshold to use!
""")

    except Exception as e:
        print(f"Error: {e}")
        import traceback

        traceback.print_exc()


if __name__ == "__main__":
    main()
