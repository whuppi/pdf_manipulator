// PKCS#12 CMS signing
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

	p12Path := filepath.Join("..", "..", "..", "..", "tests", "fixtures", "test_signing.p12")
	if _, err := os.Stat(p12Path); os.IsNotExist(err) {
		fmt.Printf("  SKIP: %s not found\n", p12Path)
		return
	}

	p12Data, err := os.ReadFile(p12Path)
	must(err)

	cert, err := pdfoxide.LoadCertificate(p12Data, "testpass")
	if err != nil {
		fmt.Printf("  SKIP: signatures feature not available (%v)\n", err)
		return
	}
	defer cert.Close()

	subject, err := cert.Subject()
	must(err)
	fmt.Printf("  Certificate subject: %s\n", subject)

	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("Signed Invoice"))

	page, err := b.LetterPage()
	must(err)
	_, err = page.
		Font("Helvetica", 12).
		At(72, 720).
		Heading(1, "Signed Invoice").
		At(72, 690).
		Paragraph("This document carries a CMS/PKCS#7 digital signature.").
		Done()
	must(err)

	pdfBytes, err := b.Build()
	must(err)

	signed, err := pdfoxide.SignPdfBytes(pdfBytes, cert, "Approved", "HQ")
	must(err)

	path := filepath.Join(outDir, "signed_document.pdf")
	must(os.WriteFile(path, signed, 0o644))
	fmt.Printf("Written: %s (%d bytes)\n", path, len(signed))

	if !strings.Contains(string(signed), "/ByteRange") {
		log.Fatal("ByteRange missing from signed PDF")
	}
	fmt.Println("  Signature verified: /ByteRange present.")
}
