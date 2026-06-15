# Extract text from every page of a PDF and print it.
# Run: python main.py document.pdf

import sys

from pdf_oxide import PdfDocument


def main():
    if len(sys.argv) < 2:
        print("Usage: python main.py <file.pdf>", file=sys.stderr)
        sys.exit(1)

    path = sys.argv[1]
    doc = PdfDocument(path)

    print(f"Opened: {path}")
    print(f"Pages: {doc.page_count()}\n")

    for i in range(doc.page_count()):
        text = doc.extract_text(i)
        print(f"--- Page {i + 1} ---")
        print(f"{text}\n")


if __name__ == "__main__":
    main()
