#!/usr/bin/env python3
"""
Download 10-K filings from SEC EDGAR.

Usage:
    python3 download_sec_filings.py --type 10-K --max 50 --output test_datasets/pdfs_1000/financial/10k
"""

import argparse
import json
import time
import urllib.error
import urllib.request
from pathlib import Path


# Major company CIKs (Central Index Key) for diverse sampling
MAJOR_COMPANIES = [
    ("0000320193", "AAPL"),  # Apple
    ("0001018724", "AMZN"),  # Amazon
    ("0001652044", "GOOGL"),  # Alphabet
    ("0001326801", "META"),  # Meta
    ("0000789019", "MSFT"),  # Microsoft
    ("0001318605", "TSLA"),  # Tesla
    ("0001045810", "NVDA"),  # Nvidia
    ("0001067983", "BRK"),  # Berkshire Hathaway
    ("0000886982", "JNJ"),  # Johnson & Johnson
    ("0000019617", "JPM"),  # JPMorgan Chase
    ("0000051143", "IBM"),  # IBM
    ("0000037996", "F"),  # Ford
    ("0000789019", "XOM"),  # ExxonMobil
    ("0000021344", "KO"),  # Coca-Cola
    ("0000310158", "WMT"),  # Walmart
]


def fetch_company_filings(cik, filing_type, max_filings=5):
    """Fetch filing information for a company."""
    # Format CIK to 10 digits
    cik_padded = str(cik).zfill(10)

    # EDGAR API endpoint
    url = f"https://data.sec.gov/submissions/CIK{cik_padded}.json"

    headers = {
        "User-Agent": "PDF Library Testing bot@example.com"  # SEC requires user agent
    }

    try:
        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req) as response:
            data = json.loads(response.read().decode("utf-8"))

        # Extract recent filings
        filings = data.get("filings", {}).get("recent", {})
        forms = filings.get("form", [])
        accession_numbers = filings.get("accessionNumber", [])
        primary_documents = filings.get("primaryDocument", [])
        filing_dates = filings.get("filingDate", [])

        results = []
        for i, form in enumerate(forms):
            if (
                form == filing_type
                and len(results) < max_filings
                # Check if it's a PDF
                and primary_documents[i].endswith(".pdf")
            ):
                accession = accession_numbers[i].replace("-", "")
                doc_url = f"https://www.sec.gov/Archives/edgar/data/{int(cik)}/{accession}/{primary_documents[i]}"

                results.append(
                    {
                        "company_cik": cik,
                        "form_type": form,
                        "filing_date": filing_dates[i],
                        "url": doc_url,
                        "filename": primary_documents[i],
                    }
                )

        return results

    except Exception as e:
        print(f"Error fetching filings for CIK {cik}: {e}")
        return []


def download_filing(filing, output_path, ticker):
    """Download a single filing."""
    # Create safe filename
    date = filing["filing_date"].replace("-", "")
    filename = f"{ticker}_{filing['form_type']}_{date}.pdf"
    output_file = output_path / filename

    if output_file.exists():
        print(f"  Skipping {filename} (already exists)")
        return True

    try:
        print(f"  Downloading {filename}...")
        headers = {"User-Agent": "PDF Library Testing bot@example.com"}

        req = urllib.request.Request(filing["url"], headers=headers)
        with urllib.request.urlopen(req) as response:
            data = response.read()

        with open(output_file, "wb") as f:
            f.write(data)

        return True

    except Exception as e:
        print(f"  Error downloading {filename}: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Download SEC filings for testing")
    parser.add_argument("--type", default="10-K", help="Filing type (10-K, 10-Q, 8-K)")
    parser.add_argument("--max", type=int, default=50, help="Maximum total filings to download")
    parser.add_argument("--per-company", type=int, default=3, help="Max filings per company")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/financial/10k", help="Output directory"
    )

    args = parser.parse_args()

    # Create output directory
    output_path = Path(args.output)
    output_path.mkdir(parents=True, exist_ok=True)

    print(f"Downloading {args.type} filings")
    print(f"Target: {args.max} total filings")
    print(f"Output directory: {output_path}")
    print()

    total_downloaded = 0
    all_filings = []

    # Collect filings from multiple companies
    for cik, ticker in MAJOR_COMPANIES:
        if total_downloaded >= args.max:
            break

        print(f"Fetching {ticker} ({cik})...")
        filings = fetch_company_filings(cik, args.type, args.per_company)

        for filing in filings:
            if total_downloaded >= args.max:
                break
            filing["ticker"] = ticker
            all_filings.append(filing)
            total_downloaded += 1

        # Be nice to SEC servers
        time.sleep(1)

    print(f"\nFound {len(all_filings)} filings to download")
    print()

    # Download all filings
    successful = 0
    for i, filing in enumerate(all_filings, 1):
        print(
            f"[{i}/{len(all_filings)}] {filing['ticker']} {filing['form_type']} {filing['filing_date']}"
        )

        if download_filing(filing, output_path, filing["ticker"]):
            successful += 1

        # Rate limiting - SEC recommends max 10 requests per second
        if i < len(all_filings):
            time.sleep(0.5)

    print()
    print(f"Downloaded {successful}/{len(all_filings)} filings successfully")


if __name__ == "__main__":
    main()
