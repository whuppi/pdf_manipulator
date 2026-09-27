// Extract form fields and annotations from a PDF.
// Run: go run main.go form.pdf

package main

import (
	"fmt"
	"os"

	"github.com/yfedoseev/pdf_oxide/go"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: go run main.go <form.pdf>")
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

	pageCount, err := doc.PageCount()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting page count: %v\n", err)
		os.Exit(1)
	}

	// Form fields are a document-level collection (not per-page).
	fields, err := doc.FormFields()
	if err == nil && len(fields) > 0 {
		fmt.Println("\n--- Form Fields ---")
		for _, f := range fields {
			fmt.Printf("  Name: %-20s Type: %-12s Value: %-16s Required: %v\n",
				fmt.Sprintf("%q", f.Name), f.Type,
				fmt.Sprintf("%q", f.Value), f.Required)
		}
	}

	// Annotations are per-page.
	for page := 0; page < pageCount; page++ {
		annotations, err := doc.Annotations(page)
		if err == nil && len(annotations) > 0 {
			fmt.Printf("\n--- Annotations (page %d) ---\n", page+1)
			for _, a := range annotations {
				fmt.Printf("  Type: %-14s Page: %d   Content: %q\n",
					a.Type, page+1, a.Content)
			}
		}
	}
}
