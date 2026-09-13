#!/usr/bin/env python3
"""
Download PDFs using Playwright to bypass anti-bot protection.

This script uses a real browser to download PDFs from various sources,
avoiding 403 Forbidden errors from automated scripts.

Usage:
    python3 browser_download_pdfs.py --category financial --max 50
    python3 browser_download_pdfs.py --category government --max 50
    python3 browser_download_pdfs.py --category all --max 200
"""

import argparse
import subprocess
import sys
import time
from pathlib import Path


# Check if playwright is installed
try:
    from playwright.sync_api import sync_playwright
except ImportError:
    print("Playwright not found. Installing...")
    subprocess.check_call([sys.executable, "-m", "pip", "install", "playwright"])
    subprocess.check_call([sys.executable, "-m", "playwright", "install", "chromium"])
    from playwright.sync_api import sync_playwright


def download_sec_filings(page, output_dir, max_count=50):
    """Download SEC filings using browser automation."""
    print("\n=== Downloading SEC Filings ===")

    companies = [
        ("AAPL", "0000320193", "Apple"),
        ("TSLA", "0001318605", "Tesla"),
        ("MSFT", "0000789019", "Microsoft"),
        ("GOOGL", "0001652044", "Alphabet"),
        ("AMZN", "0001018724", "Amazon"),
        ("META", "0001326801", "Meta"),
        ("NVDA", "0001045810", "Nvidia"),
        ("JPM", "0000019617", "JPMorgan"),
        ("BAC", "0000070858", "Bank of America"),
        ("WMT", "0000310158", "Walmart"),
    ]

    downloaded = 0

    for ticker, cik, name in companies[: min(len(companies), max_count // 5)]:
        if downloaded >= max_count:
            break

        print(f"\n{name} ({ticker})...")

        # Navigate to company filings page
        url = f"https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK={cik}&type=10-K&dateb=&owner=exclude&count=5"

        try:
            page.goto(url, timeout=30000)
            time.sleep(2)

            # Find document links
            doc_links = page.query_selector_all('a[href*="/Archives/edgar/data/"]')

            for link in doc_links[:3]:  # Get up to 3 filings per company
                if downloaded >= max_count:
                    break

                href = link.get_attribute("href")
                if "/Archives/edgar/data/" in href and href.endswith(".pdf"):
                    pdf_url = f"https://www.sec.gov{href}" if not href.startswith("http") else href
                    filename = f"{ticker}_{Path(href).name}"

                    print(f"  Downloading {filename}...")

                    try:
                        # Download using browser
                        with page.expect_download(timeout=60000) as download_info:
                            page.goto(pdf_url)

                        download = download_info.value
                        download.save_as(output_dir / "10k" / filename)
                        downloaded += 1
                        print(f"    ✓ Downloaded {filename}")
                        time.sleep(1)
                    except Exception as e:
                        print(f"    ✗ Failed: {e}")

        except Exception as e:
            print(f"  Error accessing {name}: {e}")
            continue

    return downloaded


def download_government_pdfs(page, output_dir, max_count=50):
    """Download government PDFs using browser automation."""
    print("\n=== Downloading Government PDFs ===")

    sources = [
        {
            "name": "GAO Reports",
            "url": "https://www.gao.gov/reports-testimonies/all-products",
            "selectors": ['a[href*=".pdf"]', 'a[href*="/assets/"]'],
        },
        {
            "name": "Congressional Reports",
            "url": "https://crsreports.congress.gov/",
            "selectors": ['a[href*="/pdf/"]'],
        },
        {
            "name": "Federal Register",
            "url": "https://www.federalregister.gov/documents/search?conditions%5Bpublication_date%5D%5Byear%5D=2024",
            "selectors": ['a[href$=".pdf"]'],
        },
    ]

    downloaded = 0

    for source in sources:
        if downloaded >= max_count:
            break

        print(f"\n{source['name']}...")

        try:
            page.goto(source["url"], timeout=30000, wait_until="networkidle")
            time.sleep(3)

            # Find PDF links
            pdf_links = []
            for selector in source["selectors"]:
                links = page.query_selector_all(selector)
                pdf_links.extend(links)

            for link in pdf_links[:20]:  # Limit per source
                if downloaded >= max_count:
                    break

                href = link.get_attribute("href")
                if not href or not (".pdf" in href.lower() or "/assets/" in href):
                    continue

                # Make absolute URL
                if href.startswith("/"):
                    base_url = "/".join(source["url"].split("/")[:3])
                    pdf_url = base_url + href
                elif not href.startswith("http"):
                    pdf_url = source["url"].rsplit("/", 1)[0] + "/" + href
                else:
                    pdf_url = href

                filename = f"gov_{Path(pdf_url).name}"

                print(f"  Downloading {filename}...")

                try:
                    with page.expect_download(timeout=60000) as download_info:
                        page.goto(pdf_url)

                    download = download_info.value
                    download.save_as(output_dir / "reports" / filename)
                    downloaded += 1
                    print("    ✓ Downloaded")
                    time.sleep(2)
                except Exception as e:
                    print(f"    ✗ Failed: {e}")
                    continue

        except Exception as e:
            print(f"  Error with {source['name']}: {e}")
            continue

    return downloaded


def download_academic_sources(page, output_dir, max_count=100):
    """Download academic PDFs from open access sources."""
    print("\n=== Downloading Academic PDFs ===")

    sources = [
        {
            "name": "PLOS ONE",
            "base_url": "https://journals.plos.org/plosone/search?filterJournals=PLoSONE&resultsPerPage=50",
            "pdf_pattern": "/plosone/article/file?id=",
        },
        {
            "name": "bioRxiv",
            "base_url": "https://www.biorxiv.org/search/limit_from%3A2024-01-01%20limit_to%3A2024-12-31%20numresults%3A75%20sort%3Apublication-date%20direction%3Adescending",
            "pdf_pattern": ".full.pdf",
        },
    ]

    downloaded = 0

    for source in sources:
        if downloaded >= max_count:
            break

        print(f"\n{source['name']}...")

        try:
            page.goto(source["base_url"], timeout=30000)
            time.sleep(3)

            # Find PDF download links
            pdf_links = page.query_selector_all(f'a[href*="{source["pdf_pattern"]}"]')

            for link in pdf_links[:30]:
                if downloaded >= max_count:
                    break

                href = link.get_attribute("href")
                if not href:
                    continue

                if not href.startswith("http"):
                    base = "/".join(source["base_url"].split("/")[:3])
                    pdf_url = base + href
                else:
                    pdf_url = href

                filename = f"academic_{downloaded:03d}.pdf"

                print(f"  Downloading {filename}...")

                try:
                    with page.expect_download(timeout=60000) as download_info:
                        page.goto(pdf_url)

                    download = download_info.value
                    download.save_as(output_dir / filename)
                    downloaded += 1
                    print("    ✓ Downloaded")
                    time.sleep(2)
                except Exception as e:
                    print(f"    ✗ Failed: {e}")
                    continue

        except Exception as e:
            print(f"  Error with {source['name']}: {e}")
            continue

    return downloaded


def main():
    parser = argparse.ArgumentParser(description="Download PDFs using browser automation")
    parser.add_argument(
        "--category",
        choices=["financial", "government", "academic", "all"],
        default="all",
        help="Category of PDFs to download",
    )
    parser.add_argument("--max", type=int, default=200, help="Maximum PDFs to download")
    parser.add_argument("--output", default="test_datasets/pdfs_1000", help="Output directory")

    args = parser.parse_args()

    # Create output directories
    output_base = Path(args.output)
    (output_base / "financial" / "10k").mkdir(parents=True, exist_ok=True)
    (output_base / "financial" / "10q").mkdir(parents=True, exist_ok=True)
    (output_base / "government" / "reports").mkdir(parents=True, exist_ok=True)
    (output_base / "academic").mkdir(parents=True, exist_ok=True)

    print("=== Browser-Based PDF Downloader ===")
    print(f"Category: {args.category}")
    print(f"Target: {args.max} PDFs")
    print(f"Output: {output_base}")
    print()

    total_downloaded = 0

    with sync_playwright() as p:
        # Launch browser
        print("Launching browser...")
        browser = p.chromium.launch(headless=True)
        context = browser.new_context(
            accept_downloads=True, user_agent="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"
        )
        page = context.new_page()

        try:
            if args.category in ["financial", "all"]:
                max_financial = args.max if args.category == "financial" else args.max // 3
                count = download_sec_filings(page, output_base / "financial", max_financial)
                total_downloaded += count
                print(f"\nFinancial: Downloaded {count} PDFs")

            if args.category in ["government", "all"]:
                max_gov = args.max if args.category == "government" else args.max // 3
                count = download_government_pdfs(page, output_base / "government", max_gov)
                total_downloaded += count
                print(f"\nGovernment: Downloaded {count} PDFs")

            if args.category in ["academic", "all"]:
                max_academic = args.max if args.category == "academic" else args.max // 3
                count = download_academic_sources(page, output_base / "academic", max_academic)
                total_downloaded += count
                print(f"\nAcademic: Downloaded {count} PDFs")

        finally:
            browser.close()

    print()
    print("=== Download Complete ===")
    print(f"Total PDFs downloaded: {total_downloaded}")
    print(f"Location: {output_base}")


if __name__ == "__main__":
    main()
