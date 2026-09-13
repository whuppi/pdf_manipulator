// StreamingTable with rowspan and batch_size
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
	if err != nil { log.Fatal(err) }
}

func main() {
	outDir := "output"
	must(os.MkdirAll(outDir, 0o755))

	cfg := pdfoxide.StreamingTableConfig{
		Columns: []pdfoxide.Column{
			{Header: "Category", Width: 120},
			{Header: "Item",     Width: 160},
			{Header: "Notes",    Width: 150, Align: pdfoxide.AlignRight},
		},
		RepeatHeader: true,
		MaxRowspan:   2,
		BatchSize:    2,
	}

	b, err := pdfoxide.NewDocumentBuilder()
	must(err)
	defer b.Close()
	must(b.Title("StreamingTable Demo"))

	page, err := b.LetterPage()
	must(err)
	page.Font("Helvetica", 10).At(72, 700).Heading(1, "Product Catalogue").At(72, 660)

	tbl := page.StreamingTable(cfg)
	must(tbl.PushRowSpan([]pdfoxide.SpanCell{
		{Text: "Fruits",  Rowspan: 2},
		{Text: "Apple",   Rowspan: 1},
		{Text: "crisp",   Rowspan: 1},
	}))
	must(tbl.PushRowSpan([]pdfoxide.SpanCell{
		{Text: "",       Rowspan: 1},
		{Text: "Banana", Rowspan: 1},
		{Text: "sweet",  Rowspan: 1},
	}))
	must(tbl.PushRowSpan([]pdfoxide.SpanCell{
		{Text: "Vegetables", Rowspan: 1},
		{Text: "Carrot",     Rowspan: 1},
		{Text: "earthy",     Rowspan: 1},
	}))
	fmt.Printf("  batch_count=%d, pending=%d\n", tbl.BatchCount(), tbl.PendingRowCount())
	_, err = tbl.Finish().Done()
	must(err)

	path := filepath.Join(outDir, "streaming_table_rowspan.pdf")
	must(b.Save(path))
	fmt.Printf("Written: %s\n", path)
}
