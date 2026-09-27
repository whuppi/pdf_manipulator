# Extract form fields and annotations from a PDF.
# Run: python main.py form.pdf

import sys

from pdf_oxide import PdfDocument


def main():
    if len(sys.argv) < 2:
        print("Usage: python main.py <form.pdf>")
        sys.exit(1)

    path = sys.argv[1]
    doc = PdfDocument(path)
    print(f"Opened: {path}")

    # Form fields are document-wide (not per page)
    fields = doc.get_form_fields()
    if fields:
        print("\n--- Form Fields ---")
        for f in fields:
            print(
                f"  Name: {f.name!r:<20} Type: {f.field_type():<12} "
                f"Value: {f.value!r:<16} Required: {f.is_required()}"
            )

    # Annotations are per page
    for page in range(doc.page_count()):
        annotations = doc.get_annotations(page)
        if annotations:
            print(f"\n--- Annotations (page {page + 1}) ---")
            for a in annotations:
                print(f'  Type: {a.subtype:<14} Page: {page + 1}   Contents: "{a.contents or ""}"')


if __name__ == "__main__":
    main()
