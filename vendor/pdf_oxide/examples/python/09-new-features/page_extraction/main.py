# Page extraction and chunking
#
# Demonstrates splitting a multi-page PDF into per-chunk byte buffers,
# replacing pypdf's reader.pages slicing workflow.
#
# Run: python main.py

from __future__ import annotations

import os
from itertools import islice

import pdf_oxide


def batched(iterable, n):
    it = iter(iterable)
    while chunk := list(islice(it, n)):
        yield chunk


OUT_DIR = "output"
CHUNK_SIZE = 2


def main() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)

    # Build a 5-page source document
    builder = pdf_oxide.Pdf.from_markdown("# Page 1")
    source_bytes = builder.to_bytes()
    doc = pdf_oxide.PdfDocument.from_bytes(source_bytes)
    for i in range(2, 6):
        extra = pdf_oxide.Pdf.from_markdown(f"# Page {i}").to_bytes()
        doc.merge_from(extra)

    total = doc.page_count()
    print(f"Source document: {total} pages")

    # Split into chunks of CHUNK_SIZE pages — all in memory, no temp files
    chunks = []
    for i, chunk_indices in enumerate(batched(range(total), CHUNK_SIZE)):
        chunk_bytes = doc.extract_pages_to_bytes(list(chunk_indices))
        chunk_doc = pdf_oxide.PdfDocument.from_bytes(chunk_bytes)
        print(
            f"  Chunk {i}: pages {list(chunk_indices)} → {chunk_doc.page_count()} pages, "
            f"{len(chunk_bytes):,} bytes"
        )
        assert chunk_doc.page_count() == len(chunk_indices)
        chunks.append(chunk_bytes)

    print(f"Produced {len(chunks)} chunk(s)")

    # Also demonstrate file-based extraction
    out_path = os.path.join(OUT_DIR, "page_0_only.pdf")
    doc.extract_pages([0], out_path)
    single = pdf_oxide.PdfDocument(out_path)
    assert single.page_count() == 1
    print(f"Single-page file written: {out_path}")

    # Write first chunk to disk as a demo output
    chunk_path = os.path.join(OUT_DIR, "chunk_0.pdf")
    with open(chunk_path, "wb") as f:
        f.write(chunks[0])
    print(f"Written: {chunk_path}")


if __name__ == "__main__":
    main()
