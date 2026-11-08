#!/usr/bin/env python3
"""
Download WHO (World Health Organization) reports and publications.

WHO provides open access to many reports with diverse layouts and formatting.

Usage:
    python3 download_who_reports.py --max 20 --output test_datasets/pdfs_1000/reports/who
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# Sample WHO reports with direct PDF links
WHO_REPORTS = [
    {
        "url": "https://www.who.int/publications/i/item/WHO-2019-nCoV-therapeutics-2023.2",
        "name": "WHO_COVID_Therapeutics_2023.pdf",
    },
    {
        "url": "https://apps.who.int/iris/rest/bitstreams/1479657/retrieve",
        "name": "WHO_World_Health_Statistics_2023.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/351632/9789240045064-eng.pdf",
        "name": "WHO_Immunization_Agenda_2030.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/44102/9789241548021_eng.pdf",
        "name": "WHO_Guidelines_Drinking_Water_Quality.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/349205/9789240048829-eng.pdf",
        "name": "WHO_Digital_Health_Strategy.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/350151/9789240037823-eng.pdf",
        "name": "WHO_Global_TB_Report_2021.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/345329/9789240028692-eng.pdf",
        "name": "WHO_Mental_Health_Atlas_2020.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/341140/9789240020306-eng.pdf",
        "name": "WHO_World_Malaria_Report_2020.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/336655/9789240015562-eng.pdf",
        "name": "WHO_Violence_Against_Children_2020.pdf",
    },
    {
        "url": "https://iris.who.int/bitstream/handle/10665/333919/9789240010338-eng.pdf",
        "name": "WHO_State_Of_Food_Security_2020.pdf",
    },
]


def download_who_report(report_info, output_dir):
    """Download a single WHO report."""
    url = report_info["url"]
    filename = report_info["name"]
    output_file = output_dir / filename

    if output_file.exists():
        return False, "exists"

    try:
        headers = {"User-Agent": "Mozilla/5.0 (PDF Library Testing)"}

        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=120) as response:
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
    parser = argparse.ArgumentParser(description="Download WHO reports and publications")
    parser.add_argument("--max", type=int, default=20, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/reports/who", help="Output directory"
    )

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print("=== WHO Reports Downloader ===")
    print("Source: World Health Organization (www.who.int)")
    print(f"Output directory: {output_dir}")
    print(f"Target: {args.max} reports")
    print()

    successful = 0
    failed = 0
    skipped = 0

    for i, report in enumerate(WHO_REPORTS[: args.max], 1):
        if successful >= args.max:
            break

        print(f"[{i}/{min(len(WHO_REPORTS), args.max)}] {report['name']}")

        success, result = download_who_report(report, output_dir)

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

        time.sleep(2)  # Be respectful to WHO servers

    print()
    print("=== Download Complete ===")
    print(f"Downloaded: {successful}")
    print(f"Skipped: {skipped}")
    print(f"Failed: {failed}")
    print()
    print("Notes:")
    print("- WHO reports have diverse layouts")
    print("- Mix of text, tables, charts, infographics")
    print("- Public domain documents")
    print("- Different from academic papers and government regulations")


if __name__ == "__main__":
    main()
