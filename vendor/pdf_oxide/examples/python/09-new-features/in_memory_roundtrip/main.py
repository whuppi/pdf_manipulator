# In-memory round-trip: build() → bytes → PdfDocument.from_bytes()
#
# Run: python main.py

from __future__ import annotations

import os

import pdf_oxide


OUT_DIR = "output"


def main() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)

    pdf_bytes: bytes = (
        pdf_oxide.DocumentBuilder()
        .title("In-Memory Round-Trip Demo")
        .letter_page()
        .font("Helvetica", 12)
        .at(72, 720)
        .heading(1, "In-Memory Round-Trip")
        .at(72, 690)
        .paragraph("This PDF was built in memory, never written to disk mid-way.")
        .done()
        .build()
    )

    reader = pdf_oxide.PdfDocument.from_bytes(pdf_bytes)
    text = "\n".join(reader.extract_text(p) for p in range(reader.page_count()))
    print(f"  Extracted {len(text)} chars from in-memory PDF")
    assert "In-Memory" in text, "round-trip text missing"

    path = os.path.join(OUT_DIR, "in_memory_roundtrip.pdf")
    with open(path, "wb") as f:
        f.write(pdf_bytes)
    print(f"Written: {path}")


if __name__ == "__main__":
    main()
