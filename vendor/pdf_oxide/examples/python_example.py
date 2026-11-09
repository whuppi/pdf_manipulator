#!/usr/bin/env python3
"""
Example usage of pdf-library Python bindings.

This script demonstrates the main features of the pdf-library:
- Opening PDF files
- Getting document metadata
- Extracting text
- Converting to Markdown
- Converting to HTML
- Multi-page conversion

Requirements:
    pip install pdf-library

Usage:
    python python_example.py <path-to-pdf>
"""

import sys
from pathlib import Path

from pdf_oxide import PdfDocument


def main():
    """Main example function."""
    # Get PDF path from command line or use default test file if available
    pdf_path = sys.argv[1] if len(sys.argv) > 1 else "tests/fixtures/simple.pdf"

    if not Path(pdf_path).exists():
        print(f"Error: File '{pdf_path}' not found")
        print(f"Usage: {sys.argv[0]} <path-to-pdf>")
        sys.exit(1)

    print("=" * 70)
    print("PDF Library - Python Example")
    print("=" * 70)
    print()

    # Open the PDF document
    print(f"Opening: {pdf_path}")
    try:
        doc = PdfDocument(pdf_path)
        print("✓ Successfully opened PDF")
    except OSError as e:
        print(f"✗ Failed to open PDF: {e}")
        sys.exit(1)

    print()

    # Get PDF metadata
    print("-" * 70)
    print("Document Information")
    print("-" * 70)

    major, minor = doc.version()
    print(f"PDF Version: {major}.{minor}")

    page_count = doc.page_count()
    print(f"Total Pages: {page_count}")

    print(f"Representation: {doc!r}")
    print()

    # Extract text from first page
    print("-" * 70)
    print("Text Extraction (Page 1)")
    print("-" * 70)

    try:
        text = doc.extract_text(0)
        print(f"Extracted {len(text)} characters")
        print()

        # Show preview of text (first 200 characters)
        if len(text) > 0:
            preview = text[:200]
            if len(text) > 200:
                preview += "..."
            print("Preview:")
            print(preview)
        else:
            print("(No text found on page 1)")
    except RuntimeError as e:
        print(f"✗ Failed to extract text: {e}")

    print()

    # Convert first page to Markdown
    print("-" * 70)
    print("Markdown Conversion (Page 1)")
    print("-" * 70)

    try:
        # Convert with heading detection enabled
        markdown = doc.to_markdown(0, detect_headings=True, include_images=True)
        output_path = "output_page1.md"

        # Save to file
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(markdown)

        print(f"✓ Saved Markdown to: {output_path}")
        print(f"  Size: {len(markdown)} characters")

        # Show preview (first 3 lines)
        lines = markdown.split("\n")[:3]
        if lines:
            print("  Preview:")
            for line in lines:
                print(f"    {line}")
    except RuntimeError as e:
        print(f"✗ Failed to convert to Markdown: {e}")

    print()

    # Convert first page to HTML (semantic mode)
    print("-" * 70)
    print("HTML Conversion - Semantic (Page 1)")
    print("-" * 70)

    try:
        # Convert with semantic HTML (no layout preservation)
        html = doc.to_html(0, preserve_layout=False, detect_headings=True)
        output_path = "output_page1_semantic.html"

        # Save to file
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(html)

        print(f"✓ Saved HTML (semantic) to: {output_path}")
        print(f"  Size: {len(html)} characters")
    except RuntimeError as e:
        print(f"✗ Failed to convert to HTML: {e}")

    print()

    # Convert first page to HTML (layout mode)
    print("-" * 70)
    print("HTML Conversion - Layout Preserved (Page 1)")
    print("-" * 70)

    try:
        # Convert with layout preservation using CSS positioning
        html = doc.to_html(0, preserve_layout=True, detect_headings=False)
        output_path = "output_page1_layout.html"

        # Save to file
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(html)

        print(f"✓ Saved HTML (layout) to: {output_path}")
        print(f"  Size: {len(html)} characters")
    except RuntimeError as e:
        print(f"✗ Failed to convert to HTML: {e}")

    print()

    # Convert all pages to Markdown
    print("-" * 70)
    print("Full Document Conversion (All Pages)")
    print("-" * 70)

    try:
        # Convert entire document to Markdown
        markdown_all = doc.to_markdown_all(
            detect_headings=True, include_images=True, image_output_dir="./images"
        )
        output_path = "output_full_document.md"

        # Save to file
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(markdown_all)

        print(f"✓ Saved full Markdown to: {output_path}")
        print(f"  Size: {len(markdown_all)} characters")
        print(f"  Pages: {page_count}")
    except RuntimeError as e:
        print(f"✗ Failed to convert full document: {e}")

    print()

    # Convert all pages to HTML
    try:
        html_all = doc.to_html_all(preserve_layout=False, detect_headings=True)
        output_path = "output_full_document.html"

        # Save to file
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(html_all)

        print(f"✓ Saved full HTML to: {output_path}")
        print(f"  Size: {len(html_all)} characters")
    except RuntimeError as e:
        print(f"✗ Failed to convert full document: {e}")

    print()
    print("=" * 70)
    print("Example Complete!")
    print("=" * 70)
    print()
    print("Output files created:")
    print("  - output_page1.md (Markdown, page 1)")
    print("  - output_page1_semantic.html (Semantic HTML, page 1)")
    print("  - output_page1_layout.html (Layout-preserved HTML, page 1)")
    print("  - output_full_document.md (Markdown, all pages)")
    print("  - output_full_document.html (HTML, all pages)")
    print()


if __name__ == "__main__":
    main()
