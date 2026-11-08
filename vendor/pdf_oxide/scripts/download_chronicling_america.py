#!/usr/bin/env python3
"""
Download historical newspaper PDFs from Chronicling America (Library of Congress).

Chronicling America provides access to historic newspaper pages from 1756-1963.
All newspapers are in the public domain (>95 years old).

API Documentation: https://chroniclingamerica.loc.gov/about/api/

Usage:
    python3 download_chronicling_america.py --max 100 --output test_datasets/pdfs_1000/newspapers
"""

import argparse
import json
import time
import urllib.error
import urllib.request
from pathlib import Path


# Sample of diverse newspapers from different states and time periods
SAMPLE_NEWSPAPERS = [
    {"lccn": "sn83030214", "title": "New York Times", "state": "NY", "years": "1857-1922"},
    {
        "lccn": "sn83045462",
        "title": "Evening Star (Washington DC)",
        "state": "DC",
        "years": "1854-1972",
    },
    {"lccn": "sn84026749", "title": "San Francisco Call", "state": "CA", "years": "1895-1913"},
    {"lccn": "sn83030313", "title": "The Sun (NY)", "state": "NY", "years": "1833-1916"},
    {"lccn": "sn83045487", "title": "Washington Times", "state": "DC", "years": "1902-1939"},
    {"lccn": "sn84031492", "title": "Appeal (St. Paul, MN)", "state": "MN", "years": "1889-1923"},
    {"lccn": "sn83030272", "title": "New-York Tribune", "state": "NY", "years": "1866-1924"},
    {"lccn": "sn85038615", "title": "Richmond Planet", "state": "VA", "years": "1894-1938"},
    {"lccn": "sn84026847", "title": "Los Angeles Herald", "state": "CA", "years": "1900-1911"},
    {"lccn": "sn84026844", "title": "San Francisco Examiner", "state": "CA", "years": "1895-1922"},
]


def get_newspaper_pages(lccn, max_pages=10):
    """Get list of available pages for a newspaper using Chronicling America API."""
    try:
        # Search for pages from this newspaper
        url = f"https://chroniclingamerica.loc.gov/lccn/{lccn}.json"

        headers = {"User-Agent": "Mozilla/5.0 (PDF Library Testing)"}

        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=30) as response:
            data = json.loads(response.read().decode("utf-8"))

        # Get issues
        issues = data.get("issues", [])
        pages = []

        for issue in issues[:max_pages]:
            issue_url = issue.get("url")
            if issue_url:
                # Fetch issue details to get pages
                issue_req = urllib.request.Request(issue_url + ".json", headers=headers)
                with urllib.request.urlopen(issue_req, timeout=30) as issue_response:
                    issue_data = json.loads(issue_response.read().decode("utf-8"))

                    # Get first page of each issue
                    issue_pages = issue_data.get("pages", [])
                    if issue_pages:
                        page_url = issue_pages[0].get("url")
                        if page_url:
                            pages.append(
                                {"url": page_url, "date": issue.get("date_issued", "unknown")}
                            )

                        if len(pages) >= max_pages:
                            break

        return pages[:max_pages]

    except Exception as e:
        print(f"    Error fetching pages: {e}")
        return []


def download_newspaper_page(page_info, newspaper_title, output_dir):
    """Download a single newspaper page as PDF."""
    try:
        page_url = page_info["url"]
        date = page_info["date"]

        # Get PDF URL (Chronicling America provides PDF for each page)
        pdf_url = page_url + ".pdf"

        # Create safe filename
        safe_title = (
            newspaper_title.replace(" ", "_").replace("(", "").replace(")", "").replace(",", "")
        )
        filename = f"{safe_title}_{date}.pdf"
        output_file = output_dir / filename

        if output_file.exists():
            return False, "exists"

        headers = {"User-Agent": "Mozilla/5.0 (PDF Library Testing)"}

        req = urllib.request.Request(pdf_url, headers=headers)
        with urllib.request.urlopen(req, timeout=60) as response:
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
        return False, str(e)


def main():
    parser = argparse.ArgumentParser(
        description="Download historical newspapers from Chronicling America"
    )
    parser.add_argument("--max", type=int, default=100, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/newspapers", help="Output directory"
    )
    parser.add_argument(
        "--pages-per-newspaper", type=int, default=10, help="Pages to download per newspaper"
    )

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print("=== Chronicling America Newspaper Downloader ===")
    print("Source: Library of Congress")
    print(f"Output directory: {output_dir}")
    print(f"Target: {args.max} newspaper pages")
    print()

    successful = 0
    failed = 0
    skipped = 0

    for newspaper in SAMPLE_NEWSPAPERS:
        if successful >= args.max:
            break

        lccn = newspaper["lccn"]
        title = newspaper["title"]
        state = newspaper["state"]

        print(f"Newspaper: {title} ({state})")
        print("  Fetching available pages...")

        pages = get_newspaper_pages(lccn, args.pages_per_newspaper)

        if not pages:
            print("  No pages found")
            continue

        print(f"  Found {len(pages)} pages, downloading...")

        for page in pages:
            if successful >= args.max:
                break

            success, result = download_newspaper_page(page, title, output_dir)

            if success:
                successful += 1
                size_kb = result // 1024
                print(f"    ✓ {page['date']} ({size_kb} KB) [{successful}/{args.max}]")
            elif result == "exists":
                skipped += 1
                print(f"    - {page['date']} (already exists)")
            else:
                failed += 1
                print(f"    ✗ {page['date']} (error: {result})")

            # Be nice to LOC servers
            time.sleep(1)

        print()

    print("=== Download Complete ===")
    print(f"Downloaded: {successful}")
    print(f"Skipped: {skipped}")
    print(f"Failed: {failed}")
    print()
    print("Notes:")
    print("- All newspapers are in the public domain (>95 years old)")
    print("- Pages are from 1756-1963")
    print("- Diverse layouts: multi-column, historical typography")


if __name__ == "__main__":
    main()
