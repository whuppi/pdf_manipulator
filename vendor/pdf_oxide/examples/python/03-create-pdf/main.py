# Create PDFs from Markdown, HTML, and plain text.
# Run: python main.py

from pdf_oxide import Pdf


def main():
    print("Creating PDFs...")

    # From Markdown
    markdown = """# Project Report

## Summary

This document was generated from **Markdown** using pdf_oxide.

- Fast rendering
- Clean typography
- Cross-platform
"""
    pdf = Pdf.from_markdown(markdown)
    pdf.save("from_markdown.pdf")
    print("Saved: from_markdown.pdf")

    # From HTML
    html = """<html><body>
<h1>Invoice #1234</h1>
<p>Generated from <em>HTML</em> using pdf_oxide.</p>
<table><tr><th>Item</th><th>Price</th></tr>
<tr><td>Widget</td><td>$9.99</td></tr></table>
</body></html>"""
    pdf = Pdf.from_html(html)
    pdf.save("from_html.pdf")
    print("Saved: from_html.pdf")

    # From plain text
    text = "Hello, World!\n\nThis PDF was created from plain text using pdf_oxide."
    pdf = Pdf.from_text(text)
    pdf.save("from_text.pdf")
    print("Saved: from_text.pdf")

    print("Done. 3 PDFs created.")


if __name__ == "__main__":
    main()
