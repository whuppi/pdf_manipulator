#!/usr/bin/env python3
"""
Inspect the raw content stream of the problematic PDF to see what's in the TJ arrays.
"""

import re

import fitz  # PyMuPDF


PDF_PATH = "test_datasets/pdfs/mixed/5PFVA6CO2FP66IJYJJ4YMWOLK5EHRCCD.pdf"

doc = fitz.open(PDF_PATH)
page = doc[0]

# Get raw content stream
content_stream = page.get_contents()

# Handle list of content streams
if isinstance(content_stream, list):
    # Concatenate all streams
    content_bytes = b"".join(content_stream)
elif isinstance(content_stream, bytes):
    content_bytes = content_stream
else:
    content_bytes = str(content_stream).encode("latin-1")

# Decode
try:
    content = content_bytes.decode("latin-1")
except Exception:
    content = str(content_bytes)

print("=" * 80)
print("SEARCHING FOR TJ ARRAYS IN CONTENT STREAM")
print("=" * 80)

# Find TJ arrays - pattern: [...] TJ
tj_pattern = r"\[(.*?)\]\s*TJ"
matches = re.findall(tj_pattern, content, re.DOTALL)

print(f"\nFound {len(matches)} TJ arrays\n")

# Show first 10 TJ arrays
for i, match in enumerate(matches[:10]):
    print(f"TJ Array #{i + 1}:")
    print(f"  Content: [{match[:200]}{'...' if len(match) > 200 else ''}]")

    # Try to parse elements
    # Simple heuristic: strings in () and numbers
    strings = re.findall(r"\(([^)]*)\)", match)
    numbers = re.findall(r"(?<!\()\s(-?\d+\.?\d*)\s", match)

    print(f"  Strings: {len(strings)}")
    if strings:
        print(f"  First 5 strings: {strings[:5]}")
    print(f"  Numbers: {len(numbers)}")
    if numbers:
        print(f"  First 5 numbers: {numbers[:5]}")
    print()

doc.close()
