// Page extraction to bytes
//
// Demonstrates ExtractPagesToBytes: build a 2-page PDF, extract only page 0,
// and write the single-page result to disk.
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

	// Build a 2-page PDF in memory.
	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("Page Extraction Demo"))

	page1, err := b.LetterPage()
	must(err)
	_, err = page1.
		Font("Helvetica", 12).
		At(72, 720).
		Heading(1, "Page 1 — Keep This").
		At(72, 690).
		Paragraph("This is the first page. It will be extracted into a separate PDF.").
		Done()
	must(err)

	page2, err := b.LetterPage()
	must(err)
	_, err = page2.
		Font("Helvetica", 12).
		At(72, 720).
		Heading(1, "Page 2 — Discard This").
		At(72, 690).
		Paragraph("This page will NOT appear in the extracted output.").
		Done()
	must(err)

	pdfBytes, err := b.Build()
	must(err)
	fmt.Printf("Original PDF size: %d bytes (2 pages)\n", len(pdfBytes))

	// Open with editor and extract only page 0.
	editor, err := pdfoxide.OpenEditorFromBytes(pdfBytes)
	must(err)
	defer editor.Close()

	extracted, err := editor.ExtractPagesToBytes([]int{0})
	must(err)
	if len(extracted) == 0 {
		log.Fatal("extracted PDF is empty")
	}
	fmt.Printf("Extracted PDF size: %d bytes (1 page)\n", len(extracted))

	path := filepath.Join(outDir, "page_0.pdf")
	must(os.WriteFile(path, extracted, 0o644))
	fmt.Printf("Written: %s\n", path)
}
