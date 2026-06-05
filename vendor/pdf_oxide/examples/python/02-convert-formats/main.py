# Convert PDF pages to Markdown, HTML, and plain text files.
# Run: python main.py document.pdf

import os
import sys

from pdf_oxide import PdfDocument


def main():
    if len(sys.argv) < 2:
        print("Usage: python main.py <file.pdf>", file=sys.stderr)
        sys.exit(1)

    path = sys.argv[1]
    doc = PdfDocument(path)

    os.makedirs("output", exist_ok=True)
    pages = doc.page_count()
    print(f"Converting {pages} pages from {path}...")

    for i in range(pages):
        n = i + 1
        for ext, content in [
            ("md", doc.to_markdown(i)),
            ("html", doc.to_html(i)),
            ("txt", doc.extract_text(i)),
        ]:
            filename = f"output/page_{n}.{ext}"
            with open(filename, "w") as f:
                f.write(content)
            print(f"Saved: {filename}")

    print("Done. Files written to output/")


if __name__ == "__main__":
    main()
