// Encrypted in-memory save
//
// Demonstrates SaveEncryptedToBytes: build a PDF, open it in the editor,
// encrypt it with AES-256 and return the bytes without touching disk mid-way.
// Run: go run -tags pdf_oxide_dev main.go

package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

func must(err error) {
	if err != nil {
		log.Fatal(err)
	}
}

func main() {
	outDir := "output"
	must(os.MkdirAll(outDir, 0o755))

	// Build a simple PDF in memory.
	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("Encrypted Bytes Demo"))

	page, err := b.LetterPage()
	must(err)
	_, err = page.
		Font("Helvetica", 12).
		At(72, 720).
		Heading(1, "Encrypted In-Memory PDF").
		At(72, 690).
		Paragraph("This PDF is encrypted with AES-256 directly in memory.").
		Done()
	must(err)

	pdfBytes, err := b.Build()
	must(err)
	fmt.Printf("Original PDF size: %d bytes\n", len(pdfBytes))

	// Open with editor and encrypt.
	editor, err := pdfoxide.OpenEditorFromBytes(pdfBytes)
	must(err)
	defer editor.Close()

	encrypted, err := editor.SaveEncryptedToBytes("user123", "owner123")
	must(err)
	if len(encrypted) == 0 {
		log.Fatal("encrypted PDF is empty")
	}
	fmt.Printf("Encrypted PDF size: %d bytes\n", len(encrypted))

	path := filepath.Join(outDir, "encrypted.pdf")
	must(os.WriteFile(path, encrypted, 0o644))
	fmt.Printf("Written: %s\n", path)
}
