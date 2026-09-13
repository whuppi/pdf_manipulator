// Create PDFs from Markdown, HTML, and plain text.
// Run: go run main.go

package main

import (
	"fmt"
	"log"
	"os"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func must(err error) {
	if err != nil {
		log.Fatal(err)
	}
}

func main() {
	must(os.MkdirAll("output", 0o755))
	fmt.Println("Creating PDFs...")

	markdown := `# Project Report

## Summary

This document was generated from **Markdown** using pdf_oxide.

- Fast rendering
- Clean typography
- Cross-platform
`
	pdf, err := pdfoxide.FromMarkdown(markdown)
	if err != nil {
		log.Fatalf("Markdown: %v", err)
	}
	must(pdf.Save("output/from_markdown.pdf"))
	fmt.Println("Saved: output/from_markdown.pdf")

	html := `<html><body>
<h1>Invoice #1234</h1>
<p>Generated from <em>HTML</em> using pdf_oxide.</p>
<table><tr><th>Item</th><th>Price</th></tr>
<tr><td>Widget</td><td>$9.99</td></tr></table>
</body></html>`
	pdf, err = pdfoxide.FromHtml(html)
	if err != nil {
		log.Fatalf("HTML: %v", err)
	}
	must(pdf.Save("output/from_html.pdf"))
	fmt.Println("Saved: output/from_html.pdf")

	text := "Hello, World!\n\nThis PDF was created from plain text using pdf_oxide."
	pdf, err = pdfoxide.FromText(text)
	if err != nil {
		log.Fatalf("Text: %v", err)
	}
	must(pdf.Save("output/from_text.pdf"))
	fmt.Println("Saved: output/from_text.pdf")

	fmt.Println("Done. 3 PDFs created in output/")
}
