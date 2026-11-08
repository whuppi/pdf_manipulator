# Open a PDF, modify metadata, delete a page, and save.
# Run: python main.py input.pdf output.pdf

import sys

from pdf_oxide import PdfDocument


def main():
    if len(sys.argv) < 3:
        print("Usage: python main.py <input.pdf> <output.pdf>")
        sys.exit(1)

    input_path, output_path = sys.argv[1], sys.argv[2]

    doc = PdfDocument(input_path)
    print(f"Opened: {input_path}")

    doc.set_title("Edited Document")
    print('Set title: "Edited Document"')

    doc.set_author("pdf_oxide")
    print('Set author: "pdf_oxide"')

    if doc.page_count() > 1:
        doc.delete_page(1)  # 0-indexed, deletes page 2
        print("Deleted page 2")
    else:
        print("(skipped delete — single-page document)")

    doc.save(output_path)
    print(f"Saved: {output_path}")


if __name__ == "__main__":
    main()
