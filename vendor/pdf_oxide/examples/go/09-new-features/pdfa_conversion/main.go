// PDF/A conversion: validate → convert → validate
//
// Demonstrates the full archival pipeline:
//   1. Build a PDF in memory
//   2. Validate PDF/A-2b conformance (expect errors before conversion)
//   3. Convert to PDF/A-2b
//   4. Validate again (expect compliant or fewer errors)
// Run: go run -tags pdf_oxide_dev main.go

package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func must(err error) {
	if err != nil {
		log.Fatal(err)
	}
}

func main() {
	outDir := "output"
	must(os.MkdirAll(outDir, 0o755))

	// Build a simple PDF in memory.
	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("PDF/A-2b Conversion Demo"))

	page, err := b.LetterPage()
	must(err)
	_, err = page.
		Font("Helvetica", 12).
		At(72, 720).
		Heading(1, "PDF/A-2b Conversion Demo").
		At(72, 690).
		Paragraph("This document will be converted to PDF/A-2b archival format.").
		Done()
	must(err)

	pdfBytes, err := b.Build()
	must(err)
	fmt.Printf("Original PDF size: %d bytes\n", len(pdfBytes))

	// Step 1: validate before conversion.
	fmt.Println("Validating PDF/A-2b before conversion...")
	doc, err := pdfoxide.OpenFromBytes(pdfBytes)
	must(err)
	if result, err := doc.ValidatePdfA(2); err != nil {
		fmt.Printf("  skipped: %v\n", err)
	} else {
		fmt.Printf("  compliant: %v, errors: %d\n", result.Compliant, len(result.Errors))
	}
	doc.Close()

	// Step 2: convert to PDF/A-2b (level index 2: 0=A1b 1=A1a 2=A2b ...).
	fmt.Println("Converting to PDF/A-2b...")
	editor, err := pdfoxide.OpenEditorFromBytes(pdfBytes)
	must(err)
	if err := editor.ConvertToPdfA(2); err != nil {
		fmt.Printf("  conversion note: %v\n", err)
	} else {
		fmt.Println("  conversion succeeded")
	}
	outBytes, err := editor.SaveToBytes()
	must(err)
	editor.Close()

	// Step 3: validate after conversion.
	fmt.Println("Validating PDF/A-2b after conversion...")
	doc2, err := pdfoxide.OpenFromBytes(outBytes)
	must(err)
	if result, err := doc2.ValidatePdfA(2); err != nil {
		fmt.Printf("  skipped: %v\n", err)
	} else {
		fmt.Printf("  compliant: %v, errors: %d\n", result.Compliant, len(result.Errors))
	}
	doc2.Close()

	fmt.Printf("Output PDF size: %d bytes\n", len(outBytes))
	path := filepath.Join(outDir, "pdfa.pdf")
	must(os.WriteFile(path, outBytes, 0o644))
	fmt.Printf("Written: %s\n", path)
}
