#!/usr/bin/env python3
"""
Download PDFs from ArXiv for testing.

Usage:
    python3 download_arxiv.py --category cs.AI --max 20 --output test_datasets/pdfs_1000/academic/arxiv
"""

import argparse
import sys
import time
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path


def fetch_arxiv_papers(category, max_results=20, start_index=0):
    """Fetch paper metadata from ArXiv API."""
    base_url = "http://export.arxiv.org/api/query?"
    query = f"search_query=cat:{category}&start={start_index}&max_results={max_results}&sortBy=submittedDate&sortOrder=descending"

    url = base_url + query

    try:
        with urllib.request.urlopen(url) as response:
            data = response.read().decode("utf-8")
        return data
    except urllib.error.URLError as e:
        print(f"Error fetching from ArXiv: {e}")
        return None


def parse_arxiv_response(xml_data):
    """Parse ArXiv API response XML."""
    try:
        root = ET.fromstring(xml_data)

        # Namespace handling
        ns = {"atom": "http://www.w3.org/2005/Atom", "arxiv": "http://arxiv.org/schemas/atom"}

        entries = []
        for entry in root.findall("atom:entry", ns):
            paper_id = entry.find("atom:id", ns).text.split("/abs/")[-1]
            title = entry.find("atom:title", ns).text.strip().replace("\n", " ")

            # Get PDF link
            pdf_link = None
            for link in entry.findall("atom:link", ns):
                if link.get("title") == "pdf":
                    pdf_link = link.get("href")
                    break

            if pdf_link:
                entries.append({"id": paper_id, "title": title, "pdf_url": pdf_link})

        return entries
    except ET.ParseError as e:
        print(f"Error parsing XML: {e}")
        return []


def download_pdf(url, output_path, filename):
    """Download a PDF file."""
    output_file = output_path / filename

    if output_file.exists():
        print(f"  Skipping {filename} (already exists)")
        return True

    try:
        print(f"  Downloading {filename}...")
        urllib.request.urlretrieve(url, output_file)
        return True
    except Exception as e:
        print(f"  Error downloading {filename}: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Download ArXiv papers for testing")
    parser.add_argument(
        "--category", default="cs.AI", help="ArXiv category (e.g., cs.AI, cs.LG, math.CO)"
    )
    parser.add_argument("--max", type=int, default=20, help="Maximum number of papers to download")
    parser.add_argument(
        "--output", default="test_datasets/pdfs_1000/academic/arxiv", help="Output directory"
    )
    parser.add_argument("--start", type=int, default=0, help="Start index for pagination")

    args = parser.parse_args()

    # Create output directory
    output_path = Path(args.output)
    output_path.mkdir(parents=True, exist_ok=True)

    print(f"Downloading {args.max} papers from {args.category}")
    print(f"Output directory: {output_path}")
    print()

    # Fetch papers
    xml_data = fetch_arxiv_papers(args.category, args.max, args.start)
    if not xml_data:
        print("Failed to fetch paper list")
        sys.exit(1)

    papers = parse_arxiv_response(xml_data)
    print(f"Found {len(papers)} papers")
    print()

    # Download each paper
    successful = 0
    for i, paper in enumerate(papers, 1):
        print(f"[{i}/{len(papers)}] {paper['title'][:60]}...")

        # Create safe filename
        paper_id = paper["id"].replace("/", "_")
        filename = f"arxiv_{paper_id}.pdf"

        if download_pdf(paper["pdf_url"], output_path, filename):
            successful += 1

        # Be nice to ArXiv servers
        if i < len(papers):
            time.sleep(3)  # 3 second delay between downloads

    print()
    print(f"Downloaded {successful}/{len(papers)} papers successfully")


if __name__ == "__main__":
    main()
