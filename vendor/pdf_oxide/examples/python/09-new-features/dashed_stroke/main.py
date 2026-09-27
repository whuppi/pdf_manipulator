# Dashed stroke lines and rectangles
#
# Demonstrates stroke_rect_dashed and stroke_line_dashed on a page.
# Run: python main.py

from __future__ import annotations

import os

import pdf_oxide


OUT_DIR = "output"


def main() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)

    doc = pdf_oxide.DocumentBuilder().title("Dashed Stroke Demo")
    page = (
        doc.letter_page()
        .font("Helvetica", 12)
        .at(72, 720)
        .heading(1, "Dashed Stroke Demo")
        .at(72, 680)
        .text("Rectangles and lines drawn with configurable dash patterns.")
    )

    # Dashed rectangle — [5 on, 3 off] pattern, blue border
    page.stroke_rect_dashed(
        72,
        580,
        300,
        80,
        dash=[5.0, 3.0],
        width=2.0,
        color=(0.0, 0.2, 0.8),
    )

    # Dashed line — [8 on, 4 off] pattern, red
    page.stroke_line_dashed(
        72,
        550,
        372,
        550,
        dash=[8.0, 4.0],
        width=1.5,
        color=(0.8, 0.0, 0.0),
    )

    # Fine dotted rectangle — [2 on, 2 off] pattern with phase offset, green
    page.stroke_rect_dashed(
        72,
        460,
        200,
        60,
        dash=[2.0, 2.0],
        phase=1.0,
        width=1.0,
        color=(0.0, 0.6, 0.0),
    )

    path = os.path.join(OUT_DIR, "dashed_stroke.pdf")
    page.done().save(path)
    print(f"Written: {path}")


if __name__ == "__main__":
    main()
