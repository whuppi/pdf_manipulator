// Extract words with bounding boxes and tables from a PDF page.
// Run: go run main.go document.pdf

package main

import (
	"fmt"
	"os"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: go run main.go <file.pdf>")
		os.Exit(1)
	}
	path := os.Args[1]

	doc, err := pdfoxide.Open(path)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	defer doc.Close()
	fmt.Printf("Opened: %s\n", path)

	page := 0

	words, err := doc.ExtractWords(page)
	if err != nil {
		fmt.Fprintf(os.Stderr, "ExtractWords error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("\n--- Words (page %d) ---\n", page+1)
	limit := len(words)
	if limit > 20 {
		limit = 20
	}
	for _, w := range words[:limit] {
		fmt.Printf("%-20s x=%-7.1f y=%-7.1f w=%-7.1f h=%-7.1f font=%s size=%.1f\n",
			fmt.Sprintf("%q", w.Text), w.X, w.Y, w.Width, w.Height, w.FontName, w.FontSize)
	}
	if len(words) > 20 {
		fmt.Printf("... (%d more words)\n", len(words)-20)
	}

	tables, err := doc.ExtractTables(page)
	if err != nil {
		fmt.Fprintf(os.Stderr, "ExtractTables error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("\n--- Tables (page %d) ---\n", page+1)
	if len(tables) == 0 {
		fmt.Println("(no tables found)")
	}
	for i, t := range tables {
		fmt.Printf("Table %d: %d rows x %d cols\n", i+1, t.RowCount, t.ColCount)
		for r := 0; r < t.RowCount && r < 5; r++ {
			for c := 0; c < t.ColCount && c < 6; c++ {
				fmt.Printf("  [%d,%d] %q", r, c, t.CellText(r, c))
			}
			fmt.Println()
		}
	}
}
