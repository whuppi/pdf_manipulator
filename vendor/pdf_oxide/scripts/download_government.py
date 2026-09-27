#!/usr/bin/env python3
"""
Download government PDF documents for testing.

Usage:
    python3 download_government.py --max 20 --output test_datasets/pdfs_1000/government/reports
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# Sample government PDF sources with direct links
# These are known to be publicly available
GOVERNMENT_SOURCES = [
    # GAO Reports (Government Accountability Office)
    {
        "url": "https://www.gao.gov/assets/gao-24-106203.pdf",
        "name": "GAO_Defense_Contractor_Business_Systems.pdf",
        "category": "Defense",
    },
    {
        "url": "https://www.gao.gov/assets/gao-24-105743.pdf",
        "name": "GAO_Federal_Real_Property.pdf",
        "category": "Government Operations",
    },
    {
        "url": "https://www.gao.gov/assets/gao-23-106732.pdf",
        "name": "GAO_Medicare_Hospital_Payments.pdf",
        "category": "Healthcare",
    },
    {
        "url": "https://www.gao.gov/assets/gao-24-107073.pdf",
        "name": "GAO_Federal_Student_Aid.pdf",
        "category": "Education",
    },
    {
        "url": "https://www.gao.gov/assets/gao-24-106867.pdf",
        "name": "GAO_Veterans_Mental_Health.pdf",
        "category": "Veterans Affairs",
    },
    # EPA Documents
    {
        "url": "https://www.epa.gov/sites/default/files/2015-09/documents/budget-in-brief-fy-2016.pdf",
        "name": "EPA_Budget_Brief_2016.pdf",
        "category": "Environment",
    },
    # Federal Register samples
    {
        "url": "https://www.govinfo.gov/content/pkg/FR-2023-01-03/pdf/2022-28612.pdf",
        "name": "Federal_Register_2023_01_03.pdf",
        "category": "Regulatory",
    },
    # Congressional Research Service
    {
        "url": "https://crsreports.congress.gov/product/pdf/R/R47032",
        "name": "CRS_Defense_Primer.pdf",
        "category": "Policy",
    },
    # National archives
    {
        "url": "https://www.archives.gov/files/publications/prologue/2023/spring-summer/jfk.pdf",
        "name": "NARA_JFK_Assassination_Records.pdf",
        "category": "Historical",
    },
]


def download_pdf(source, output_path):
    """Download a single PDF."""
    output_file = output_path / source["name"]

    if output_file.exists():
        print(f"  Skipping {source['name']} (already exists)")
        return True

    try:
        print(f"  Downloading {source['name']} ({source['category']})...")
        headers = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"}

        req = urllib.request.Request(source["url"], headers=headers)
        with urllib.request.urlopen(req, timeout=30) as response:
            data = response.read()

        with open(output_file, "wb") as f:
            f.write(data)

        # Verify it's a PDF
        if data[:4] != b"%PDF":
            print(f"    Warning: {source['name']} may not be a valid PDF")
            output_file.unlink()
            return False

        print(f"    Downloaded {len(data) // 1024} KB")
        return True

    except Exception as e:
        print(f"    Error: {e}")
        if output_file.exists():
            output_file.unlink()
        return False


def main():
    parser = argparse.ArgumentParser(description="Download government PDF documents")
    parser.add_argument("--max", type=int, default=20, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/government/reports", help="Output directory"
    )

    args = parser.parse_args()

    # Create output directory
    output_path = Path(args.output)
    output_path.mkdir(parents=True, exist_ok=True)

    print("Downloading government PDF documents")
    print(f"Output directory: {output_path}")
    print()

    successful = 0
    failed = 0

    for i, source in enumerate(GOVERNMENT_SOURCES[: args.max], 1):
        print(f"[{i}/{min(len(GOVERNMENT_SOURCES), args.max)}] {source['name']}")

        if download_pdf(source, output_path):
            successful += 1
        else:
            failed += 1

        # Be nice to servers
        if i < min(len(GOVERNMENT_SOURCES), args.max):
            time.sleep(2)

    print()
    print(f"Downloaded {successful}/{successful + failed} documents successfully")
    if failed > 0:
        print(f"Failed: {failed} (may be network issues or moved URLs)")


if __name__ == "__main__":
    main()
