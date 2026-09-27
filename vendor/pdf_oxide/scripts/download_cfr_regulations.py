#!/usr/bin/env python3
"""
Download Code of Federal Regulations (CFR) PDFs from GovInfo.gov.

The CFR is the codification of the general and permanent rules published
in the Federal Register by the executive departments and agencies of the
Federal Government. It's divided into 50 titles.

Usage:
    python3 download_cfr_regulations.py --max 50 --output test_datasets/pdfs_1000/government/cfr
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# CFR Titles with interesting/diverse content
CFR_TITLES = [
    (7, "Agriculture"),
    (8, "Aliens and Nationality"),
    (10, "Energy"),
    (12, "Banks and Banking"),
    (14, "Aeronautics and Space"),
    (15, "Commerce and Foreign Trade"),
    (16, "Commercial Practices"),
    (17, "Commodity and Securities Exchanges"),
    (18, "Conservation of Power and Water Resources"),
    (19, "Customs Duties"),
    (20, "Employees' Benefits"),
    (21, "Food and Drugs"),
    (24, "Housing and Urban Development"),
    (26, "Internal Revenue"),
    (27, "Alcohol, Tobacco Products and Firearms"),
    (29, "Labor"),
    (30, "Mineral Resources"),
    (33, "Navigation and Navigable Waters"),
    (34, "Education"),
    (36, "Parks, Forests, and Public Property"),
    (37, "Patents, Trademarks, and Copyrights"),
    (38, "Pensions, Bonuses, and Veterans' Relief"),
    (40, "Protection of Environment"),
    (42, "Public Health"),
    (43, "Public Lands: Interior"),
    (44, "Emergency Management and Assistance"),
    (45, "Public Welfare"),
    (47, "Telecommunication"),
    (49, "Transportation"),
    (50, "Wildlife and Fisheries"),
]


def download_cfr_title(title_num, title_name, output_dir, year="2024"):
    """Download a CFR title PDF."""
    # GovInfo.gov URL pattern for CFR PDFs
    # Example: https://www.govinfo.gov/content/pkg/CFR-2024-title21-vol1/pdf/CFR-2024-title21-vol1.pdf

    # Most titles have multiple volumes, try volume 1 first
    url = f"https://www.govinfo.gov/content/pkg/CFR-{year}-title{title_num}-vol1/pdf/CFR-{year}-title{title_num}-vol1.pdf"
    output_file = (
        output_dir / f"CFR_{year}_Title{title_num:02d}_Vol1_{title_name.replace(' ', '_')}.pdf"
    )

    if output_file.exists():
        print(f"  Skipping Title {title_num} (already exists)")
        return True

    try:
        print(f"  Downloading Title {title_num}: {title_name}...")
        headers = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) PDF Library Testing"}

        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=60) as response:
            data = response.read()

        # Verify it's a PDF
        if data[:4] != b"%PDF":
            print("    Warning: Not a PDF file")
            return False

        with open(output_file, "wb") as f:
            f.write(data)

        print(f"    Downloaded {len(data) // 1024 // 1024} MB")
        return True

    except urllib.error.HTTPError as e:
        if e.code == 404:
            print(f"    Not found for {year} (may not have vol1)")
        else:
            print(f"    HTTP Error {e.code}")
        return False
    except Exception as e:
        print(f"    Error: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Download Code of Federal Regulations PDFs")
    parser.add_argument("--max", type=int, default=50, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/government/cfr", help="Output directory"
    )
    parser.add_argument("--year", default="2024", help="CFR year edition")

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Downloading Code of Federal Regulations ({args.year})")
    print(f"Output directory: {output_dir}")
    print()

    successful = 0
    failed = 0

    for title_num, title_name in CFR_TITLES[: args.max]:
        print(
            f"[{successful + failed + 1}/{min(len(CFR_TITLES), args.max)}] Title {title_num}: {title_name}"
        )

        if download_cfr_title(title_num, title_name, output_dir, args.year):
            successful += 1
        else:
            failed += 1

        if successful >= args.max:
            break

        # Be nice to GovInfo servers
        time.sleep(2)

    print()
    print(f"Downloaded {successful}/{successful + failed} CFR titles successfully")
    print(f"Failed: {failed}")
    print()
    print("Note: CFR PDFs are large (often 10-50 MB each)")
    print("      They contain comprehensive federal regulations")


if __name__ == "__main__":
    main()
