# Process multiple PDFs concurrently using concurrent.futures.
# Run: python main.py file1.pdf file2.pdf ...

import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed

from pdf_oxide import PdfDocument


def process_pdf(path):
    doc = PdfDocument(path)
    pages = doc.page_count()
    total_words = 0
    total_tables = 0
    for p in range(pages):
        total_words += len(doc.extract_words(p))
        total_tables += len(doc.extract_tables(p))
    return path, pages, total_words, total_tables


def main():
    paths = sys.argv[1:]
    if not paths:
        print("Usage: python main.py <file1.pdf> <file2.pdf> ...")
        sys.exit(1)

    print(f"Processing {len(paths)} PDFs concurrently...")
    start = time.time()

    with ProcessPoolExecutor() as pool:
        futures = {pool.submit(process_pdf, p): p for p in paths}
        for future in as_completed(futures):
            try:
                path, pages, words, tables = future.result()
                print(f"[{path}]\tpages={pages}\twords={words}\ttables={tables}")
            except Exception as e:
                print(f"[{futures[future]}]\tERROR: {e}")

    elapsed = time.time() - start
    print(f"\nDone: {len(paths)} files processed in {elapsed:.2f}s")


if __name__ == "__main__":
    main()
