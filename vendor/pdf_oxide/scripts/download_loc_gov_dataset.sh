#!/bin/bash
# Download Library of Congress 1000 .gov PDF dataset (673.5 MB)
# This provides 1,000 diverse government PDFs from various .gov domains

set -e

OUTPUT_DIR="test_datasets/pdfs_1000/government/loc_dataset"
DOWNLOAD_URL="https://labs.loc.gov/static/labs/work/experiments/1000-govdocs-dataset.zip"

echo "=== Library of Congress 1000 .gov PDF Dataset Downloader ==="
echo "Source: Library of Congress Web Archiving Program"
echo "Size: 673.5 MB (1,000 PDF files)"
echo "Output: $OUTPUT_DIR"
echo

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Download the dataset
echo "Downloading 1000 .gov PDF dataset..."
if command -v wget &> /dev/null; then
    wget -O "$OUTPUT_DIR/1000-govdocs-dataset.zip" "$DOWNLOAD_URL"
elif command -v curl &> /dev/null; then
    curl -L -o "$OUTPUT_DIR/1000-govdocs-dataset.zip" "$DOWNLOAD_URL"
else
    echo "Error: Neither wget nor curl is available"
    exit 1
fi

# Extract the dataset
echo
echo "Extracting PDFs..."
cd "$OUTPUT_DIR"
unzip -q 1000-govdocs-dataset.zip

# Count PDFs
PDF_COUNT=$(find . -name "*.pdf" -type f | wc -l)

echo
echo "=== Download Complete ==="
echo "PDFs extracted: $PDF_COUNT"
echo "Location: $OUTPUT_DIR"
echo
echo "This dataset contains diverse government documents from:"
echo "- Federal agencies"
echo "- State governments"
echo "- Educational institutions (.edu)"
echo "- Various .gov domains"
