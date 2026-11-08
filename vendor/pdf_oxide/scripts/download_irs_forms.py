#!/usr/bin/env python3
"""
Download IRS tax forms (very different layouts from academic papers).

These forms have:
- Tables and form fields
- Government formatting
- Different fonts and styles
- Checkboxes and fillable fields

Usage:
    python3 download_irs_forms.py --max 50 --output test_datasets/pdfs_1000/forms/irs
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# Popular IRS forms with diverse layouts
IRS_FORMS = [
    # Individual forms
    "1040",
    "1040-SR",
    "1040-X",
    "1040-ES",
    "1040-V",
    "1040-Schedule-A",
    "1040-Schedule-B",
    "1040-Schedule-C",
    "1040-Schedule-D",
    "1040-Schedule-E",
    "1040-Schedule-F",
    "W-2",
    "W-4",
    "W-4P",
    "W-7",
    "W-8BEN",
    "W-9",
    # Business forms
    "1065",
    "1120",
    "1120-S",
    "941",
    "940",
    "990",
    "1099-MISC",
    "1099-NEC",
    "1099-INT",
    "1099-DIV",
    # Instructions (longer documents)
    "i1040",
    "i1040sca",
    "i1040scb",
    "i1040scc",
    "i1065",
    "i1120",
    "i941",
    # Estate and gift
    "706",
    "709",
    "8971",
    # International
    "2555",
    "8938",
    "FBAR",
    # Credits and deductions
    "2441",
    "8812",
    "8862",
    "8863",
    "8880",
    # Additional forms with unique layouts
    "4506-T",
    "4868",
    "8822",
    "9465",
    "3903",
    "5329",
    "8949",
    "SS-4",
    "SS-5",
]


def download_irs_form(form_name, output_dir, year="2024"):
    """Download a single IRS form."""
    # IRS form URL pattern
    url = f"https://www.irs.gov/pub/irs-pdf/f{form_name}.pdf"
    output_file = output_dir / f"IRS_Form_{form_name}_{year}.pdf"

    if output_file.exists():
        print(f"  Skipping {form_name} (already exists)")
        return True

    try:
        print(f"  Downloading Form {form_name}...")
        headers = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) PDF Library Testing"}

        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=30) as response:
            data = response.read()

        # Verify it's a PDF
        if data[:4] != b"%PDF":
            print("    Warning: Not a PDF file")
            return False

        with open(output_file, "wb") as f:
            f.write(data)

        print(f"    Downloaded {len(data) // 1024} KB")
        return True

    except urllib.error.HTTPError as e:
        if e.code == 404:
            print(f"    Not found (form may not exist for {year})")
        else:
            print(f"    HTTP Error {e.code}")
        return False
    except Exception as e:
        print(f"    Error: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Download IRS tax forms")
    parser.add_argument("--max", type=int, default=50, help="Maximum forms to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/forms/irs", help="Output directory"
    )
    parser.add_argument("--year", default="2024", help="Tax year")

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Downloading IRS forms ({args.year})")
    print(f"Output directory: {output_dir}")
    print()

    successful = 0
    failed = 0

    for i, form in enumerate(IRS_FORMS[: args.max], 1):
        print(f"[{i}/{min(len(IRS_FORMS), args.max)}] Form {form}")

        if download_irs_form(form, output_dir, args.year):
            successful += 1
        else:
            failed += 1

        # Be nice to IRS servers
        if i < min(len(IRS_FORMS), args.max):
            time.sleep(1)

    print()
    print(f"Downloaded {successful}/{successful + failed} forms successfully")
    if failed > 0:
        print(f"Failed: {failed} (some forms may not be available in PDF)")


if __name__ == "__main__":
    main()
