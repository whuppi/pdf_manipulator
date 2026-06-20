#!/usr/bin/env python3
"""Convert PDF specification to markdown."""

import pymupdf4llm


print("Converting PDF specification to markdown...")
print("This may take a few minutes for a 750+ page document...")

try:
    md_text = pymupdf4llm.to_markdown(
        "docs/spec/PDF32000_2008.pdf",
        page_chunks=False,  # Single document
    )

    with open("docs/spec/pdf.md", "w", encoding="utf-8") as f:
        f.write(md_text)

    print("✅ Conversion complete!")
    print("   Output: docs/spec/pdf.md")
    print(f"   Size: {len(md_text):,} characters")

except Exception as e:
    print(f"❌ Error: {e}")
    import traceback

    traceback.print_exc()
