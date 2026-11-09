"""
Minimal smoke test run inside Docker containers to verify the wheel installs
and the core C extension loads correctly on each target distro/libc.

Checks:
  - import succeeds (catches missing symbols like __memcmpeq on glibc<2.35)
  - version() returns a valid PDF version tuple
  - page_count() returns a positive integer
  - extract_text(0) returns a string (empty is OK for the minimal test fixture)
"""

import sys

import pdf_oxide

doc = pdf_oxide.PdfDocument("/fixtures/simple.pdf")

major, minor = doc.version()
assert isinstance(major, int) and major >= 1, f"bad version: {major}.{minor}"

pages = doc.page_count()
assert isinstance(pages, int) and pages >= 1, f"bad page_count: {pages}"

text = doc.extract_text(0)
assert isinstance(text, str), f"extract_text(0) returned {type(text)}, expected str"

print(
    f"[OK] {sys.platform} — pdf_oxide loaded, "
    f"PDF {major}.{minor}, {pages} page(s), {len(text)} chars extracted"
)
