#!/usr/bin/env python3
"""
Download government policy documents from GovInfo.gov bulk data repository.

GovInfo provides bulk access to Federal Register, Congressional Records,
Federal Rules, and other government policy documents.

Bulk Data Repository: https://www.govinfo.gov/bulkdata/
API: https://api.govinfo.gov/

Usage:
    python3 download_govinfo_policies.py --max 100 --output test_datasets/pdfs_1000/policies
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# Collections available in GovInfo bulk data
COLLECTIONS = [
    {
        "code": "FR",
        "name": "Federal Register",
        "description": "Daily federal government actions and rules",
    },
    {
        "code": "CFR",
        "name": "Code of Federal Regulations",
        "description": "Codified federal regulations",
    },
    {"code": "BILLS", "name": "Congressional Bills", "description": "Bills from U.S. Congress"},
    {"code": "PLAW", "name": "Public Laws", "description": "Enacted legislation"},
    {"code": "CRPT", "name": "Congressional Reports", "description": "Committee reports"},
    {"code": "CHRG", "name": "Congressional Hearings", "description": "Hearing transcripts"},
]


def get_collection_packages(collection_code, year="2024", limit=10):
    """Get list of available packages in a collection."""
    try:
        # GovInfo bulk data structure: /bulkdata/{collection}/{year}/
        url = f"https://www.govinfo.gov/bulkdata/{collection_code}/{year}"

        headers = {"User-Agent": "Mozilla/5.0 (PDF Library Testing)"}

        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=30) as response:
            html = response.read().decode("utf-8")

        # Parse HTML to find package links (simplified - in production use proper HTML parser)
        packages = []
        lines = html.split("\n")
        for line in lines:
            if 'href="' in line and collection_code in line:
                # Extract package ID from link
                start = line.find('href="') + 6
                end = line.find('"', start)
                if start > 0 and end > 0:
                    link = line[start:end]
                    if link.endswith("/"):
                        package_id = link.rstrip("/").split("/")[-1]
                        if package_id != year:  # Skip year directory itself
                            packages.append(package_id)

        return packages[:limit]

    except Exception as e:
        print(f"    Error fetching packages: {e}")
        return []


def download_package_pdf(collection_code, year, package_id, output_dir, collection_name):
    """Download PDF for a package."""
    try:
        # Try to get PDF directly
        # GovInfo structure: /bulkdata/{collection}/{year}/{package}/pdf/{package}.pdf
        pdf_url = f"https://www.govinfo.gov/content/pkg/{collection_code}-{year}-{package_id}/pdf/{collection_code}-{year}-{package_id}.pdf"

        safe_collection = collection_name.replace(" ", "_")
        filename = f"{safe_collection}_{year}_{package_id}.pdf"
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
        if e.code == 404:
            # Try alternate URL structure
            try:
                alt_pdf_url = (
                    f"https://www.govinfo.gov/content/pkg/{package_id}/pdf/{package_id}.pdf"
                )
                req = urllib.request.Request(alt_pdf_url, headers=headers)
                with urllib.request.urlopen(req, timeout=60) as response:
                    data = response.read()

                if data[:4] != b"%PDF":
                    return False, "not_pdf"

                with open(output_file, "wb") as f:
                    f.write(data)

                return True, len(data)
            except Exception:
                return False, "http_404"
        return False, f"http_{e.code}"
    except Exception as e:
        return False, str(e)[:50]


def download_federal_register_direct(output_dir, max_docs=50):
    """
    Download recent Federal Register documents directly.
    Federal Register has predictable URLs for recent issues.
    """
    print("\nTrying direct Federal Register download...")
    successful = 0

    # Try recent Federal Register issues (2024)
    years = ["2024", "2023"]

    for year in years:
        if successful >= max_docs:
            break

        # Federal Register issues are numbered sequentially
        # Try recent issue numbers
        for issue_num in range(1, 250, 5):  # Sample every 5th issue
            if successful >= max_docs:
                break

            pdf_url = f"https://www.govinfo.gov/content/pkg/FR-{year}-{issue_num:05d}/pdf/FR-{year}-{issue_num:05d}.pdf"

            output_file = output_dir / f"Federal_Register_{year}_{issue_num:05d}.pdf"

            if output_file.exists():
                continue

            try:
                headers = {"User-Agent": "Mozilla/5.0 (PDF Library Testing)"}
                req = urllib.request.Request(pdf_url, headers=headers)

                with urllib.request.urlopen(req, timeout=30) as response:
                    data = response.read()

                if data[:4] == b"%PDF":
                    with open(output_file, "wb") as f:
                        f.write(data)

                    successful += 1
                    size_mb = len(data) // 1024 // 1024
                    print(f"  ✓ Federal Register {year} Issue {issue_num} ({size_mb} MB)")

                time.sleep(2)  # Be nice to servers

            except urllib.error.HTTPError:
                pass  # Issue doesn't exist, continue
            except Exception as e:
                print(f"  ✗ Error: {e}")

    return successful


def main():
    parser = argparse.ArgumentParser(description="Download policy documents from GovInfo")
    parser.add_argument("--max", type=int, default=100, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/policies", help="Output directory"
    )
    parser.add_argument("--year", default="2024", help="Year to download from")

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print("=== GovInfo Policy Documents Downloader ===")
    print("Source: www.govinfo.gov")
    print(f"Output directory: {output_dir}")
    print(f"Target: {args.max} policy documents")
    print()

    # Try direct Federal Register download first (most reliable)
    successful = download_federal_register_direct(output_dir, args.max)

    if successful > 0:
        print()
        print("=== Download Complete ===")
        print(f"Downloaded: {successful} Federal Register issues")
        print()
        print("Notes:")
        print("- Federal Register is the daily journal of the U.S. government")
        print("- Contains rules, proposed rules, notices from federal agencies")
        print("- Public domain documents")
    else:
        print()
        print("=== No Documents Downloaded ===")
        print("GovInfo bulk data may require API key or have changed structure")
        print("Consider using CFR download script instead")


if __name__ == "__main__":
    main()
