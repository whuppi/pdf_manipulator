#!/usr/bin/env python3
"""Deep dive into font encoding for the problematic PDF."""

import re

import fitz  # PyMuPDF


PDF_PATH = "test_datasets/pdfs/mixed/5PFVA6CO2FP66IJYJJ4YMWOLK5EHRCCD.pdf"

doc = fitz.open(PDF_PATH)
page = doc[0]

print("=" * 80)
print("FONT ENCODING ANALYSIS")
print("=" * 80)

# Get fonts used on the page
fonts = page.get_fonts()
print(f"\nFonts on page 1: {len(fonts)}")
for i, font in enumerate(fonts):
    print(f"\n  Font #{i + 1}:")
    print(f"    Name: {font[3]}")  # Font name
    print(f"    Type: {font[1]}")  # Font type
    print(f"    Encoding: {font[2]}")  # Encoding
    print(f"    Reference: {font[0]}")  # Font reference

# Get raw content stream
content_stream = page.get_contents()
content_bytes = b"".join(content_stream) if isinstance(content_stream, list) else content_stream
content = content_bytes.decode("latin-1", errors="replace")

# Find the first text showing operation with "Fiscal"
print("\n" + "=" * 80)
print("SEARCHING FOR 'Fiscal' IN CONTENT STREAM")
print("=" * 80)


# Look for TJ arrays or Tj operations
tj_pattern = r"\[(.*?)\]\s*TJ"
tj_single_pattern = r"\((.*?)\)\s*Tj"

# Find all TJ arrays
tj_arrays = list(re.finditer(tj_pattern, content, re.DOTALL))
print(f"\nFound {len(tj_arrays)} TJ arrays")

# Show first 5 TJ arrays with their raw bytes
for i, match in enumerate(tj_arrays[:5]):
    array_content = match.group(1)
    print(f"\n--- TJ Array #{i + 1} ---")
    print("Raw content (first 200 chars):")
    print(array_content[:200])

    # Try to extract hex strings
    hex_pattern = r"<([0-9A-Fa-f]+)>"
    hex_strings = re.findall(hex_pattern, array_content)
    if hex_strings:
        print(f"\nHex strings found: {len(hex_strings)}")
        for j, hex_str in enumerate(hex_strings[:3]):
            print(f"  Hex #{j + 1}: {hex_str[:40]}...")
            # Convert hex to bytes
            try:
                byte_array = bytes.fromhex(hex_str)
                print(f"    Bytes: {list(byte_array[:20])}")
                # Try UTF-16 BE decode
                try:
                    text = byte_array.decode("utf-16-be")
                    print(f"    UTF-16 BE: '{text[:50]}'")
                except Exception:
                    pass
                # Try latin-1 decode
                try:
                    text = byte_array.decode("latin-1")
                    print(f"    Latin-1: '{text[:50]}'")
                except Exception:
                    pass
            except Exception:
                pass

    # Try to extract parenthesized strings
    paren_pattern = r"\(([^)]+)\)"
    paren_strings = re.findall(paren_pattern, array_content)
    if paren_strings:
        print(f"\nParenthesized strings: {len(paren_strings)}")
        for j, s in enumerate(paren_strings[:3]):
            print(f"  String #{j + 1}: '{s[:50]}'")

doc.close()
