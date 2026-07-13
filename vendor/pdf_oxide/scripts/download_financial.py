#!/usr/bin/env python3
"""
Download financial PDF documents (annual reports, earnings reports).

Since SEC EDGAR files are mostly HTML now, this script uses alternative sources:
- Company annual reports (PDF versions from investor relations)
- Earnings reports
- Proxy statements

Usage:
    python3 download_financial.py --max 30 --output test_datasets/pdfs_1000/financial
"""

import argparse
import time
import urllib.error
import urllib.request
from pathlib import Path


# Direct links to company annual reports and financial documents (PDFs)
# These are typically available on company investor relations pages
FINANCIAL_PDF_SOURCES = [
    # Tech Companies
    {
        "url": "https://s2.q4cdn.com/470004039/files/doc_financials/2023/ar/_10-K-2023-(As-Filed).pdf",
        "name": "Apple_10K_2023.pdf",
        "company": "Apple",
        "type": "10-K",
    },
    {
        "url": "https://s2.q4cdn.com/470004039/files/doc_financials/2023/q4/filing/_10-Q-Q4-2023-As-Filed.pdf",
        "name": "Apple_10Q_Q4_2023.pdf",
        "company": "Apple",
        "type": "10-Q",
    },
    # Alternative: Look for older SEC filings that are still in PDF
    # These are examples of PDF filings from SEC archives
    {
        "url": "https://www.sec.gov/Archives/edgar/data/320193/000032019322000108/aapl-20220924.pdf",
        "name": "Apple_10K_2022_SEC.pdf",
        "company": "Apple",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/1318605/000095017023001409/tsla-20221231.pdf",
        "name": "Tesla_10K_2022_SEC.pdf",
        "company": "Tesla",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/789019/000156459022026876/msft-10k_20220630.pdf",
        "name": "Microsoft_10K_2022_SEC.pdf",
        "company": "Microsoft",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/1652044/000165204423000016/goog-20221231.pdf",
        "name": "Alphabet_10K_2022_SEC.pdf",
        "company": "Alphabet",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/1018724/000101872423000004/amzn-20221231.pdf",
        "name": "Amazon_10K_2022_SEC.pdf",
        "company": "Amazon",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/1326801/000132680123000013/meta-20221231.pdf",
        "name": "Meta_10K_2022_SEC.pdf",
        "company": "Meta",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/1045810/000104581023000029/nvda-20230129.pdf",
        "name": "Nvidia_10K_2023_SEC.pdf",
        "company": "Nvidia",
        "type": "10-K",
    },
    # Financial sector
    {
        "url": "https://www.sec.gov/Archives/edgar/data/19617/000001961723000125/jpm-20221231.pdf",
        "name": "JPMorgan_10K_2022_SEC.pdf",
        "company": "JPMorgan Chase",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/70858/000007085823000033/bac-20221231.pdf",
        "name": "BankOfAmerica_10K_2022_SEC.pdf",
        "company": "Bank of America",
        "type": "10-K",
    },
    # Consumer goods
    {
        "url": "https://www.sec.gov/Archives/edgar/data/21344/000002134423000008/ko-20221231.pdf",
        "name": "CocaCola_10K_2022_SEC.pdf",
        "company": "Coca-Cola",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/310158/000031015823000010/wmt-20230131.pdf",
        "name": "Walmart_10K_2023_SEC.pdf",
        "company": "Walmart",
        "type": "10-K",
    },
    # Industrial
    {
        "url": "https://www.sec.gov/Archives/edgar/data/37996/000003799623000007/f-20221231.pdf",
        "name": "Ford_10K_2022_SEC.pdf",
        "company": "Ford",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/1467373/000146737323000046/gm-20221231.pdf",
        "name": "GM_10K_2022_SEC.pdf",
        "company": "General Motors",
        "type": "10-K",
    },
    # Healthcare
    {
        "url": "https://www.sec.gov/Archives/edgar/data/200406/000020040623000006/jnj-20230101.pdf",
        "name": "JohnsonJohnson_10K_2023_SEC.pdf",
        "company": "Johnson & Johnson",
        "type": "10-K",
    },
    {
        "url": "https://www.sec.gov/Archives/edgar/data/1318605/000156459023003038/tsla-10q_20230331.pdf",
        "name": "Tesla_10Q_Q1_2023_SEC.pdf",
        "company": "Tesla",
        "type": "10-Q",
    },
    # Energy
    {
        "url": "https://www.sec.gov/Archives/edgar/data/34088/000003408823000015/xom-20221231.pdf",
        "name": "ExxonMobil_10K_2022_SEC.pdf",
        "company": "ExxonMobil",
        "type": "10-K",
    },
]


def download_pdf(source, output_path):
    """Download a single financial PDF."""
    output_file = output_path / source["name"]

    if output_file.exists():
        print(f"  Skipping {source['name']} (already exists)")
        return True

    try:
        print(f"  Downloading {source['company']} {source['type']}...")
        headers = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) PDF Library Testing"}

        req = urllib.request.Request(source["url"], headers=headers)
        with urllib.request.urlopen(req, timeout=60) as response:
            data = response.read()

        # Verify it's a PDF
        if data[:4] != b"%PDF":
            print("    Warning: Not a PDF file, skipping")
            return False

        with open(output_file, "wb") as f:
            f.write(data)

        print(f"    Downloaded {len(data) // 1024} KB")
        return True

    except urllib.error.HTTPError as e:
        print(f"    HTTP Error {e.code}: {e.reason}")
        if e.code == 404:
            print("    (File may have been moved or archived)")
        return False
    except Exception as e:
        print(f"    Error: {e}")
        if output_file.exists():
            output_file.unlink()
        return False


def main():
    parser = argparse.ArgumentParser(description="Download financial PDF documents")
    parser.add_argument("--max", type=int, default=30, help="Maximum PDFs to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/financial", help="Output directory"
    )
    parser.add_argument(
        "--type", choices=["10-K", "10-Q", "all"], default="all", help="Type of filing to download"
    )

    args = parser.parse_args()

    # Create output directories
    output_path = Path(args.output)
    (output_path / "10k").mkdir(parents=True, exist_ok=True)
    (output_path / "10q").mkdir(parents=True, exist_ok=True)

    print("Downloading financial PDF documents")
    print(f"Output directory: {output_path}")
    print()

    # Filter sources by type
    sources = FINANCIAL_PDF_SOURCES
    if args.type != "all":
        sources = [s for s in sources if s["type"] == args.type]

    sources = sources[: args.max]

    successful = 0
    failed = 0

    for i, source in enumerate(sources, 1):
        print(f"[{i}/{len(sources)}] {source['company']} {source['type']}")

        # Determine output subdirectory
        if source["type"] == "10-K":
            dest = output_path / "10k"
        elif source["type"] == "10-Q":
            dest = output_path / "10q"
        else:
            dest = output_path

        if download_pdf(source, dest):
            successful += 1
        else:
            failed += 1

        # Be nice to SEC servers
        if i < len(sources):
            time.sleep(1)

    print()
    print(f"Downloaded {successful}/{successful + failed} documents successfully")
    if failed > 0:
        print(f"Failed: {failed} (SEC may have moved/archived some files)")
    print()
    print("Note: SEC EDGAR has moved to HTML format for recent filings.")
    print("These are historical PDF filings from SEC archives (2022-2023).")


if __name__ == "__main__":
    main()
