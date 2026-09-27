# Encrypted PDF output
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
        .title("Encrypted PDF Demo")
        .letter_page()
        .font("Helvetica", 12)
        .at(72, 720)
        .heading(1, "Encrypted PDF")
        .at(72, 690)
        .paragraph("This PDF is encrypted with a user password.")
        .done()
        .build()
    )
    print(f"  Original PDF size: {len(pdf_bytes)} bytes")

    doc = pdf_oxide.PdfDocument.from_bytes(pdf_bytes)
    encrypted = doc.to_bytes_encrypted(user_password="user123", allow_copy=False)

    assert encrypted.startswith(b"%PDF"), "encrypted output does not start with %PDF"
    assert len(encrypted) > 0, "encrypted output is empty"
    print(f"  Encrypted PDF size: {len(encrypted)} bytes")

    path = os.path.join(OUT_DIR, "encrypted.pdf")
    with open(path, "wb") as f:
        f.write(encrypted)
    print(f"Written: {path}")


if __name__ == "__main__":
    main()
