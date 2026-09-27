// Dashed stroke lines and rectangles
//
// Demonstrates StrokeRectDashed and StrokeLineDashed on a page.
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

	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("Dashed Stroke Demo"))

	page, err := b.LetterPage()
	must(err)

	page.Font("Helvetica", 12).
		At(72, 720).Heading(1, "Dashed Stroke Demo").
		At(72, 680).Text("Rectangles and lines drawn with configurable dash patterns.")

	// Dashed rectangle — [5 on, 3 off], blue border
	page.StrokeRectDashed(72, 580, 300, 80, 2, 0, 0.2, 0.8, []float32{5, 3}, 0)

	// Dashed line — [8 on, 4 off], red
	page.StrokeLineDashed(72, 550, 372, 550, 1.5, 0.8, 0, 0, []float32{8, 4}, 0)

	// Fine dotted rectangle — [2 on, 2 off] with phase offset, green
	page.StrokeRectDashed(72, 460, 200, 60, 1, 0, 0.6, 0, []float32{2, 2}, 1)

	_, err = page.Done()
	must(err)

	path := filepath.Join(outDir, "dashed_stroke.pdf")
	must(b.Save(path))
	fmt.Printf("Written: %s\n", path)
}
