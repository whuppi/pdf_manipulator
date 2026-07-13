// Convert PDF pages to Markdown, HTML, and plain text files.
// Run: go run main.go document.pdf

package main

import (
	"fmt"
	"log"
	"os"

	"github.com/yfedoseev/pdf_oxide/go"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: go run main.go <file.pdf>")
		os.Exit(1)
	}

	path := os.Args[1]
	doc, err := pdfoxide.Open(path)
	if err != nil {
		log.Fatalf("Failed to open %s: %v", path, err)
	}
	defer doc.Close()

	os.MkdirAll("output", 0o755)
	pages, _ := doc.PageCount()
	fmt.Printf("Converting %d pages from %s...\n", pages, path)

	for i := 0; i < pages; i++ {
		n := i + 1
		md, _ := doc.ToMarkdown(i)
		os.WriteFile(fmt.Sprintf("output/page_%d.md", n), []byte(md), 0o644)
		fmt.Printf("Saved: output/page_%d.md\n", n)

		html, _ := doc.ToHtml(i)
		os.WriteFile(fmt.Sprintf("output/page_%d.html", n), []byte(html), 0o644)
		fmt.Printf("Saved: output/page_%d.html\n", n)

		text, _ := doc.ExtractText(i)
		os.WriteFile(fmt.Sprintf("output/page_%d.txt", n), []byte(text), 0o644)
		fmt.Printf("Saved: output/page_%d.txt\n", n)
	}

	fmt.Println("Done. Files written to output/")
}
