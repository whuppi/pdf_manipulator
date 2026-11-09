// PDF/UA accessible + decorative images
// Run: go run main.go

package main

import (
	"fmt"
	"log"
	"os"
	"path/filepath"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

var whitePng = []byte{
	0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
	0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
	0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00,
	0x0C, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
	0x00, 0x05, 0xFE, 0x02, 0xFE, 0x0D, 0xEF, 0x46, 0xB8, 0x00, 0x00, 0x00,
	0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
}

func must(err error) {
	if err != nil { log.Fatal(err) }
}

func main() {
	outDir := "output"
	must(os.MkdirAll(outDir, 0o755))

	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("Accessible PDF Demo"))
	must(b.TaggedPdfUa1())
	must(b.Language("en-US"))

	page, err := b.A4Page()
	must(err)
	_, err = page.
		Font("Helvetica", 12).
		At(72, 750).
		Heading(1, "Accessible document with images").
		At(72, 720).
		Paragraph("The image below has descriptive alt text for screen readers.").
		ImageWithAlt(whitePng, 72, 580, 100, 100, "A white placeholder image").
		At(72, 545).
		Paragraph("The logo below is purely decorative and marked as an artifact.").
		ImageArtifact(whitePng, 72, 445, 60, 60).
		Done()
	must(err)

	path := filepath.Join(outDir, "pdf_ua_accessible_images.pdf")
	must(b.Save(path))
	fmt.Printf("Written: %s\n", path)
}
