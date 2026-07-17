# PDF/A, PDF/X, PDF/UA compliance validation
#
# Run: python main.py

from __future__ import annotations

import pdf_oxide


def main() -> None:
    pdf_bytes: bytes = (
        pdf_oxide.DocumentBuilder()
        .title("Compliance Validation Demo")
        .letter_page()
        .font("Helvetica", 12)
        .at(72, 720)
        .heading(1, "Compliance Validation")
        .at(72, 690)
        .paragraph("Testing PDF/A, PDF/X, and PDF/UA compliance validators.")
        .done()
        .build()
    )

    doc = pdf_oxide.PdfDocument.from_bytes(pdf_bytes)

    print("Validating PDF/A-2b compliance...")
    try:
        result = doc.validate_pdf_a("2b")
        print(f"  is_compliant: {result['valid']}")
        print(f"  errors:   {result['errors']}")
        print(f"  warnings: {result['warnings']}")
    except (RuntimeError, NotImplementedError, AttributeError) as e:
        print(f"  validate_pdf_a skipped or errored: {e}")

    print("Validating PDF/X-4 compliance...")
    try:
        result = doc.validate_pdf_x("4")
        print(f"  is_compliant: {result['valid']}")
        print(f"  errors:   {result['errors']}")
        print(f"  warnings: {result['warnings']}")
    except (RuntimeError, NotImplementedError, AttributeError) as e:
        print(f"  validate_pdf_x skipped or errored: {e}")

    print("Validating PDF/UA-1 compliance...")
    try:
        result = doc.validate_pdf_ua()
        print(f"  is_compliant: {result['valid']}")
        print(f"  errors:   {result['errors']}")
        print(f"  warnings: {result['warnings']}")
    except (RuntimeError, NotImplementedError, AttributeError) as e:
        print(f"  validate_pdf_ua skipped or errored: {e}")


if __name__ == "__main__":
    main()
