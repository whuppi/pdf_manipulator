#!/usr/bin/env python3
"""
Download diverse PDFs from various sources for comprehensive testing.

Focuses on documents with DIFFERENT layouts from academic papers:
- Newspapers
- Magazines
- Books
- Brochures
- Newsletters
- Public domain documents

Usage:
    python3 download_diverse_pdfs.py --max 100 --output test_datasets/pdfs_1000/diverse
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# Diverse PDF sources with very different layouts
DIVERSE_SOURCES = [
    # Internet Archive newspapers (various layouts, columns, fonts)
    {
        "url": "https://archive.org/download/sim_american-agriculturist_1842-01_1_1/sim_american-agriculturist_1842-01_1_1.pdf",
        "name": "Newspaper_American_Agriculturist_1842.pdf",
        "type": "newspaper",
    },
    {
        "url": "https://archive.org/download/sim_boston-weekly-magazine_1838-09-29_1_1/sim_boston-weekly-magazine_1838-09-29_1_1.pdf",
        "name": "Magazine_Boston_Weekly_1838.pdf",
        "type": "magazine",
    },
    {
        "url": "https://archive.org/download/sim_scientific-american_1845-08-28_1_1/sim_scientific-american_1845-08-28_1_1.pdf",
        "name": "Magazine_Scientific_American_1845.pdf",
        "type": "magazine",
    },
    # Project Gutenberg books (different from papers)
    {
        "url": "https://www.gutenberg.org/files/1342/1342-pdf.pdf",
        "name": "Book_Pride_and_Prejudice.pdf",
        "type": "book",
    },
    {
        "url": "https://www.gutenberg.org/files/11/11-pdf.pdf",
        "name": "Book_Alice_in_Wonderland.pdf",
        "type": "book",
    },
    {
        "url": "https://www.gutenberg.org/files/84/84-pdf.pdf",
        "name": "Book_Frankenstein.pdf",
        "type": "book",
    },
    # RFC documents (technical but different format)
    {
        "url": "https://www.rfc-editor.org/rfc/rfc1.pdf",
        "name": "RFC_0001_Host_Software.pdf",
        "type": "technical",
    },
    {
        "url": "https://www.rfc-editor.org/rfc/rfc2616.pdf",
        "name": "RFC_2616_HTTP_1_1.pdf",
        "type": "technical",
    },
    {
        "url": "https://www.rfc-editor.org/rfc/rfc793.pdf",
        "name": "RFC_0793_TCP.pdf",
        "type": "technical",
    },
    # WHO documents (health reports, different formatting)
    {
        "url": "https://iris.who.int/bitstream/handle/10665/44102/9789241564182_eng.pdf",
        "name": "WHO_World_Health_Statistics_2012.pdf",
        "type": "report",
    },
    # UN documents
    {
        "url": "https://documents.un.org/api/symbol/access?j=N2145456&t=pdf",
        "name": "UN_Security_Council_Resolution.pdf",
        "type": "government",
    },
    # EU documents
    {
        "url": "https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX:32016R0679",
        "name": "EU_GDPR_Regulation.pdf",
        "type": "legal",
    },
    # NASA documents (space mission reports, technical diagrams)
    {
        "url": "https://ntrs.nasa.gov/api/citations/19930013432/downloads/19930013432.pdf",
        "name": "NASA_Apollo_11_Preliminary_Science_Report.pdf",
        "type": "technical",
    },
]


def download_pdf(source, output_dir):
    """Download a single PDF."""
    output_file = output_dir / source["name"]

    if output_file.exists():
        print(f"  Skipping {source['name']} (already exists)")
        return True

    try:
        print(f"  Downloading {source['name']} ({source['type']})...")
        headers = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"}

        req = urllib.request.Request(source["url"], headers=headers)
        with urllib.request.urlopen(req, timeout=60) as response:
            data = response.read()

        # Verify it's a PDF
        if data[:4] != b"%PDF":
            print("    Warning: Not a PDF file")
            return False

        with open(output_file, "wb") as f:
            f.write(data)

        print(f"    Downloaded {len(data) // 1024} KB")
        return True

    except Exception as e:
        print(f"    Error: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Download diverse PDF documents")
    parser.add_argument("--max", type=int, default=100, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/diverse", help="Output directory"
    )

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print("Downloading diverse PDF documents")
    print(f"Output directory: {output_dir}")
    print()

    successful = 0
    failed = 0

    for i, source in enumerate(DIVERSE_SOURCES[: args.max], 1):
        print(f"[{i}/{min(len(DIVERSE_SOURCES), args.max)}] {source['name']}")

        if download_pdf(source, output_dir):
            successful += 1
        else:
            failed += 1

        # Be nice to servers
        if i < min(len(DIVERSE_SOURCES), args.max):
            time.sleep(2)

    print()
    print(f"Downloaded {successful}/{successful + failed} documents successfully")
    if failed > 0:
        print(f"Failed: {failed}")


if __name__ == "__main__":
    main()
