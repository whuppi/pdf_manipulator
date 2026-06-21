#!/usr/bin/env python3
"""
Check if PDFs are Tagged PDFs with structure trees.

Tagged PDFs contain explicit reading order information in their structure tree
that we should be using instead of heuristic layout analysis!
"""

import pypdf


def check_pdf_structure(pdf_path):
    """Check if a PDF is tagged and has structure information."""
    print(f"\n{'=' * 70}")
    print(f"Analyzing: {pdf_path}")
    print(f"{'=' * 70}\n")

    try:
        with open(pdf_path, "rb") as f:
            reader = pypdf.PdfReader(f)

            # Check PDF version
            if hasattr(reader, "pdf_header"):
                print(f"PDF Version: {reader.pdf_header}")

            # Check for Structure Tree Root (Tagged PDF indicator)
            catalog = reader.trailer["/Root"]

            # Check for MarkInfo
            if "/MarkInfo" in catalog:
                mark_info = catalog["/MarkInfo"]
                print(f"✅ MarkInfo present: {mark_info}")
                if "/Marked" in mark_info:
                    marked = mark_info["/Marked"]
                    if marked:
                        print(f"   → Marked: {marked} (Tagged PDF!)")
                    else:
                        print(f"   → Marked: {marked} (Not tagged)")
            else:
                print("❌ No MarkInfo (likely not tagged)")

            # Check for StructTreeRoot
            if "/StructTreeRoot" in catalog:
                print("✅ StructTreeRoot present (Tagged PDF!)")
                struct_root = catalog["/StructTreeRoot"]
                print(f"   Structure Tree Root: {struct_root}")

                # Check for K (structure children)
                if "/K" in struct_root:
                    k = struct_root["/K"]
                    if isinstance(k, list):
                        print(f"   → Has {len(k)} top-level structure elements")
                    else:
                        print(f"   → Has structure elements: {k}")

                # Check for ParentTree (maps content to structure)
                if "/ParentTree" in struct_root:
                    print("   → Has ParentTree (maps marked content to structure)")

                return True
            else:
                print("❌ No StructTreeRoot (not a Tagged PDF)")
                return False

    except Exception as e:
        print(f"❌ Error analyzing PDF: {e}")
        return False


def main():
    """Check multiple PDFs for structure information."""

    # PDFs to analyze
    pdfs = [
        # Problem PDF (columns mixing)
        "test_datasets/pdfs/academic/arxiv_2510.21165v1.pdf",
        # Simple forms (work well)
        "test_datasets/pdfs/forms/IRS_Form_1040_2024.pdf",
        # Government documents
        "test_datasets/pdfs/government/CFR_2024_Title07_Vol1_Agriculture.pdf",
    ]

    results = {}
    for pdf_path in pdfs:
        try:
            is_tagged = check_pdf_structure(pdf_path)
            results[pdf_path] = is_tagged
        except Exception as e:
            print(f"\n❌ Failed to process {pdf_path}: {e}")
            results[pdf_path] = None

    # Summary
    print(f"\n{'=' * 70}")
    print("SUMMARY")
    print(f"{'=' * 70}\n")

    for pdf_path, is_tagged in results.items():
        filename = pdf_path.split("/")[-1]
        if is_tagged is None:
            print(f"❓ {filename}: ERROR")
        elif is_tagged:
            print(f"✅ {filename}: Tagged PDF (has structure tree)")
        else:
            print(f"❌ {filename}: Not tagged (no structure tree)")

    print("\n" + "=" * 70)
    print("CONCLUSION")
    print("=" * 70)
    print("""
If PDFs ARE tagged (have StructTreeRoot):
  → We should USE the structure tree for reading order!
  → This is PDF-spec-compliant and explicit
  → No heuristics needed!

If PDFs are NOT tagged:
  → We need heuristic layout analysis (XY-Cut, etc.)
  → But make parameters adaptive based on document properties
  → This is a fallback, not the primary approach
""")


if __name__ == "__main__":
    main()
