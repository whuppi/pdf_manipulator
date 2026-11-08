// PDF/A, PDF/X, PDF/UA compliance validation
//
// Demonstrates ValidatePdfA, ValidatePdfX, and ValidatePdfUa on a simple
// in-memory PDF. Failures are non-fatal: the example prints the result and
// carries on so CI never blocks on a validator returning errors.
// Run: go run -tags pdf_oxide_dev main.go

package main

import (
	"fmt"
	"log"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func must(err error) {
	if err != nil {
		log.Fatal(err)
	}
}

func main() {
	// Build a simple PDF in memory.
	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("Compliance Validation Demo"))

	page, err := b.LetterPage()
	must(err)
	_, err = page.
		Font("Helvetica", 12).
		At(72, 720).
		Heading(1, "Compliance Validation").
		At(72, 690).
		Paragraph("Testing PDF/A, PDF/X, and PDF/UA compliance validators.").
		Done()
	must(err)

	pdfBytes, err := b.Build()
	must(err)

	doc, err := pdfoxide.OpenFromBytes(pdfBytes)
	must(err)
	defer doc.Close()

	// PDF/A-2b (level index 2: 0=A1b 1=A1a 2=A2b ...)
	fmt.Println("Validating PDF/A-2b compliance...")
	if result, err := doc.ValidatePdfA(2); err != nil {
		fmt.Printf("  skipped or errored: %v\n", err)
	} else {
		fmt.Printf("  compliant: %v\n", result.Compliant)
		fmt.Printf("  errors:    %v\n", result.Errors)
		fmt.Printf("  warnings:  %v\n", result.Warnings)
	}

	// PDF/X-4 (level index 2: 0=X1a2001 1=X32002 2=X4)
	fmt.Println("Validating PDF/X-4 compliance...")
	if compliant, messages, err := doc.ValidatePdfX(2); err != nil {
		fmt.Printf("  skipped or errored: %v\n", err)
	} else {
		fmt.Printf("  compliant: %v\n", compliant)
		fmt.Printf("  messages:  %v\n", messages)
	}

	// PDF/UA-1
	fmt.Println("Validating PDF/UA-1 compliance...")
	if compliant, messages, err := doc.ValidatePdfUa(); err != nil {
		fmt.Printf("  skipped or errored: %v\n", err)
	} else {
		fmt.Printf("  compliant: %v\n", compliant)
		fmt.Printf("  messages:  %v\n", messages)
	}
}
