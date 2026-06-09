#!/usr/bin/env python3
"""
Download PhD theses and dissertations from various university repositories.

These have very different layouts from academic papers:
- Much longer (100-300 pages vs 10-20)
- Different typography
- Chapters, appendices, bibliographies
- University-specific formatting

Usage:
    python3 download_oatd_theses.py --max 50 --output test_datasets/pdfs_1000/theses
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# Direct links to openly accessible university theses/dissertations
# These are known working URLs from major universities
THESIS_SOURCES = [
    # MIT Theses
    {
        "url": "https://dspace.mit.edu/bitstream/handle/1721.1/7582/41638069-MIT.pdf",
        "name": "MIT_Thesis_CompSci_1.pdf",
    },
    {
        "url": "https://dspace.mit.edu/bitstream/handle/1721.1/68865/755806673-MIT.pdf",
        "name": "MIT_Thesis_AI_1.pdf",
    },
    # Stanford Digital Repository
    {
        "url": "https://stacks.stanford.edu/file/druid:yx282xq2090/thesis-augmented.pdf",
        "name": "Stanford_Thesis_ML_1.pdf",
    },
    # Berkeley EECS
    {
        "url": "https://www2.eecs.berkeley.edu/Pubs/TechRpts/2016/EECS-2016-1.pdf",
        "name": "Berkeley_Thesis_Systems_1.pdf",
    },
    {
        "url": "https://www2.eecs.berkeley.edu/Pubs/TechRpts/2017/EECS-2017-1.pdf",
        "name": "Berkeley_Thesis_Theory_1.pdf",
    },
    {
        "url": "https://www2.eecs.berkeley.edu/Pubs/TechRpts/2018/EECS-2018-1.pdf",
        "name": "Berkeley_Thesis_Security_1.pdf",
    },
    # CMU Technical Reports (similar to theses)
    {
        "url": "https://www.cs.cmu.edu/~./jelsas/papers/p2p-usec06.pdf",
        "name": "CMU_TechReport_Security_1.pdf",
    },
    # ETH Zurich
    {
        "url": "https://www.research-collection.ethz.ch/bitstream/handle/20.500.11850/87535/eth-7046-02.pdf",
        "name": "ETH_Thesis_Math_1.pdf",
    },
    # Caltech Thesis Repository (many open access)
    {
        "url": "https://thesis.library.caltech.edu/1/01/thesis.pdf",
        "name": "Caltech_Thesis_Physics_1.pdf",
    },
    # University of Washington
    {
        "url": "https://digital.lib.washington.edu/researchworks/bitstream/handle/1773/2248/Thesis.pdf",
        "name": "UW_Thesis_1.pdf",
    },
]


def download_thesis(thesis_info, output_dir):
    """Download a single thesis PDF."""
    url = thesis_info["url"]
    filename = thesis_info["name"]
    output_file = output_dir / filename

    if output_file.exists():
        return False, "exists"

    try:
        headers = {"User-Agent": "Mozilla/5.0 (PDF Library Testing)"}

        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=120) as response:  # Longer timeout for large files
            data = response.read()

        # Verify it's a PDF
        if data[:4] != b"%PDF":
            return False, "not_pdf"

        with open(output_file, "wb") as f:
            f.write(data)

        return True, len(data)

    except urllib.error.HTTPError as e:
        return False, f"http_{e.code}"
    except Exception as e:
        return False, str(e)[:50]


def main():
    parser = argparse.ArgumentParser(description="Download university theses and dissertations")
    parser.add_argument("--max", type=int, default=50, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/theses", help="Output directory"
    )

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print("=== University Thesis Downloader ===")
    print("Source: MIT, Stanford, Berkeley, CMU, ETH, Caltech, UW")
    print(f"Output directory: {output_dir}")
    print(f"Target: {args.max} theses")
    print()

    successful = 0
    failed = 0
    skipped = 0

    for i, thesis in enumerate(THESIS_SOURCES, 1):
        if successful >= args.max:
            break

        print(f"[{i}/{len(THESIS_SOURCES)}] {thesis['name']}")

        success, result = download_thesis(thesis, output_dir)

        if success:
            successful += 1
            size_mb = result // 1024 // 1024
            print(f"  ✓ Downloaded ({size_mb} MB)")
        elif result == "exists":
            skipped += 1
            print("  - Already exists")
        else:
            failed += 1
            print(f"  ✗ Error: {result}")

        # Be respectful to university servers
        time.sleep(2)

    print()
    print("=== Download Complete ===")
    print(f"Downloaded: {successful}")
    print(f"Skipped: {skipped}")
    print(f"Failed: {failed}")
    print()
    print("Notes:")
    print("- PhD theses are much longer than papers (100-300 pages)")
    print("- Different typography and formatting")
    print("- Chapters, appendices, extensive bibliographies")
    print("- Good test of long-document processing")


if __name__ == "__main__":
    main()
