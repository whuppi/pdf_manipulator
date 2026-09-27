#!/usr/bin/env python3
"""
Download SEC EDGAR filings (10-K, 10-Q, 8-K) in bulk using sec-edgar-downloader.

This script downloads financial filings from major companies to provide
diverse financial document layouts for testing.

Usage:
    pip install sec-edgar-downloader
    python3 download_sec_edgar_bulk.py --max 200 --output test_datasets/pdfs_1000/financial/sec
"""

import argparse
import sys
from pathlib import Path


try:
    from sec_edgar_downloader import Downloader
except ImportError:
    print("Installing sec-edgar-downloader...")
    import subprocess

    subprocess.check_call([sys.executable, "-m", "pip", "install", "sec-edgar-downloader"])
    from sec_edgar_downloader import Downloader

# Major companies across diverse sectors
COMPANIES = [
    # Tech
    ("AAPL", "Apple"),
    ("MSFT", "Microsoft"),
    ("GOOGL", "Alphabet"),
    ("AMZN", "Amazon"),
    ("META", "Meta"),
    ("NVDA", "Nvidia"),
    ("TSLA", "Tesla"),
    ("NFLX", "Netflix"),
    ("ADBE", "Adobe"),
    ("CRM", "Salesforce"),
    ("ORCL", "Oracle"),
    ("INTC", "Intel"),
    # Finance
    ("JPM", "JPMorgan"),
    ("BAC", "Bank of America"),
    ("WFC", "Wells Fargo"),
    ("C", "Citigroup"),
    ("GS", "Goldman Sachs"),
    ("MS", "Morgan Stanley"),
    ("BLK", "BlackRock"),
    ("SCHW", "Charles Schwab"),
    # Healthcare
    ("JNJ", "Johnson & Johnson"),
    ("UNH", "UnitedHealth"),
    ("PFE", "Pfizer"),
    ("ABBV", "AbbVie"),
    ("TMO", "Thermo Fisher"),
    ("LLY", "Eli Lilly"),
    # Consumer
    ("WMT", "Walmart"),
    ("HD", "Home Depot"),
    ("PG", "Procter & Gamble"),
    ("KO", "Coca-Cola"),
    ("PEP", "PepsiCo"),
    ("NKE", "Nike"),
    ("MCD", "McDonald's"),
    ("SBUX", "Starbucks"),
    # Industrial
    ("BA", "Boeing"),
    ("CAT", "Caterpillar"),
    ("GE", "General Electric"),
    ("HON", "Honeywell"),
    ("UPS", "UPS"),
    ("LMT", "Lockheed Martin"),
    # Energy
    ("XOM", "ExxonMobil"),
    ("CVX", "Chevron"),
    ("COP", "ConocoPhillips"),
]


def download_filings(ticker, company_name, output_dir, filing_types=None, limit_per_type=2):
    """Download filings for a single company."""
    print(f"\n{company_name} ({ticker})")

    dl = Downloader("PDFLibraryTesting", "testing@pdf-library.dev", output_dir)

    downloaded_count = 0
    if filing_types is None:
        filing_types = ["10-K", "10-Q", "8-K"]
    for filing_type in filing_types:
        try:
            print(f"  Downloading {filing_type} filings...")
            # Download recent filings (after 2020 for more PDFs)
            dl.get(filing_type, ticker, limit=limit_per_type, after="2020-01-01")
            downloaded_count += limit_per_type
            print(f"    ✓ Downloaded up to {limit_per_type} {filing_type} filings")
        except Exception as e:
            print(f"    ✗ Error: {e}")

    return downloaded_count


def main():
    parser = argparse.ArgumentParser(description="Download SEC EDGAR filings in bulk")
    parser.add_argument("--max", type=int, default=200, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/financial/sec", help="Output directory"
    )
    parser.add_argument(
        "--types", default="10-K,10-Q,8-K", help="Filing types to download (comma-separated)"
    )
    parser.add_argument(
        "--limit-per-type", type=int, default=2, help="Number of each filing type per company"
    )

    args = parser.parse_args()

    # Create output directory
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    filing_types = args.types.split(",")

    print("=== SEC EDGAR Bulk Downloader ===")
    print(f"Output directory: {output_dir}")
    print(f"Filing types: {', '.join(filing_types)}")
    print(f"Limit per type: {args.limit_per_type}")
    print(f"Target: {args.max} total filings")
    print()

    total_downloaded = 0

    for ticker, company_name in COMPANIES:
        if total_downloaded >= args.max:
            break

        count = download_filings(
            ticker, company_name, str(output_dir), filing_types, args.limit_per_type
        )
        total_downloaded += count

        print(f"  Progress: {total_downloaded}/{args.max}")

        if total_downloaded >= args.max:
            break

    print()
    print("=== Download Complete ===")
    print(f"Downloaded approximately {total_downloaded} filings")
    print(f"Location: {output_dir}")
    print()
    print("Note: SEC filings are often in HTML format.")
    print("      PDF extraction may vary by filing.")


if __name__ == "__main__":
    main()
