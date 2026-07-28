#!/usr/bin/env python3
"""
Compare text extraction results from different PDF libraries
on the problematic spacing PDF.

Tests:
- PyMuPDF4LLM
- PyMuPDF (raw)
- pdfplumber
- PyPDF2
- pdfminer.six

This helps determine if the spacing issue is in the PDF itself
or specific to our implementation.
"""

import sys
from pathlib import Path


# PDF to test
PDF_PATH = "test_datasets/pdfs/mixed/5PFVA6CO2FP66IJYJJ4YMWOLK5EHRCCD.pdf"


def test_pymupdf4llm():
    """Test with PyMuPDF4LLM (optimized for LLM consumption)"""
    try:
        import pymupdf4llm

        result = pymupdf4llm.to_markdown(PDF_PATH)
        # Extract first page only
        first_page = result.split("\n\n")[0] if result else ""
        return first_page[:500]
    except ImportError:
        return "❌ PyMuPDF4LLM not installed (pip install pymupdf4llm)"
    except Exception as e:
        return f"❌ Error: {e}"


def test_pymupdf_raw():
    """Test with PyMuPDF raw text extraction"""
    try:
        import fitz  # PyMuPDF

        doc = fitz.open(PDF_PATH)
        page = doc[0]
        text = page.get_text("text")
        doc.close()
        return text[:500]
    except ImportError:
        return "❌ PyMuPDF not installed (pip install PyMuPDF)"
    except Exception as e:
        return f"❌ Error: {e}"


def test_pdfplumber():
    """Test with pdfplumber"""
    try:
        import pdfplumber

        with pdfplumber.open(PDF_PATH) as pdf:
            page = pdf.pages[0]
            text = page.extract_text()
            return text[:500] if text else "No text extracted"
    except ImportError:
        return "❌ pdfplumber not installed (pip install pdfplumber)"
    except Exception as e:
        return f"❌ Error: {e}"


def test_pypdf2():
    """Test with PyPDF2"""
    try:
        from PyPDF2 import PdfReader

        reader = PdfReader(PDF_PATH)
        page = reader.pages[0]
        text = page.extract_text()
        return text[:500]
    except ImportError:
        return "❌ PyPDF2 not installed (pip install PyPDF2)"
    except Exception as e:
        return f"❌ Error: {e}"


def test_pdfminer():
    """Test with pdfminer.six"""
    try:
        from io import StringIO

        from pdfminer.high_level import extract_text_to_fp
        from pdfminer.layout import LAParams

        output = StringIO()
        with open(PDF_PATH, "rb") as f:
            extract_text_to_fp(f, output, page_numbers=[0], laparams=LAParams())
        text = output.getvalue()
        return text[:500]
    except ImportError:
        return "❌ pdfminer.six not installed (pip install pdfminer.six)"
    except Exception as e:
        return f"❌ Error: {e}"


def check_spacing_issue(text):
    """Check if text contains the characteristic spacing issue"""
    patterns = [
        "F i s c a l",
        "Y e a r",
        "C o m m e r c e",
    ]

    found_issues = []
    for pattern in patterns:
        if pattern in text:
            found_issues.append(pattern)

    if found_issues:
        return f"❌ SPACING ISSUE FOUND: {', '.join(found_issues)}"
    else:
        return "✅ No spacing issues detected"


def main():
    # Check if PDF exists
    pdf_file = Path(PDF_PATH)
    if not pdf_file.exists():
        print(f"❌ PDF file not found: {PDF_PATH}")
        sys.exit(1)

    print("=" * 80)
    print("PDF Extractor Comparison")
    print("=" * 80)
    print(f"Testing: {PDF_PATH}")
    print()

    extractors = [
        ("PyMuPDF4LLM", test_pymupdf4llm),
        ("PyMuPDF (raw)", test_pymupdf_raw),
        ("pdfplumber", test_pdfplumber),
        ("PyPDF2", test_pypdf2),
        ("pdfminer.six", test_pdfminer),
    ]

    results = {}

    for name, test_func in extractors:
        print(f"\n{'=' * 80}")
        print(f"{name}")
        print("-" * 80)

        text = test_func()
        results[name] = text

        print(text)
        print()
        print(check_spacing_issue(text))

    # Summary
    print("\n" + "=" * 80)
    print("SUMMARY")
    print("=" * 80)

    has_issue = []
    no_issue = []

    for name, text in results.items():
        if "F i s c a l" in text or "Y e a r" in text:
            has_issue.append(name)
        elif not text.startswith("❌"):
            no_issue.append(name)

    if has_issue:
        print(f"\n❌ Libraries with spacing issue: {', '.join(has_issue)}")

    if no_issue:
        print(f"\n✅ Libraries without spacing issue: {', '.join(no_issue)}")

    if not has_issue and no_issue:
        print("\n🎯 CONCLUSION: Only our library has this issue - we need to fix merge threshold")
    elif has_issue and not no_issue:
        print("\n🎯 CONCLUSION: All libraries have this issue - likely a PDF authoring problem")
    elif has_issue and no_issue:
        print("\n🎯 CONCLUSION: Mixed results - some libraries handle it better than others")


if __name__ == "__main__":
    main()
