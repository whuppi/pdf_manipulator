#!/usr/bin/env python3
"""
Download historical newspaper PDFs from Internet Archive.

Internet Archive has thousands of digitized newspapers from their
microfilm/microfiche scanning project.

Usage:
    pip install internetarchive
    python3 download_internet_archive_newspapers.py --max 50 --output test_datasets/pdfs_1000/newspapers/archive
"""

import argparse
import sys
from pathlib import Path


try:
    import internetarchive as ia
except ImportError:
    print("Installing internetarchive...")
    import subprocess

    subprocess.check_call([sys.executable, "-m", "pip", "install", "internetarchive"])
    import internetarchive as ia

# Known newspaper collections on Internet Archive
NEWSPAPER_SEARCHES = [
    "newspaper mediatype:texts format:pdf",
    "gazette mediatype:texts format:pdf",
    "journal mediatype:texts format:pdf",
    "tribune mediatype:texts format:pdf",
    "herald mediatype:texts format:pdf",
]


def download_from_archive(query, output_dir, max_pdfs=50):
    """Download PDFs matching query from Internet Archive."""
    print(f"\nSearching: '{query}'")

    try:
        results = ia.search_items(query, fields=["identifier", "title"])
        downloaded = 0

        for result in results:
            if downloaded >= max_pdfs:
                break

            identifier = result.get("identifier")
            title = result.get("title", identifier)

            print(f"  Checking: {title}")

            try:
                item = ia.get_item(identifier)

                # Look for PDF files in the item
                pdf_files = [
                    f
                    for f in item.files
                    if f.get("format") == "Text PDF" or f["name"].endswith(".pdf")
                ]

                if not pdf_files:
                    print("    No PDFs found")
                    continue

                # Download first PDF
                pdf_file = pdf_files[0]
                filename = pdf_file["name"]

                # Create safe filename
                safe_title = identifier.replace("/", "_").replace(" ", "_")
                output_file = output_dir / f"IA_{safe_title}.pdf"

                if output_file.exists():
                    print("    Already exists")
                    continue

                print(f"    Downloading {filename}...")

                # Download the file
                item.download(files=[filename], destdir=str(output_dir), no_directory=True)

                # Rename to our naming convention
                downloaded_file = output_dir / filename
                if downloaded_file.exists() and not output_file.exists():
                    downloaded_file.rename(output_file)
                    downloaded += 1
                    size_mb = output_file.stat().st_size // 1024 // 1024
                    print(f"    ✓ Downloaded ({size_mb} MB)")

            except Exception as e:
                print(f"    ✗ Error: {e}")
                continue

        return downloaded

    except Exception as e:
        print(f"  Search error: {e}")
        return 0


def main():
    parser = argparse.ArgumentParser(description="Download newspapers from Internet Archive")
    parser.add_argument("--max", type=int, default=50, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/newspapers/archive", help="Output directory"
    )

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print("=== Internet Archive Newspaper Downloader ===")
    print("Source: archive.org")
    print(f"Output directory: {output_dir}")
    print(f"Target: {args.max} newspaper PDFs")
    print()

    total_downloaded = 0
    per_search = max(5, args.max // len(NEWSPAPER_SEARCHES))

    for search_query in NEWSPAPER_SEARCHES:
        if total_downloaded >= args.max:
            break

        downloaded = download_from_archive(search_query, output_dir, per_search)
        total_downloaded += downloaded

    print()
    print("=== Download Complete ===")
    print(f"Downloaded: {total_downloaded} newspaper PDFs")
    print()
    print("Notes:")
    print("- Newspapers from various time periods and locations")
    print("- Scanned from microfilm/microfiche")
    print("- Public domain or freely accessible")


if __name__ == "__main__":
    main()
