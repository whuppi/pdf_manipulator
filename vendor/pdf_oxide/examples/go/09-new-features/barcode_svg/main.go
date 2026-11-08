// Barcode SVG generation
//
// Demonstrates generating 1D barcodes and QR codes as vector SVG strings.
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
	if err != nil {
		log.Fatal(err)
	}
}

func main() {
	outDir := "output"
	must(os.MkdirAll(outDir, 0o755))

	// 1D barcode — Code 128 SVG (format=0)
	bc, err := pdfoxide.GenerateBarcode("PDF-OXIDE-0341", 0, 300)
	must(err)
	defer bc.Close()

	svg, err := bc.SVGData()
	must(err)
	if !strings.HasPrefix(svg, "<svg") {
		log.Fatalf("expected SVG output for Code128, got: %s", svg[:40])
	}
	path := filepath.Join(outDir, "code128.svg")
	must(os.WriteFile(path, []byte(svg), 0o644))
	fmt.Printf("Written: %s (%d bytes)\n", path, len(svg))

	// 1D barcode — EAN-13 SVG (format=2)
	ean, err := pdfoxide.GenerateBarcode("5901234123457", 2, 300)
	must(err)
	defer ean.Close()

	svg, err = ean.SVGData()
	must(err)
	if !strings.HasPrefix(svg, "<svg") {
		log.Fatalf("expected SVG output for EAN-13, got: %s", svg[:40])
	}
	path = filepath.Join(outDir, "ean13.svg")
	must(os.WriteFile(path, []byte(svg), 0o644))
	fmt.Printf("Written: %s (%d bytes)\n", path, len(svg))

	// QR code SVG (errorCorrection=1=Medium, sizePx=256)
	qr, err := pdfoxide.GenerateQRCode("https://github.com/yfedoseev/pdf_oxide", 1, 256)
	must(err)
	defer qr.Close()

	svg, err = qr.SVGData()
	must(err)
	if !strings.HasPrefix(svg, "<svg") {
		log.Fatalf("expected SVG output for QR code, got: %s", svg[:40])
	}
	if !strings.Contains(svg, "<rect") {
		log.Fatal("QR SVG must contain rect elements")
	}
	path = filepath.Join(outDir, "qr_code.svg")
	must(os.WriteFile(path, []byte(svg), 0o644))
	fmt.Printf("Written: %s (%d bytes)\n", path, len(svg))

	fmt.Println("All barcode SVG checks passed.")
}
