# PDF/A conversion: validate → convert → compress → bytes
#
# Demonstrates the full IDP archival pipeline:
#   1. Open a PDF from bytes
#   2. Validate PDF/A conformance
#   3. Convert to PDF/A-2b in-place
#   4. Get compressed bytes for upload/storage
#
# Run: python main.py

from __future__ import annotations

import os

import pdf_oxide


OUT_DIR = "output"


def main() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)

    # Build a sample PDF to work with
    source_bytes = pdf_oxide.Pdf.from_markdown(
        "# Archive Me\n\nThis document will be converted to PDF/A."
    ).to_bytes()

    doc = pdf_oxide.PdfDocument.from_bytes(source_bytes)

    # Step 1: validate before conversion
    pre = doc.validate_pdf_a("2b")
    print(f"Before conversion — valid: {pre['valid']}, errors: {len(pre['errors'])}")

    # Step 2: convert to PDF/A-2b in-place
    result = doc.convert_to_pdf_a("2b")
    print(f"Conversion success: {result['success']}")
    print(f"  Actions taken ({len(result['actions'])}):")
    for action in result["actions"]:
        print(f"    - {action}")
    if result["errors"]:
        print(f"  Unfixed issues ({len(result['errors'])}):")
        for err in result["errors"]:
            print(f"    ! {err}")

    # Step 3: validate after conversion
    post = doc.validate_pdf_a("2b")
    print(f"After conversion  — valid: {post['valid']}, errors: {len(post['errors'])}")

    # Step 4: get compressed bytes (e.g. for S3 upload)
    output_bytes = doc.to_bytes(compress=True, garbage_collect=True)
    assert output_bytes[:5] == b"%PDF-", "result is not a valid PDF"
    print(f"Output size: {len(output_bytes):,} bytes")

    path = os.path.join(OUT_DIR, "pdfa_converted.pdf")
    with open(path, "wb") as f:
        f.write(output_bytes)
    print(f"Written: {path}")


if __name__ == "__main__":
    main()
