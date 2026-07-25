// Process multiple PDFs concurrently using goroutines.
// Run: go run main.go file1.pdf file2.pdf ...

package main

import (
	"fmt"
	"os"
	"sync"
	"time"

	"github.com/yfedoseev/pdf_oxide/go"
)

type result struct {
	path   string
	pages  int
	words  int
	tables int
	err    error
}

func main() {
	paths := os.Args[1:]
	if len(paths) == 0 {
		fmt.Fprintln(os.Stderr, "Usage: go run main.go <file1.pdf> <file2.pdf> ...")
		os.Exit(1)
	}

	fmt.Printf("Processing %d PDFs concurrently...\n", len(paths))
	start := time.Now()

	results := make([]result, len(paths))
	var wg sync.WaitGroup

	for i, path := range paths {
		wg.Add(1)
		go func(idx int, p string) {
			defer wg.Done()
			doc, err := pdfoxide.Open(p)
			if err != nil {
				results[idx] = result{path: p, err: err}
				return
			}
			defer doc.Close()

			pages, _ := doc.PageCount()
			totalWords, totalTables := 0, 0
			for pg := 0; pg < pages; pg++ {
				if words, err := doc.ExtractWords(pg); err == nil {
					totalWords += len(words)
				}
				if tables, err := doc.ExtractTables(pg); err == nil {
					totalTables += len(tables)
				}
			}
			results[idx] = result{path: p, pages: pages, words: totalWords, tables: totalTables}
		}(i, path)
	}

	wg.Wait()
	for _, r := range results {
		if r.err != nil {
			fmt.Printf("[%s]\tERROR: %v\n", r.path, r.err)
		} else {
			fmt.Printf("[%s]\tpages=%d\twords=%d\ttables=%d\n", r.path, r.pages, r.words, r.tables)
		}
	}
	fmt.Printf("\nDone: %d files processed in %.2fs\n", len(paths), time.Since(start).Seconds())
}
