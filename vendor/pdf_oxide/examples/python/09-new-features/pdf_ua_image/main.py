# PDF/UA accessible + decorative images
#
# Run: python main.py

from __future__ import annotations

import os

import pdf_oxide


OUT_DIR = "output"

WHITE_PNG = bytes(
    [
        0x89,
        0x50,
        0x4E,
        0x47,
        0x0D,
        0x0A,
        0x1A,
        0x0A,
        0x00,
        0x00,
        0x00,
        0x0D,
        0x49,
        0x48,
        0x44,
        0x52,
        0x00,
        0x00,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0x01,
        0x08,
        0x02,
        0x00,
        0x00,
        0x00,
        0x90,
        0x77,
        0x53,
        0xDE,
        0x00,
        0x00,
        0x00,
        0x0C,
        0x49,
        0x44,
        0x41,
        0x54,
        0x78,
        0x9C,
        0x63,
        0xF8,
        0xFF,
        0xFF,
        0x3F,
        0x00,
        0x05,
        0xFE,
        0x02,
        0xFE,
        0x0D,
        0xEF,
        0x46,
        0xB8,
        0x00,
        0x00,
        0x00,
        0x00,
        0x49,
        0x45,
        0x4E,
        0x44,
        0xAE,
        0x42,
        0x60,
        0x82,
    ]
)


def main() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)

    doc = (
        pdf_oxide.DocumentBuilder().title("Accessible PDF Demo").tagged_pdf_ua1().language("en-US")
    )
    page = (
        doc.a4_page()
        .font("Helvetica", 12)
        .at(72, 750)
        .heading(1, "Accessible document with images")
        .at(72, 720)
        .paragraph("The image below has descriptive alt text for screen readers.")
        .image_with_alt(WHITE_PNG, 72, 580, 100, 100, "A white placeholder image")
        .at(72, 545)
        .paragraph("The logo below is purely decorative and marked as an artifact.")
        .image_artifact(WHITE_PNG, 72, 445, 60, 60)
    )

    path = os.path.join(OUT_DIR, "pdf_ua_accessible_images.pdf")
    page.done().save(path)
    print(f"Written: {path}")


if __name__ == "__main__":
    main()
