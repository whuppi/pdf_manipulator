// In-memory round-trip: Build() → bytes → OpenFromBytes()
// Run: go run main.go

package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func must(err error) {
	if err != nil { log.Fatal(err) }
}

func main() {
	outDir := "output"
	must(os.MkdirAll(outDir, 0o755))

	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("In-Memory Round-Trip Demo"))

	page, err := b.LetterPage()
	must(err)
	_, err = page.
		Font("Helvetica", 12).
		At(72, 720).
		Heading(1, "In-Memory Round-Trip").
		At(72, 690).
		Paragraph("This PDF was built in memory, never written to disk mid-way.").
		Done()
	must(err)

	pdfBytes, err := b.Build()
	must(err)

	doc, err := pdfoxide.OpenFromBytes(pdfBytes)
	must(err)
	defer doc.Close()

	text, err := doc.ExtractText(0)
	must(err)
	fmt.Printf("  Extracted %d chars from in-memory PDF\n", len(text))
	if !strings.Contains(text, "In-Memory") {
		log.Fatal("round-trip text missing")
	}

	path := filepath.Join(outDir, "in_memory_roundtrip.pdf")
	must(os.WriteFile(path, pdfBytes, 0o644))
	fmt.Printf("Written: %s\n", path)
}
