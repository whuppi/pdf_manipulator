// Open a PDF, modify metadata, delete a page, and save.
// Run: go run main.go input.pdf output.pdf

package main

import (
	"fmt"
	"os"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "Usage: go run main.go <input.pdf> <output.pdf>")
		os.Exit(1)
	}
	input, output := os.Args[1], os.Args[2]

	editor, err := pdfoxide.OpenEditor(input)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	defer editor.Close()
	fmt.Printf("Opened: %s\n", input)

	if err := editor.SetTitle("Edited Document"); err != nil {
		fmt.Fprintf(os.Stderr, "SetTitle error: %v\n", err)
		os.Exit(1)
	}
	fmt.Println(`Set title: "Edited Document"`)

	if err := editor.SetAuthor("pdf_oxide"); err != nil {
		fmt.Fprintf(os.Stderr, "SetAuthor error: %v\n", err)
		os.Exit(1)
	}
	fmt.Println(`Set author: "pdf_oxide"`)

	pages, _ := editor.PageCount()
	if pages > 1 {
		if err := editor.DeletePage(1); err != nil {
			fmt.Fprintf(os.Stderr, "DeletePage error: %v\n", err)
			os.Exit(1)
		}
		fmt.Println("Deleted page 2")
	} else {
		fmt.Println("(skipped delete — single-page document)")
	}

	if err := editor.Save(output); err != nil {
		fmt.Fprintf(os.Stderr, "Save error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Saved: %s\n", output)
}
