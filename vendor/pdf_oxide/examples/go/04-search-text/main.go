// Search for a term across all pages of a PDF and print matches.
// Run: go run main.go document.pdf "query"

package main

import (
	"fmt"
	"log"
	"os"

	"github.com/yfedoseev/pdf_oxide/go"
)

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "Usage: go run main.go <file.pdf> <query>")
		os.Exit(1)
	}

	path := os.Args[1]
	query := os.Args[2]

	doc, err := pdfoxide.Open(path)
	if err != nil {
		log.Fatalf("Failed to open %s: %v", path, err)
	}
	defer doc.Close()

	pages, _ := doc.PageCount()
	fmt.Printf("Searching for %q in %s (%d pages)...\n\n", query, path, pages)

	total := 0
	pagesWithHits := 0

	for i := 0; i < pages; i++ {
		results, err := doc.SearchPage(i, query, false)
		if err != nil || len(results) == 0 {
			continue
		}
		pagesWithHits++
		fmt.Printf("Page %d: %d match(es)\n", i+1, len(results))
		for _, r := range results {
			fmt.Printf("  - %q (x=%.1f y=%.1f)\n", r.Text, r.X, r.Y)
			total++
		}
		fmt.Println()
	}

	fmt.Printf("Found %d total matches across %d pages.\n", total, pagesWithHits)
}
