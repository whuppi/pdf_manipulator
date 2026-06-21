// Extract text from every page of a PDF and print it.
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

	pages, _ := doc.PageCount()
	fmt.Printf("Opened: %s\n", path)
	fmt.Printf("Pages: %d\n\n", pages)

	for i := 0; i < pages; i++ {
		text, err := doc.ExtractText(i)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error on page %d: %v\n", i+1, err)
			continue
		}
		fmt.Printf("--- Page %d ---\n", i+1)
		fmt.Printf("%s\n\n", text)
	}
}
