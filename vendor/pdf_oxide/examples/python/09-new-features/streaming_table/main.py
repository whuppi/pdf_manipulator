# StreamingTable with rowspan and batch_size
#
# batch_size caps memory: the Rust layer auto-flushes every N rows so the
# client-side buffer stays bounded even for million-row streams.
#
# Run: python main.py

from __future__ import annotations

import os

import pdf_oxide


OUT_DIR = "output"


def main() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)

    doc = pdf_oxide.DocumentBuilder().title("StreamingTable Demo")
    page = (
        doc.letter_page()
        .font("Helvetica", 10)
        .at(72, 700)
        .heading(1, "Product Catalogue")
        .at(72, 660)
    )

    # batch_size=2: Rust auto-flushes every 2 rows, keeping memory bounded.
    tbl = page.streaming_table(
        columns=[
            pdf_oxide.Column("Category", width=120),
            pdf_oxide.Column("Item", width=160),
            pdf_oxide.Column("Notes", width=150, align=pdf_oxide.Align.RIGHT),
        ],
        repeat_header=True,
        max_rowspan=2,
        batch_size=2,
    )
    tbl.push_row_span([("Fruits", 2), ("Apple", 1), ("crisp", 1)])
    tbl.push_row_span([("", 1), ("Banana", 1), ("sweet", 1)])
    tbl.push_row_span([("Vegetables", 1), ("Carrot", 1), ("earthy", 1)])

    print(f"  batch_count={tbl.batch_count()}, pending={tbl.pending_row_count()}")

    path = os.path.join(OUT_DIR, "streaming_table_rowspan.pdf")
    tbl.finish().done().save(path)
    print(f"Written: {path}")


if __name__ == "__main__":
    main()
